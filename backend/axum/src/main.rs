use std::{net::SocketAddr, process::ExitCode};

use nas_os_axum::{
    apps::services::{
        antivirus, nas_app_manager,
        nas_system_settings::{access, discovery, power::schedule, updates},
    },
    http::app_channel,
    services::{
        background, database, grants, mail_events, manifest, notifications, provisioning, shares,
        storage, sync_ports,
    },
    widgets::services::tasks,
    AppState,
};

fn server_port() -> u16 {
    std::env::var("PORT")
        .ok()
        .and_then(|port| port.parse().ok())
        .unwrap_or(3000)
}

async fn shutdown_signal() {
    let mut terminate = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        .expect("unable to listen for the shutdown signal");
    tokio::select! {
        result = tokio::signal::ctrl_c() => result.expect("unable to listen for the shutdown signal"),
        _ = terminate.recv() => {}
    }
    println!("shutdown requested, closing the server");
}

#[tokio::main]
async fn main() -> ExitCode {
    match run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}

fn request_file(
    arguments: &mut impl Iterator<Item = String>,
    command: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let request = arguments
        .next()
        .ok_or_else(|| format!("{command} requires a request file"))?;
    if arguments.next().is_some() {
        return Err(format!("unexpected arguments for {command}").into());
    }
    Ok(request)
}

async fn run_lifecycle(
    mut arguments: impl Iterator<Item = String>,
) -> Result<(), Box<dyn std::error::Error>> {
    let phase = arguments
        .next()
        .ok_or("missing antivirus lifecycle phase")?;
    if arguments.next().is_some() || !["boot", "shutdown"].contains(&phase.as_str()) {
        return Err("unexpected antivirus lifecycle arguments".into());
    }
    antivirus::executor::lifecycle(phase == "boot")
        .await
        .map_err(Into::into)
}

async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut arguments = std::env::args().skip(1);
    let command = arguments.next();
    match command.as_deref() {
        Some("--antivirus-lifecycle") => run_lifecycle(arguments).await,
        Some("--antivirus-job") => {
            if arguments.next().is_some() {
                return Err("unexpected antivirus arguments".into());
            }
            antivirus::executor::run().await.map_err(Into::into)
        }
        Some("--cleanup-job") => {
            if arguments.next().is_some() {
                return Err("unexpected cleanup arguments".into());
            }
            nas_os_axum::services::cleanup::executor::run()
                .await
                .map_err(Into::into)
        }
        Some("--reconcile-storage") => storage::reconcile().await.map_err(Into::into),
        Some(command @ "--initialize-storage") => {
            let request = request_file(&mut arguments, command)?;
            storage::initialize_reported(std::path::Path::new(&request))
                .await
                .map_err(Into::into)
        }
        Some(command @ "--rename-storage") => {
            let request = request_file(&mut arguments, command)?;
            storage::rename_reported(std::path::Path::new(&request))
                .await
                .map_err(Into::into)
        }
        Some(command @ "--eject-storage") => {
            let request = request_file(&mut arguments, command)?;
            storage::eject::eject_reported(std::path::Path::new(&request))
                .await
                .map_err(Into::into)
        }
        Some(command @ "--read-storage-smart") => {
            let request = request_file(&mut arguments, command)?;
            storage::smart_reported(std::path::Path::new(&request)).map_err(Into::into)
        }
        _ => serve().await,
    }
}

async fn serve() -> Result<(), Box<dyn std::error::Error>> {
    let path = database::location()?;
    let db = database::open(&path).await?;
    notifications::prune(&db).await;
    tasks::prune(&db).await;
    println!("database ready at {}", path.display());

    nas_app_manager::sync_preinstalled(&db).await?;
    // The grants first, so an application started in background gets its lower priority.
    tokio::spawn({
        let db = db.clone();
        async move {
            let manifests = manifest::all().await;
            grants::sync(&manifests).await;
            shares::sync(&manifests).await;
            sync_ports::sync(&manifests).await;
            background::start(db).await;
        }
    });
    tokio::spawn(provisioning::resume(db.clone()));
    tokio::spawn(nas_app_manager::watch_updates(db.clone()));
    tokio::spawn(database::keep_copy(db.clone(), path.clone()));
    access::apply_saved(&db).await;
    discovery::apply_saved(&db).await;
    updates::report_previous(&db).await;

    manifest::ensure_sockets_directory().await;

    let state = AppState::new(db.clone());
    antivirus::resume(db.clone()).await;
    let antivirus_scheduler = tokio::spawn(antivirus::scheduler::run(db.clone()));
    let scheduler = tokio::spawn(schedule::run(db.clone()));
    tokio::spawn(mail_events::watch(db.clone(), state.session.clone()));
    let applications = tokio::spawn(app_channel::serve(state.clone()));

    let address = SocketAddr::from(([127, 0, 0, 1], server_port()));
    let listener = tokio::net::TcpListener::bind(address).await?;

    println!("nas-os listening on http://{address}");

    let result = axum::serve(
        listener,
        nas_os_axum::build_router(state).into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown_signal())
    .await;

    antivirus_scheduler.abort();
    scheduler.abort();
    applications.abort();

    db.close().await;
    Ok(result?)
}
