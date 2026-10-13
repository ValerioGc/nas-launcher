//! Prints the OpenAPI document of the launcher.

fn main() {
    match nas_os_axum::http::routes::openapi().to_pretty_json() {
        Ok(json) => println!("{json}"),
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    }
}
