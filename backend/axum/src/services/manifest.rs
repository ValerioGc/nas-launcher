//! Reads `nas-app.json` from installed packages. Socket paths are derived from app IDs,
//! never trusted from manifests, to prevent proxying arbitrary system sockets.

use crate::services::packages::catalogue::Category;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

const SUPPORTED: u32 = 1;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
struct Declared {
    manifest_version: u32,
    app: DeclaredApp,
    #[serde(default)]
    run: DeclaredRun,
    #[serde(default)]
    ui: DeclaredUi,
    #[serde(default)]
    provides: Vec<Capability>,
    #[serde(default)]
    permissions: DeclaredPermissions,
}

/// Permissions requested beyond the application sandbox.
/// `services::grants` validates paths and network access. `services::system_packages`
/// validates Debian dependencies.
#[derive(Debug, Clone, Default, Deserialize)]
struct DeclaredPermissions {
    #[serde(default)]
    paths: Vec<String>,
    #[serde(default)]
    network: Vec<String>,
    #[serde(default)]
    packages: Vec<String>,
    /// Folders of the data disks shared on the local network, see `services::shares`.
    #[serde(default)]
    shares: bool,
    /// The fixed cleanups of the system and the caches of the NAS, see `services::cleanup`.
    #[serde(default)]
    cleanup: bool,
}

#[derive(Debug, Clone, Deserialize)]
struct DeclaredApp {
    id: String,
    name: String,
    #[serde(default)]
    system: bool,
    #[serde(default)]
    version: String,
    #[serde(default)]
    icon: String,
    #[serde(default)]
    category: Option<Category>,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct DeclaredRun {
    #[serde(default)]
    exec: String,
    #[serde(default)]
    socket: String,
    /// Whether it may stay running without its window, while it says it has work to do.
    #[serde(default)]
    background: bool,
}

#[derive(Debug, Clone, Deserialize)]
struct DeclaredUi {
    #[serde(default = "one")]
    protocol: u32,
}
impl Default for DeclaredUi {
    fn default() -> Self {
        Self { protocol: 1 }
    }
}
fn one() -> u32 {
    1
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Capability {
    pub capability: String,
    #[serde(default = "one")]
    pub version: u32,
    pub endpoint: String,
}

/// An application that answers for a capability, on a path the browser reaches.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, utoipa::ToSchema)]
#[schema(as = CapabilityProvider)]
pub struct Provider {
    pub app: String,
    pub endpoint: String,
    pub version: u32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Manifest {
    pub id: String,
    pub name: String,
    pub system: bool,
    pub version: String,
    pub icon: String,
    pub category: Option<Category>,
    pub protocol: u32,
    pub provides: Vec<Capability>,
    /// The paths `permissions.paths` asks for, as written
    #[serde(skip)]
    pub paths: Vec<String>,
    /// The kinds of outgoing traffic `permissions.network` asks for, as written
    #[serde(skip)]
    pub network: Vec<String>,
    /// The Debian packages `permissions.packages` asks for, as written
    #[serde(skip)]
    pub packages: Vec<String>,
    /// Allows background execution through `run.background` and `services::background`.
    #[serde(skip)]
    pub background: bool,
    /// Allows local-network shares through `permissions.shares` and `services::shares`.
    #[serde(skip)]
    pub shares: bool,
    /// Allows system and cache cleanup through `permissions.cleanup` and `services::cleanup`.
    #[serde(skip)]
    pub cleanup: bool,
    #[serde(skip)]
    pub socket: PathBuf,
    #[serde(skip)]
    pub exec: String,
}

impl Manifest {
    /// Grants handled by `services::grants`, including paths and network capabilities.
    /// The `run:background` entry applies reduced priority to background applications.
    pub fn grants_asked(&self) -> Vec<String> {
        self.paths
            .iter()
            .cloned()
            .chain(self.network.iter().map(|kind| format!("network:{kind}")))
            .chain(self.background.then(|| "run:background".to_owned()))
            .collect()
    }
}

fn apps_directory() -> PathBuf {
    crate::services::packages::apps_directory()
}

pub fn sockets_directory() -> PathBuf {
    if let Ok(configured) = std::env::var("NAS_APP_SOCKETS") {
        return PathBuf::from(configured);
    }

    if cfg!(debug_assertions) {
        if let Some(beside) = crate::services::database::beside("sockets") {
            return beside;
        }
    }

    PathBuf::from("/run/nas-os/apps")
}

pub async fn ensure_sockets_directory() {
    use std::os::unix::fs::PermissionsExt;
    const MODE: u32 = 0o3770;

    let directory = sockets_directory();
    if let Err(error) = tokio::fs::create_dir_all(&directory).await {
        eprintln!("cannot create {}: {error}", directory.display());
        return;
    }

    let Ok(metadata) = tokio::fs::metadata(&directory).await else {
        return;
    };
    if metadata.permissions().mode() & 0o7777 == MODE {
        return;
    }
    if let Err(error) =
        tokio::fs::set_permissions(&directory, std::fs::Permissions::from_mode(MODE)).await
    {
        eprintln!(
            "cannot set the permissions of {}: {error}",
            directory.display()
        );
    }
}

pub fn socket_of(id: &str) -> PathBuf {
    sockets_directory().join(format!("{id}.sock"))
}

/// The manifest of a package unpacked in `directory`, before it is put in place.
pub async fn read_unpacked(directory: &Path, id: &str) -> Option<Manifest> {
    parse(directory, id).await
}

async fn parse(directory: &Path, id: &str) -> Option<Manifest> {
    let file = directory.join("nas-app.json");
    let text = tokio::fs::read_to_string(&file).await.ok()?;

    let declared: Declared = match serde_json::from_str(&text) {
        Ok(declared) => declared,
        Err(error) => {
            eprintln!("unreadable manifest {}: {error}", file.display());
            return None;
        }
    };

    if declared.manifest_version != SUPPORTED {
        eprintln!(
            "{} declares manifest_version {}, which this launcher does not read",
            file.display(),
            declared.manifest_version
        );
        return None;
    }

    if declared.app.id != id {
        eprintln!(
            "{} declares the id {} but sits in {id}: the directory is the one that counts",
            file.display(),
            declared.app.id
        );
        return None;
    }

    let socket = socket_of(id);
    if !declared.run.socket.is_empty() && Path::new(&declared.run.socket) != socket {
        eprintln!(
            "{id} declares the socket {} and will be reached at {}: the launcher derives it",
            declared.run.socket,
            socket.display()
        );
    }

    Some(Manifest {
        id: id.to_owned(),
        name: declared.app.name,
        system: declared.app.system,
        version: declared.app.version,
        icon: declared.app.icon,
        category: declared.app.category,
        protocol: declared.ui.protocol,
        provides: declared.provides,
        paths: declared.permissions.paths,
        network: declared.permissions.network,
        packages: declared.permissions.packages,
        background: declared.run.background,
        shares: declared.permissions.shares,
        cleanup: declared.permissions.cleanup,
        socket,
        exec: declared.run.exec,
    })
}

/// Validates IDs before they become directory, socket or systemd unit names.
pub fn is_identifier(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

fn is_public_endpoint(endpoint: &str) -> bool {
    endpoint.len() <= 256
        && endpoint.starts_with('/')
        && !endpoint.starts_with("//")
        && !endpoint.contains("..")
        && endpoint != "/internal"
        && !endpoint.starts_with("/internal/")
        && endpoint
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "/-_.".contains(c))
}

/// The applications that declare a capability, with an endpoint the proxy serves.
pub fn providers(manifests: &[Manifest], capability: &str) -> Vec<Provider> {
    manifests
        .iter()
        .flat_map(|manifest| {
            manifest
                .provides
                .iter()
                .filter(|offered| {
                    offered.capability == capability && is_public_endpoint(&offered.endpoint)
                })
                .map(|offered| Provider {
                    app: manifest.id.clone(),
                    endpoint: offered.endpoint.clone(),
                    version: offered.version,
                })
        })
        .collect()
}

/// Manifest identity derived from inode, size, and modification time.
type Stamp = (u64, u64, i64, i64);
/// The manifests already read, by their file and the folder of the sockets they were derived with.
type ReadManifests = std::collections::HashMap<(PathBuf, PathBuf), (Stamp, Option<Manifest>)>;

/// Caches parsed manifests and reloads them after file changes.
static READ: std::sync::LazyLock<std::sync::Mutex<ReadManifests>> =
    std::sync::LazyLock::new(Default::default);

fn stamp(metadata: &std::fs::Metadata) -> Stamp {
    use std::os::unix::fs::MetadataExt;
    (
        metadata.ino(),
        metadata.len(),
        metadata.mtime(),
        metadata.mtime_nsec(),
    )
}

fn read_cache() -> std::sync::MutexGuard<'static, ReadManifests> {
    READ.lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

pub async fn read(id: &str) -> Option<Manifest> {
    if !is_identifier(id) {
        return None;
    }

    read_from(&apps_directory().join(id), id).await
}

/// The manifest of the package in `directory`, from the cache when its file did not change.
async fn read_from(directory: &Path, id: &str) -> Option<Manifest> {
    let key = (directory.join("nas-app.json"), sockets_directory());
    let Ok(metadata) = tokio::fs::metadata(&key.0).await else {
        read_cache().remove(&key);
        return None;
    };
    let current = stamp(&metadata);
    if let Some((known, manifest)) = read_cache().get(&key) {
        if *known == current {
            return manifest.clone();
        }
    }
    let manifest = parse(directory, id).await;
    read_cache().insert(key, (current, manifest.clone()));
    manifest
}

pub async fn all() -> Vec<Manifest> {
    let Ok(mut entries) = tokio::fs::read_dir(apps_directory()).await else {
        return Vec::new();
    };

    let mut found = Vec::new();
    while let Ok(Some(entry)) = entries.next_entry().await {
        let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
            continue;
        };
        // Staging and rollback directories start with a dot and are nobody's application.
        if name.starts_with('.') {
            continue;
        }
        if let Some(manifest) = read(&name).await {
            found.push(manifest);
        }
    }
    found.sort_by(|one, other| one.id.cmp(&other.id));
    found
}

#[cfg(test)]
#[path = "../../tests/services/manifest.rs"]
mod tests;
