# Developing nas-os

## Boundaries and structure

```text
src/
  api/client.ts                  # HTTP, errors, and successful-mutation events
  apps/
    registry.ts                  # UI identities and internal application loading
    AppHost.vue                  # Mount open views and show the active one
    AppTabs.vue                  # Tab navigation, desktop, and closing views
    AppView.vue                  # Shared application surface
    components/                  # Primitives shared only by applications
    styles/_apps.scss            # Application styles scoped under .app_surface
    <app>/                       # Domain components, types, and store
  components/                    # Shell and shared controls, including commons/
  components/setup/              # NAS preparation card
  views/                         # Sign-in, console, and preparation pages
  composables/                   # Resources, debounce, polling, and appearance
  utils/                         # Sorting, contrast, account validation, and error formatting
  i18n/launcher/, i18n/apps/      # Italian/English catalogs split by shell and application
  stores/                        # apps, shell, widgets, auth, and setup domains
  styles/                        # Tokens, themes, placeholders, mixins, and reset
  styles/themes/<type>/          # data-palette themes listed in themes/_index.scss
  widgets/WidgetDrawer.vue       # Shared drawer animation, Escape handling, and focus
  widgets/WidgetPanel.vue        # Operation title, polling settings, and close control
  widgets/monitor/               # Host for the nas-system-monitor widget iframe
  widgets/tasks/                 # Operation register widget
  tests/                         # Mirrored unit, component, script, and end-to-end tests
backend/
  axum/src/
    main.rs                      # Open the database and start the server
    utils.rs                     # Shared is_real(Mode), now_ms, and systemctl helpers
    http/                        # Routes, state, errors, extractors, and API boundary
    controllers/                 # Shared shell handlers
    services/                    # Database, settings, sessions, notifications, registry,
                                 # desktop, manifests, lifecycle, packages, and provisioning
    apps/controllers/<app>.rs    # Application routes and handlers
    apps/services/<app>.rs       # Domain logic, types, and validation; large domains use folders
    widgets/controllers/<w>.rs   # Header-widget routes
    widgets/services/<w>.rs      # Widget logic and types
    **/*.sh                      # Privileged helpers beside the Rust module invoking them
  axum/migrations/               # Numbered SQL migrations embedded in the binary
  axum/tests/                    # Rust tests mirroring source modules
  data/                          # Ignored development and end-to-end state
scripts/                         # Local and pipeline build, format, and release scripts
system/apps/                     # Optional preinstalled application archives
system/packages/ui_installation/ # Users, permissions, units, catalog, launcher, and UI
system/packages/ui_configuration/  # Token, terminal user, and data volumes, run by the wizard
docs/user_manual/                # User manual included as a PDF at the package root
```

Gateway and wizard are standalone sibling projects, `../nas-gateway` and `../nas-wizard`, each
with its own repository, version, and pipeline. The wizard prepares the operating system,
downloads and installs both components, and owns the installation manual. This project owns
the user manual describing the interface.

Application types belong in `src/apps/<app>/types.ts`; widget types belong beside their widget.
Keep domain types out of the shell's shared type file. The shell only needs the application
opening contract. The notification menu belongs to nas-notification-manager even though it
appears in the header. Reuse the HTTP client, application frame, and shared controls.

## Internal and external applications

The external application package, release, and API contract is documented in
[APPS.md](docs/development/external_apps/APPS.md).

Internal applications compile into both the UI bundle and backend binary. Changing them requires
a launcher release. Application management, settings, and notifications remain internal because
the shell requires them. Global search is internal because it displays providers' results.
Applications and settings entries are discovered locally. File search uses installed applications
with the `search` capability. The launcher lists them through `GET /api/capabilities/search`;
the browser queries their own routes.

### Application registry

`services/registry.rs` defines built-in applications and owns queries to `installed_apps`.
Built-ins always exist and cannot be installed or removed. Other applications have persistent
`installed_apps` rows. `/etc/nas-os/catalogue.json` lists installable releases.

The manifest's `app.system` and catalog's `system` classify external applications as system
applications without making them built-ins or changing their category. In the standard catalog,
all external applications except Backup and Picture Manager are system applications. The others
use `preinstall` and receive desktop shortcuts during initial setup. Terminal is the external
`nas-system-terminal` application. `services/terminal.rs` retains its sessions, journal access,
and stored preferences, exposed through the application Unix channel.

Installed applications are the union of database records and actual package directories. A
manually unpacked package appears without a row, allowing unpublished builds to be tested. A row
whose directory disappeared remains visible so it can be removed. Where recorded and packaged
versions disagree, the directory wins because it determines the code opened and compared during
update checks.

Icons are served from `/api/apps/{id}/icon`. The resolved path must stay inside the package, use
an image extension, and remain under half a megabyte. Responses include `nosniff` and a restrictive
policy. An SVG displayed as an image is different from opening the SVG document directly, so the
latter must not execute arbitrary package content.

Backend responsibilities follow the same separation.

- `http/` owns route assembly, shared state, typed errors, JSON extraction, and the API boundary.
- `controllers/` and `services/` own shared shell behavior, including health, registry, desktop,
  database, settings, notifications, packages, and application processes.
- `utils.rs` contains common mode switches, `now_ms`, and `systemctl`.
- `apps/` holds matching controller/service modules named after frontend applications in
  snake_case. Large domains use subdirectories. Settings retain access, networking, and power
  services. The external nas-security-center owns the Access UI and reaches existing routes
  through `src/api/access.ts`; enforcement remains in the launcher.
- `widgets/` owns the operation register and common polling preferences. nas-system-monitor owns
  hardware collection and its widget; the launcher hosts only its iframe.

Each group owns its memory state. `SessionData.apps` holds the detected launcher release and
current apt job. Applications use shared services, such as notifications, without importing
another application's modules. Add routers through `apps::controllers::routes` or
`widgets::controllers::routes`. Rust filenames use snake_case; mixed-case modules fail Clippy
with warnings denied.

## Machine operations

`apps/services/nas_system_settings/power.rs` requests restart/shutdown through `systemctl` and
logind. Use a fixed program and enum-derived arguments instead of shell command strings. The
launcher remains an unprivileged service; individual polkit rules grant only the required actions.
Power actions require `NAS_POWER_MODE=real`; simulated responses explicitly identify themselves.

### Factory reset

`apps/services/nas_system_settings/reset.rs` starts `nas-os-reset.service` through polkit rule 75,
which allows only that unit's `start` action. It uses `--no-block` because the root helper stops
the launcher itself. `reset/factory-reset.sh` performs the reset.

- Disable and stop the gateway, applications, and launcher.
- Remove the terminal user and home only when created by `ui_configuration`, identified by the
  `nas-os terminal` account comment.
- Remove launcher, gateway, application, and wizard state, the release token, and network-disk
  passwords. Keep encrypted data disks' automatic keys.
- Remove both NAS firewall tables and the wireless helper. Restore the original hostname,
  networking, DNS, Samba, and service states from the wizard's protected initial backup. Keep
  installed Debian package upgrades.
- Start the installed `nas-wizard.service` again.

Only mounts introduced by the NAS are unmounted, and encrypted volumes opened by it are closed
before package removal. Data-disk files remain intact. A NAS-added package now required by an
upgraded pre-existing package is retained at its current version. Missing wizard binary/unit
returns `noWizard` before deletion. Missing initial configuration returns `noOriginalState`;
never capture an already configured NAS as its original baseline. `GET /api/system/reset` also
returns `disabled` outside real reset mode. The root helper verifies backup integrity before
changes and rejects concurrent updates, package installation, or disk operations. The browser
may disconnect when the original hostname/address returns; reopen the wizard at that address.

The gateway validates the account password through `POST /auth/confirm`, returning a single-use
code valid for one minute. The browser includes it in `X-NAS-Confirmation` on
`POST /api/system/reset`. The gateway consumes it and sets `X-NAS-Reauthenticated`; the launcher
extractor `http::extract::Reauthenticated` returns 403 without that header. Browser-provided
versions of trusted headers are stripped by the gateway.

### Exporting and importing configuration

`apps/services/nas_system_settings/config.rs` exports/imports plaintext configuration through
`GET` and `POST /api/system/config`, both requiring the same password reconfirmation as reset.
Exports include general settings without NAS name, access settings and allowed addresses,
power scheduling, widget polling, and desktop state. They exclude networking, hostname, account,
and token. Selected application sections are identified by their application IDs through
`GET /api/system/config?apps=<id>,<id>`. `config::EXPORTABLE` defines supported sections, and
`GET /api/system/config/apps` lists them. Terminal, notifications, and App Manager are currently
supported; App Manager includes its saved catalog and installed-application list. Unsupported
sections remain disabled. Backup rules are not stored there, and Explorer preferences are local
to the browser.

Import validates all sections using their normal save rules before writing. One transaction
replaces the configuration, then applies allowed addresses and scheduling. Desktop entries retain
folders and applications present or pending installation. With App Manager data, real package
mode installs missing catalog applications and removes unlisted optional applications while
retaining their data. Built-in/system applications and catalog entries with `preinstall` are
never removed. A missing catalog entry creates a notification. Newer export formats are rejected.
Version-1 exports without desktop/application sections preserve the current sections.

The browser writes the file through `src/apps/nas-system-settings/configFile.ts`, including the
gateway's `/auth/policy`. The optional file password is separate from the account password used
for confirmation. Without one, the file is readable JSON. With one, WebCrypto encrypts it using
PBKDF2 SHA-256 and AES-GCM without an added dependency. Import detects encryption and requests
the file password. Neither password reaches the launcher.

### Power scheduling

`power/schedule.rs` runs beside the server. Rules live in the database rather than systemd timers.
It converts rules to systemd calendar expressions, such as `Mon..Fri *-*-* 07:30:00`, and asks
`systemd-analyze calendar` for the next event. It waits for the exact second when the event is
within a minute, otherwise sleeps and checks again. `schedule::rules_changed()` interrupts the
wait so a newly saved near-future rule is not missed.

A powered-off machine cannot execute a power-on rule. Before shutdown, `power/wake.rs` records
the next wake time beside the database and starts root-only `nas-os-wake.service`, which writes
`/sys/class/rtc/rtc0/wakealarm`. Polkit grants only starting that unit. Failure still permits
shutdown and creates a notification explaining the missing wake alarm.

## Networking

`nas_system_settings/network/machine.rs` reads the current connection, scans available networks,
and connects through `NAS_NETWORK_MODE=real`. Read-only collection uses `ip -json`, sysfs,
NetworkManager's `nmcli`, or `/proc/net/wireless` where appropriate. Optional tools leave fields
empty instead of failing the whole read. The default route identifies the active connection.

`nmcli -t` separates fields with colons and escapes embedded colons with backslashes. SSIDs may
contain them, so parse escaped fields instead of splitting naively. Deduplicate repeated SSIDs,
keeping the strongest reading. Unavailable network data returns 503 and the UI offers retry.

## Effective settings

- Theme, locale, accent, and wallpaper apply through `useAppearance` and `@/i18n`. Accent is
  written to `--accent` and shared with applications.
- `createShortcut` is read during installation through `packages::run_install`,
  `desktop::shortcut_on_install`, and `desktop::add_app`. Updates preserve existing shortcuts and
  do not recreate removed ones. App Manager refreshes the list afterward.
- Widget refresh intervals drive `usePolling`.
- Power scheduling checks application tasks and active root units, including `nas-os-update`,
  `nas-os-apt@`, `nas-os-storage-*`, and `nas-os-app-packages@`. `schedule::wait_for_work` waits up
  to an hour for application tasks and indefinitely for privileged jobs. `power::run` returns
  409 during those jobs. `GET /system/power/busy` supplies the same list to manual confirmation.
- Networking reads and connections are real when enabled.
- Address restrictions are enforced by both the launcher middleware and, in real firewall mode,
  nftables protecting the NAS.
- `access::record_failure` reads `notifyFailures` and records the address, wrong field, and time.
- Notification delivery remains unimplemented. Webhook preferences may be stored, but the editor
  is disabled so configuration does not imply delivery.
- Debian updates are read from apt and installed only in real apt mode. See [System updates](#system-updates).

## Search-engine exclusion

NAS pages, including sign-in, must not be indexed. `index.html` declares
`noindex, nofollow, noarchive, nosnippet, noimageindex, notranslate` and `no-referrer`, avoiding
address disclosure through external Referer headers. `http/boundary.rs::no_index` adds equivalent
`X-Robots-Tag` directives to backend and proxied application responses. The gateway applies them
to static files as well. `public/robots.txt` also denies crawling with `Disallow: /`.

## Allowed addresses

`nas_system_settings/access.rs` stores exact addresses and CIDR ranges. An empty list adds no
restriction; loopback always passes. The wizard's base firewall still closes unspecified ports.

`http/guard.rs` covers requests sent directly to the launcher. The nftables layer covers the
rest of the NAS. The launcher writes the address list beside the database and starts
`nas-os-firewall.service`. Its root helper treats that file as untrusted input, accepts address
characters only, and atomically replaces the table. A rejected ruleset retains the old filter.
See [installation](system/packages/ui_installation/README.md).

Because the launcher listens only on `127.0.0.1`, normal gateway traffic appears as loopback.
The base firewall and nftables allowed-address table enforce restrictions at the network edge.

### NAS Finder discovery

The wizard and gateway answer NAS Finder UDP requests on port 39877. The wizard's base firewall
allows local IPv4 discovery only while `nas_discovery` contains that port. The wizard opens it
while configuring and empties it at handover. Security Center controls it afterward through
`GET`/`PUT /settings/discovery`, defaulting to off. `nas_system_settings/discovery.rs` writes `on`
to a `discovery` file beside the database, or removes it, then starts `nas-os-discovery.service`
through polkit rule 63. `apply-discovery.sh` fills or empties the fixed-port set.

At startup, enabled discovery is reapplied after the firewall resets. Disabled discovery does
nothing so it cannot close the wizard's active responder. `setup/complete` records `discoveryOff`
with instructions for enabling it. Outside real firewall mode, only the preference is saved.

## Application packages

Real installations require `NAS_PACKAGES_MODE=real`. Use separate program arguments, and register
long-running work as operations instead of holding an HTTP request open.

`/etc/nas-os/catalogue.json` records ID, name, forge, repository, and asset naming. Installation
writes it, only root edits it, and a missing file means an empty catalog. Requests reload the file
so edits take effect without restart.

`services/packages.rs` fetches the latest release, downloads the asset, compares its published
`.sha256`, extracts into a separate directory, and only then renames it over the old package.
Interrupted downloads preserve the working copy. The previous directory remains until replacement
succeeds. Application releases without checksums install with a logged warning.

The shared release token serves private application and launcher repositories. It is sent only
to configured forge origins, never unrelated download hosts (`forge::for_forge`).
`ui_configuration/install.sh` encrypts it in systemd's credential store; the UI reports only its
presence. Treat archives as untrusted. Tar rejects absolute and parent-traversal paths, but symbolic
links still require checking resolved paths. `nas_app_manager::inside` prevents package icons from
resolving outside the package, including links to the launcher database.

### Updating NAS components

`apps/services/nas_system_settings/updates.rs` checks the launcher, gateway, and wizard together.
The launcher uses `NAS_LAUNCHER_REPOSITORY`; the other components use `NAS_GATEWAY_REPOSITORY` and
`NAS_WIZARD_REPOSITORY`, defaulting to sibling `nas-gateway` and `nas-wizard` repositories under
the same owner. Installed versions come from `nas-os-gateway --version` and
`/usr/local/lib/nas-wizard/wizard/bin/nas-wizard --version`. An older gateway without `--version`
has an unknown version and can accept any available release.

One update action downloads every selected component, requires valid checksums, saves packages as
`<component>-update.tar` beside the database, and starts `nas-os-update.service`. A running service
cannot replace itself.

Root helper `install-launcher-update.sh` extracts packages, stops the launcher and any gateway
being updated, and creates one backup in `/var/lib/nas-os/backups/`. It includes databases,
`/etc/nas-os`, binaries, UI, helpers, units, polkit rules, and affected gateway/wizard files.
It runs each package's own installer, in wizard, gateway, launcher order. Wizard `install.sh --update`
reapplies OS configuration without launching setup. The gateway installer restarts it, followed
by `/health` verification. The launcher uses `ui_installation/install.sh`.

After launcher readiness, a changed launcher triggers a machine reboot; otherwise only the
launcher restarts. Any failure restores the complete backup, removes the new database WAL before
restoring the old database, and restarts the previous launcher/gateway without rebooting.
The update is atomic across components, and the last three backups are retained.

Factory reset first tries to download the latest wizard through `reset::prepare_wizard`.
`factory-reset.sh` installs it with `install.sh --update` before destructive work. If updating
fails, reset continues with the installed wizard.

`launcher-update.status` stores the outcome and updated component names. `updates::report_previous`
turns it into a notification and removes the file, distinguishing success, rollback, rejection,
and interrupted work. The newly installed launcher waits while `nas-os-update.service` remains
active, avoiding false interruption reports. Checks finding no updates create no notification.

`LauncherUpdateModal` confirms the reboot, stays open while work runs, follows download,
verification, and installation, and waits for the launcher to disappear and return. A new version
reloads the page; the previous version reports rollback.

The encrypted credential is `/etc/credstore.encrypted/nas-os.launcher-token`. systemd decrypts it
with the machine key and TPM when present, imports it using `ImportCredential=nas-os.*`, and
exposes it through `$CREDENTIALS_DIRECTORY` (`credentials::launcher`). Without real token mode,
development reads `NAS_LAUNCHER_TOKEN_PATH` instead.

## System updates

`apps/services/nas_system_settings/system_updates.rs` reads Debian updates with
`apt list --upgradable`, including during development, without privileges. Refreshing the package
index and installing selections require root. The launcher writes one selected package per line
to `apt-upgrade` beside the database, then starts `nas-os-apt@refresh.service` or
`nas-os-apt@upgrade.service`, running `apt-updates.sh`. Polkit permits only these instances and
`start`. Without `NAS_APT_MODE=real`, the UI explains why installation is unavailable.

The launcher accepts only currently offered packages. The root helper independently validates
Debian package-name syntax. `--only-upgrade` prevents new installations, and `--force-confold`
preserves manually edited configuration files.

Only one job runs at a time. `apps.apt_job` records it, responses expose `running`, and the UI
polls every three seconds while retaining a disabled list. Jobs also appear in the operation
register. Success creates a notification, and failures record their own diagnostics.

## Opening an external application

An application runs independently on a Unix socket. `http/proxy.rs` proxies `/apps/<id>/` on the
launcher's origin, strips the prefix, supplies `X-Forwarded-Prefix`, and connects to
`/run/nas-os/apps/<id>.sock`. Application-local redirects are rewritten under the prefix, and
`/apps/<id>` redirects to its trailing-slash form. No extra DNS, hostname, port, or client setup
is needed, including through a forwarded NAS port.

Shared origin requires trusted applications. Application JavaScript can otherwise act within the
shell's origin. Applications use relative URLs, such as Vite `base: './'` and `fetch('api/...')`;
absolute `/api/...` requests target the launcher instead.

Derive socket paths from the application ID, never a package-supplied socket path that could
point to another privileged Unix service. Manifest ID must match the directory ID. Socket
metadata mismatches are logged.

The proxy uses `tokio::net::UnixStream` and `hyper_util::rt::TokioIo`. It strips hop-by-hop headers
in both directions, writes a validated `X-NAS-User` from the gateway or falls back to `local`,
rejects `/internal/` with 403, forwards WebSocket upgrades through the socket, and returns 501
for other upgrades. Domain-scoped `Set-Cookie` responses are removed. The gateway strips the
session cookie from application requests and blocks attempts to replace it. In `http/routes.rs`,
the proxy sits outside API body limits/boundary handling but inside the outer address guard.

`resolveApp` creates an external `AppEntry` for unknown IDs. `AppHost` opens its `externalUrl`,
`/apps/<id>/`, without a misleading same-origin script-enabled iframe sandbox. Each application's
`frameGuard.ts` enforces no application-owned windows/dialogs and no top-level navigation, as
required by [APPS.md](docs/development/external_apps/APPS.md). Failed loads, including 503 from a
stopped process, display a start/retry action.

Development Vite proxies `/apps` like `/api`. Start the application manually on its expected
socket. Debug socket directories default to `backend/data/sockets` beside the database;
`NAS_APP_SOCKETS` overrides them. Unix socket paths must fit the platform's 108-byte limit.
End-to-end tests use `src/tests/e2e/fixtures/app.mjs`.

### Application processes and permissions

The launcher does not write units or gain root. `nas-app@.service` is the shared template, with
instances such as `nas-app@fileExplorer.service`. Polkit permits only `start`, `stop`, and
`restart` for valid application instances, never `enable`. `run-app.sh` reads `run.exec` from
the manifest and invokes it with `--listen <socket>`, rejecting absolute and escaping paths.

Applications use `DynamicUser=yes` and supplementary group `nas-os`. Their socket directory is
setgid so sockets inherit that group; `UMask=0007` allows launcher access. Sticky mode `3770`
prevents applications from replacing each other's socket. The launcher reapplies the mode at
startup.

Application units deny loopback access except the DNS resolver `127.0.0.53`. The launcher's own
loopback listener relies on gateway protection. Applications instead use
`/run/nas-os/launcher/apps.sock` (`NAS_LAUNCHER_SOCKET`, `http/app_channel.rs`) in a traversable,
non-writable directory, preventing socket-name takeover during launcher restart. The launcher
identifies the caller with `SO_PEERCRED` and `/proc/<pid>/cgroup`, requiring a matching
`nas-app@<id>.service` cgroup. Other callers receive 403.

`controllers/app_channel.rs` exposes only the application contract, including reads such as
`GET /api/system/inotify` for the user watch limit, notifications, and operation writes. Source
IDs are assigned to the authenticated application. Operations can be changed/deleted only by
their owning application.

Background applications declare `run.background` and report actual need through
`PUT /api/apps/self/background {needed}`. `services/background.rs` stores it in
`installed_apps.background`. Startup activates flagged applications after grants. Closing a frame
through `POST /apps/<id>/close` keeps needed background work running and stops other applications.
The `run:background` grant lowers priority through `Nice=10`, `CPUWeight=50`, and I/O
best-effort priority 7.

The sandbox hides data disks (`/srv`, `/mnt`, `/media`), logs, launcher state except the current
package, `/sys`, `/run/udev`, and other processes. `services/grants.rs` accepts only predefined
`permissions.paths`, including `/srv/storage`, read-only `/proc`, `/sys`, and `/run/udev`.
Unsupported requests reject installation without leaving a package. Valid requests become
`app-grants/<id>` beside the database. `nas-os-app-grants@<id>.service`, polkit rule 55, runs
`apply-app-grants.sh` as root to write `nas-app@<id>.service.d/grants.conf`, revalidating the same
allowlist. Apply grants before replacement, remove them on uninstall, and reconcile them at
startup. Running applications whose grants changed are restarted.

`permissions.network` supports `smtp` and `sync`. `network:<type>` grants add the application
cgroup to base-firewall sets `nas_apps_smtp` for ports 25, 465, and 587, or `nas_apps_sync` for
LAN Syncthing traffic. A sync-capable application reports active work through
`PUT /api/apps/self/sync`. `services/sync_ports.rs` writes `sync-ports/<id>` and starts
`nas-os-sync-ports.service` through polkit rule 62. `apply-sync-ports.sh` fills or empties `nas_sync`.
Startup and sync-app installation reconcile it; uninstall closes the grant. Reports are serialized
and reapplied even unchanged to retry failed helpers. Sync renews them every 30 seconds. LAN
traffic is limited to TCP 22000 and UDP 22000/21027.

`permissions.packages` uses the allowlist in `services/system_packages.rs`, including `msmtp`,
`7zip`, `samba`, and `syncthing`. Requests in `app-packages/<id>` start
`nas-os-app-packages@<id>.service` through polkit rule 56. `app-packages.sh` installs prerequisites
before package replacement and removes only its own additions that no other application needs.

`services/mail_events.rs` delivers refused sign-ins, launcher updates, and system-update events
to the `mail-events` provider, currently nas-mail-client. It starts the application when needed
and posts to its internal Unix-socket endpoint. While that application is installed, every six
hours it checks launcher and apt updates, categorizing system packages by source archive as
`security`, `stable`, or `other`.

`services/cleanup.rs` handles `POST /api/apps/self/cleanup` only for `permissions.cleanup`
applications. It validates action, allowlisted target, temporary-file age, and journal size,
writes `cleanup.request.json`, and starts `nas-os-cleanup.service` through polkit rule 59.
`nas-os-axum --cleanup-job` in `services/cleanup/executor.rs` revalidates and uses fixed paths and
commands as root, writing `cleanup.result.json`. Only one cleanup runs at a time. Outside real
app mode it returns 503. Every target supports a non-destructive preview; execution measures
before/after size. It avoids symlinks and filesystem traversal, preserves the latest update
backup, current/previous/newer kernels, and `/tmp` files used by running programs.

`services/caches.rs` aggregates `cache` providers through internal Unix-socket endpoints, starting
stopped providers when required. It adds antivirus scan-history size from the root job's report,
because that directory is private. `forget_scanned` clears it. `controllers/caches.rs` exposes
`GET /api/caches` and `DELETE /api/caches/{app}/{id}`.

`controllers/mail.rs` provides `GET /api/mail/status` for Account settings and
`POST /internal/mail/code` outside `/api` for gateway recovery/verification emails. The gateway
calls it over loopback and never forwards it from the browser. Username, password, and recovery
email live in the gateway under `/auth/account` and `/auth/recovery`. Account settings and
`usePasswordRecovery.ts` use `src/api/account.ts`. Email recovery is off by default and toggled
through `PUT /auth/account/recovery`. Sign-in shows password recovery only when
`GET /auth/recovery` reports it enabled with a verified address.

Explorer and Backup request `/srv/storage`. They receive group `nas-storage`, appropriate
`BindPaths`, and `/etc/nas-os/storage-volumes.tsv` through `NAS_STORAGE_ROOT` and
`NAS_STORAGE_REGISTRY`. Root `nas-os-storage.service`, triggered by timer/udev, excludes system
filesystems, mounts data partitions by UUID, and publishes only `.nas-users/<wizard-user>` through
`/srv/storage/v-<uuid>` bind mounts. The backend validates the map and active mounts before serving
requests. Search, file pickers, and shortcuts share these logical paths. Terminal sessions do not
yet use this directory as their session root.

Launcher/application units have no capabilities and restrict clock, kernel logs, hostname, and
socket families. `systemd-analyze security --offline=yes` can inspect them without installation.
Recorded scores were 1.7/10 for the launcher, 1.5 for applications, and 1.1 for grant application;
these are measurements rather than a security guarantee.

Lifecycle management requires `NAS_APPS_MODE=real`. Otherwise operations return 503 naming the
switch and the UI hides management controls using `appsManaged` from `/apps/credentials`.
Installation restarts the application at the end; uninstall stops it before removing files.

### Frame messaging

Applications use their own relative API paths. The intended shell/application communication
channel is `postMessage` through `composables/useAppBridge.ts`. Every message includes `nas: 1`.
Outgoing messages target the application's explicit origin, never `*`. Incoming messages must
match both an open application's origin and its frame window, preventing one frame from speaking
for another.

The shell sends resolved theme, accent, and locale when a frame loads, reports `ready`, or those
values change. Applications can request notifications, notification descriptions (`describe`),
and opening installed applications at specific locations, such as `?notification=12`.
Notifications are attributed to the requesting frame. Only nas-log-manager may request warning/error
notifications (`diagnostics`), detailed logs (`diagnostic-log`), or deletion (`remove-diagnostic`).
The shell uses its console session and replies with `<request>-result`, avoiding direct application
access to the launcher API.

## Ongoing operations

`widgets/services/tasks.rs` is an operation register, not a scheduler. Applications register work,
report progress, and finish it. The widget separates Ongoing and Completed tabs, newest-created
first. Pausing records state that the owner observes and honors. Applications declare whether
pause is supported; unsupported controls are hidden. Unknown progress produces an indeterminate
bar. Finished operations cannot reopen, and failures also create notifications. Package installs
and updates use the same register.

## Machine monitoring

nas-system-monitor collects CPU, memory, processes, and sensors from `/proc` and `/sys`, exposing
`api/telemetry` to its application/widget. The launcher holds no hardware samples/types. Its
legacy `/api/system/telemetry` proxies to the provider for compatibility.

`services/process_control.rs` and its root helper retain privileged process operations, validating
PID, start time, and essential-service protections. Terminating a process in `nas-app@<id>.service`
stops the entire unit and children and verifies shutdown. The backend identifies the application
through its cgroup. The shared app/widget bridge closes that tab and unmounts its iframe through
the normal close path without sending a second stop request. Failures retain the tab. Restarting
or terminating a non-application process does not close it. `MonitorDrawer.vue` only hosts the
widget iframe, without rendering hardware data or owning its preferences.

## State and requests

Page-level resource failures cannot depend on the unavailable backend's notification register.
`MainPage` displays a retry toast through `NotificationToasts` and removes it when the error clears.

`request<T>()` wraps fetch with a ten-second timeout and emits `nas:mutation` after successful
writes. The shell uses this to refresh notification summaries without owning notification-app
logic. Automatic reads use `usePolling`, which suspends work while hidden and refreshes
immediately on visibility return. Every 15 seconds the header reads only unread notifications.
The notification manager alone reads full history, up to 500 rows.

Applications use `useResource` for loading, errors, retry, and actions. Stale responses are
ignored, previously loaded data remains visible after failures, and failed requests never invent
successful results.

The application menu, App Manager, and desktop share `useContextMenu`, which stores position and
selection. The action reflects the current state, offering add-to-desktop for missing shortcuts
and shortcut removal for existing ones.

`appsStore` owns the application list and open tab IDs. `AppHost` retains mounted views with
`v-show` across tab changes and unmounts them on close. Modals are independent of the active tab.
General settings use their domain store; forms edit a draft and apply only the saved response.

## Persistence

Only the launcher opens its SQLite `launcher.db`; the gateway owns its separate database.
Do not duplicate system-owned state such as SSH, journal logs, or disk contents.

| Data                                    | Storage                                       |
| --------------------------------------- | --------------------------------------------- |
| Notifications with old-entry cleanup    | SQLite                                        |
| General UI and notification preferences | SQLite JSON per user and section              |
| Desktop shortcuts and positions         | SQLite                                        |
| Power rules                             | SQLite section document                       |
| `notify_failures`                       | SQLite                                        |
| Allowed addresses                       | SQLite plus generated nftables set            |
| SSH enablement/port                     | Managed from the machine console              |
| Installed applications                  | SQLite registry and `apps/` package directory |
| Installable catalog                     | `/etc/nas-os/catalogue.json`                  |
| Release token                           | Encrypted systemd credential                  |
| Logs                                    | journald                                      |
| Process metrics and temperatures        | Read on demand from `/proc` and `/sys`        |
| Disks and updates                       | Read on demand from the system                |
| Operation history                       | SQLite, 200 rows and one day of history       |
| Account, Argon2id hashes, sessions      | Gateway SQLite database `gateway.db`          |

### Database and environment configuration

`services/database.rs` opens the database at startup. Failure prevents startup and is logged.
Path precedence is `NAS_DB_PATH`, then `$STATE_DIRECTORY/launcher.db` from
`StateDirectory=nas-os` (production `/var/lib/nas-os/launcher.db`), then debug-only
`backend/data/launcher.db`. Release builds require an explicit/systemd path.

End-to-end scenarios share `backend/data/e2e.db`, configured in `playwright.config.ts`. Independent
scenarios run in parallel. Scenarios changing shared locale, general settings, catalog, or task
history are listed in `SHARED` and run afterward, sequentially. Add new shared-state writers there.

SQLite uses WAL, `synchronous=FULL`, foreign keys, and a five-second busy timeout. Confirmed
transactions survive power loss. Database directories/files are created as `0700`/`0600` before
opening; WAL/SHM inherit file permissions. Ctrl+C or systemd SIGTERM closes the pool and checkpoints
the WAL. `/health` returns 503 when the database is unavailable. The service state directory uses
`0710` to permit group traversal to application packages without listing launcher state;
`ProtectSystem=strict` and `UMask=0077` remain enabled.

`config/production.env` is packaged as `files/nas-os.env` and installed as
`/etc/nas-os/nas-os.env` on every install/update. Relevant units read it and then the optional,
administrator-owned `/etc/nas-os/nas-os.local.env`, which updates preserve. Development scripts
source `config/development.env` with machine-changing switches disabled and explicit environment
overrides preserved, such as `PORT=3001 pnpm run backend:dev`. Add new variables to both files.
End-to-end tests supply their own environment instead.

| Switch                    | Behavior                          | Additional NAS requirements                                               |
| ------------------------- | --------------------------------- | ------------------------------------------------------------------------- |
| `NAS_POWER_MODE=real`     | Restart, shutdown, scheduled wake | Polkit 10, `nas-os-wake`, firmware RTC wake                               |
| `NAS_NETWORK_MODE=real`   | Scan/connect networking           | NetworkManager, polkit 20                                                 |
| `NAS_FIREWALL_MODE=real`  | Allowed-address nftables filter   | Wizard nftables, `nas-os-firewall`, polkit 30                             |
| `NAS_PACKAGES_MODE=real`  | Application/NAS packages          | Forge access, `nas-os-update`, polkit 40                                  |
| `NAS_APPS_MODE=real`      | Application lifecycle             | `nas-app@.service`, polkit 50                                             |
| `NAS_APT_MODE=real`       | Debian updates                    | `nas-os-apt@.service`, polkit 60                                          |
| `NAS_TERMINAL_MODE=real`  | Terminal console/journal          | Installed terminal support                                                |
| `NAS_TOKEN_MODE=real`     | Encrypted systemd credentials     | `ui_configuration/install.sh`; development uses `NAS_LAUNCHER_TOKEN_PATH` |
| `NAS_RESET_MODE=real`     | Reset and restart wizard          | `nas-os-reset`, polkit 75, installed wizard                               |
| `NAS_PROCESSES_MODE=real` | Terminate/restart processes       | `nas-os-process@.service`, polkit 65                                      |
| `NAS_LAUNCHER_REPOSITORY` | Launcher release repository       | `owner/name` identifier                                                   |

Database, WAL, and SHM are one live database. Never copy the `.db` alone while running. Use
`VACUUM INTO '/path/copy.db'`. `database::keep_copy` creates `copies/launcher.db` at startup and
daily, writing separately and renaming atomically. Back up that consistent private copy.

### Section settings

`settings` stores one JSON document per section, including `general`, `notifications`, `access`,
and `power`, with display order and last-save time. Save all validated values together; rejected
input leaves neither partial settings nor a success notification.

`services/settings.rs` knows section names/order but not document contents. The owning domain
provides types and validation. Settings owns general/access/power, notification management owns
delivery settings, `services/terminal.rs` owns console appearance, and App Manager owns automatic
update preferences and the UI-saved catalog. A saved catalog overrides the file catalog. Domains
do not import each other's types.

Read another section through a minimal partial type when necessary, as `desktop.rs` does for
`createShortcut`. Unsaved sections return defaults. Unreadable stored JSON falls back to defaults
and logs an error, preserving UI availability. Settings still use the `local` user rather than
`X-NAS-User`. The access section contains only `notifyFailures`.

### Migrations and repository rules

See [migration rules](backend/axum/migrations/README.md). Numbered, embedded migrations run at
startup. Never edit an applied migration because checksum mismatch prevents startup. Add forward
migrations and back up before updating. Use `STRICT` tables, `CHECK` constraints, and integer
millisecond timestamps. `build.rs` rebuilds when migrations change.

- Keep SQL in repository code. Services call repository methods instead of assembling queries.
- Bind query parameters. Never interpolate user input into SQL.
- Runtime-checked `query_as` is currently used; `query!` requires a database or `.sqlx` cache in CI.
- Extract only required state, such as `State<SqlitePool>` or `State<Session>`.
- Use `ValidJson`, preserving serde explanations as typed errors. Plain `Json` errors would be
  reduced by the API boundary to a generic invalid-request message.
- Use Rust enums for fixed choices, including theme, language, background mode/pattern, power
  day/action, and notification tone. Manually validate ranges, colors, times, and known filenames.
- `deny_unknown_fields` protects API input, but removing stored JSON fields requires a migration,
  such as `json_remove`, so existing documents remain readable.
- SQLite `LIKE` is case-insensitive only for ASCII. Use Rust comparison or FTS5 where non-ASCII
  search requires case-insensitive behavior.

Inspect development state with SQLite tools.

```bash
sudo apt install sqlite3
sqlite3 backend/data/launcher.db
sqlite> .tables
sqlite> .schema
sqlite> SELECT version, description, success FROM _sqlx_migrations;
```

Stop the development backend before deleting `backend/data/` to start fresh.

## Styling

Use Vue Composition API, TypeScript, and `_` in compound CSS names. `_variables.scss` holds tokens,
`theme.scss` semantic colors, and `_placeholders.scss`/`_mixins.scss` shared primitives.
`vite.config.common.ts` injects `global.scss` for development, build, and tests.

`src/apps/styles/_apps.scss` is imported once by `main.scss`. `.app_surface` scopes shared styles
to application/widget/modal content. Components add domain layout rather than duplicating controls.
Use `%button`, `%control`, `%panel`, `%stack`, `%icon_button`, and `focus-ring` without another CSS
framework.

PostCSS merges duplicate selectors, uses PurgeCSS for unused classes, applies browser prefixes
through autoprefixer, and minifies with cssnano. `package.json` owns the browserslist read by both
tools. It matches the configured Vite baseline, including Chrome/Edge 111, Firefox 114, and
Safari 16.4, so minification preserves required prefixes. Runtime-composed class prefixes are
explicitly safelisted and checked by the build tests.

Saved theme is `light`, `dark`, or `system`. System mode follows operating-system changes live.
Applications/widgets inherit the root `data-theme`; sign-in retains its own layout. Logos have
application-specific palettes. `CommonIcon` preserves SVG color, accessible labeling, sizing, and
decorative behavior. Fonts and icons are local rather than fetched from third parties.

### SFC formatting

`.vue` files are excluded from direct CLI Prettier formatting. The
[SFC formatter](scripts/format-sfc-spacing.mjs) calls Prettier's API and then adds blank lines just
inside script/style blocks and between adjacent CSS rules. `pnpm run format` runs Prettier,
ESLint fixes, and this formatter in order. `pnpm run format:sfc` processes only SFCs;
`pnpm run format:check` validates both formatting paths without writing. VS Code's Prettier
extension does not format `.vue` directly; use the project formatter.

## Initial setup

The separate root-run wizard collects account, NAS name, locale, theme, networking, and token.
It installs gateway and launcher, runs `ui_configuration` to store the token/create the terminal
user/reconcile data volumes, creates the gateway account, then calls
`POST /api/setup/complete` over loopback without the gateway, supplying language, theme, and
optional `nasName`.

The launcher validates/saves preferences, applies the hostname through `apply_hostname` when
`NAS_HOSTNAME_MODE=real`, records `setup`, and schedules catalog entries with `preinstall: true`
through `services/provisioning.rs`. `setup_steps` persists progress, and startup resumes incomplete
steps. Repeated setup completion returns 409.

`packages::install_for_setup` records operations, logs, and step outcomes without normal package
success/failure notifications. Preparation shows failures instead. User-requested App Manager
installations retain their normal notifications.

Until configuration and provisioning finish, the router opens `/preparing` before every page,
including sign-in. `PreparingView.vue` polls `GET /api/setup` every two seconds. Successful steps
lead to sign-in; failures show details and an explicit sign-in button. An unconfigured NAS remains
waiting for the wizard. The gateway permits public setup reads but not setup completion.

## Gateway integration

The browser reaches the separate gateway on HTTP 80 or HTTPS 443. The launcher remains on
`127.0.0.1:3000`. Gateway `src/entry.rs` forwards `/apps/*` only with a valid session, returning
401 otherwise, without exposing the session cookie. `/api/*` similarly requires a session, except
explicit public setup/failure-report routes. A 401 returns the UI to sign-in.

Other paths serve `GATEWAY_UI_DIR`, either a real file or `index.html` for extensionless routes.
Hashed `assets/` files cache for a year, while the index is revalidated so updates appear on the
next load. `frame-ancestors 'none'` prevents framing the shell. Graceful shutdown stops new
connections and finishes in-flight responses, including power-action acknowledgments. The proxy
uses hyper. In development, the gateway runs on `127.0.0.1:3003` without a UI directory and Vite
proxies `/auth` to it.

The gateway exclusively owns accounts and sessions in its own SQLite database. Its password
hashes use Argon2id, including dummy-hash comparison for unknown usernames. Invalid credentials
return 401; the configured per-address lockout returns 429 after repeated failures.
`/auth/policy` stores enabled state, attempts, and duration. Session tokens are random, stored
as SHA-256 hashes, and sent in the opaque HttpOnly `nas_session` cookie. Sessions expire after
30 inactive minutes or 12 hours and survive gateway restart.

The launcher never handles account passwords. The wizard creates the account through
`nas-os-gateway account create`; without an account every sign-in is refused. Navigation validates
`/auth/session` before opening the console. Logout uses `src/api/gateway.ts` and the build-time
`NAS_GATEWAY_LOGOUT` value from Vite, defaulting to `/auth/logout`.

Production HTTPS and optional mTLS are implemented by the gateway. Device certificates are
managed in Security Center after password sign-in. Plain HTTP development does not represent the
production TLS handshake. Network restrictions still rely on the wizard's base firewall and the
allowed-address nftables filter.

## Language

Use `t` from `@/i18n` for UI text and `formatDate` for dates. Locale changes update `html.lang`.
Do not use an em dash; repository checks cover this rule. Hardcoded operational text, script
messages, and documentation are in English. Italian belongs in the Italian localization catalogs.

Catalogs mirror domain boundaries, with shell text in `i18n/launcher/` and application text in
`i18n/apps/<app>/`. Shared keys remain in the launcher catalog. `index.ts` merges catalogs into a
flat language map, so callers do not need to know which file owns a key. Matching languages must
have identical keys, and no key may be defined twice; `src/tests/i18n/` checks both rules.
Backend-supplied process names, logs, notification content, and paths are data rather than
translation keys. Sign-in uses the same catalogs.

## Tests and quality

Unit tests cover plain modules/stores; component tests mount SFCs. `src/tests/scripts/` runs real
Node processes against fake `nft`, `systemctl`, clocks, and other system commands placed first in
`PATH`. Paths mirror the tested modules, and Vue probes live under `src/tests/fixtures/`.
Production compilation excludes tests, which Sonar treats separately from covered sources.
Frontend and backend test folders contain no source comments.

```bash
pnpm run test:unit
pnpm run test:component
pnpm run test:coverage
pnpm run build
pnpm run lint
pnpm run format:check
pnpm run test:e2e
```

Install Rust coverage tools once in the development environment.

```bash
cargo install cargo-llvm-cov --locked
rustup component add llvm-tools-preview
pnpm run coverage:rust
```

Coverage is written to `backend/coverage/lcov.info`. `test:sonar` uses the same command;
`pnpm install` does not install Cargo tools. Rootless Podman scanning uses
`--userns=keep-id:uid=1000,gid=1000` so the scanner can read private files and write reports without
changing project ownership or permissions.

`playwright-bdd` generates end-to-end scenarios from `src/tests/e2e/features` and `steps`. Shared
steps in `steps/base_page.ts` select `data-testid`, supporting navigation, click, fill, visibility,
text, and URL checks. Add custom steps only when those primitives are insufficient. Production
strips test IDs; development retains them.

Integrated scenarios start backend and Vite in `e2e` mode without file watching, avoiding inotify
exhaustion, and use a fresh `backend/data/e2e.db`. The gateway is replaced by
`src/tests/e2e/fixtures/gateway.mjs` with one test account and the login/session/logout/policy
contract. The real gateway's tests belong to its own repository.

Rust module tests live under `backend/axum/tests/<matching-path>.rs`, included with `#[cfg(test)]`
and `#[path = "..."]` to access private functions. External router tests are explicit `[[test]]`
targets. No test implementation belongs inside production Rust files.

`backend/axum/tests/machine/` is the separate `machine` integration target. It runs the launcher in
its own temporary environment with real mode switches except power. Privileged commands such as
`systemctl`, `lsblk`, `smartctl`, `apt`, and `nmcli` are fake executables from `support.rs`, verified
at startup to precede real commands. They record calls and use fixture answers. Local HTTP
servers imitate GitHub/GitLab and a Unix server imitates Email. This exercises package/update/root
unit/API/binary behavior without changing the host. Real disk helpers `reconcile`,
`initialize_with_log`, `rename_reported`, and `eject_reported` are never executed.
`NAS_STORAGE_STATE_DIR` redirects unit requests from their default `/var/lib/nas-os` location.

The generated [OpenAPI schema](docs/development/openapi.json) is the API contract. Handler
`#[utoipa::path]` attributes define paths, methods, and bodies, which the router reuses.
`pnpm run api:generate` updates the schema and `src/types/api.ts` through openapi-typescript.
Rust tests, unit tests, and `pnpm run api:check` in CI reject stale generated output. Keep frontend,
Cargo package, lockfiles, schema metadata, and Sonar versions aligned.

Reuse existing dependencies. See the [security review](docs/development/SECURITY_REVIEW.md).

## Releases

Verify releases locally; the release build pipeline does not run the test suite. Pull requests
to `main`, including Dependabot updates, run unit, component, Rust, and end-to-end checks through
`.github/workflows/ci.yml`. A branch ruleset can require these checks before merge.

- `pnpm run release:test` synchronizes the npm lockfile, regenerates OpenAPI/types, formats,
  checks formatting/lint/Clippy, runs all test suites, and builds UI, release binary, and package.
- `pnpm run release:merge -- --type=patch|minor|major --branch=main` chooses the version, runs the
  same checks, updates `package.json`, `backend/axum/Cargo.toml`, and `sonar-project.properties`,
  commits, merges, tags, and pushes. `--dry-run` only reports the proposed version.
- Tags trigger `.github/workflows/build.yml` to validate version/tag consistency, compile, build
  user manual/package, publish archive and `.sha256`, and retain two releases. Notes come from the
  matching `CHANGELOG.txt` section or commits since the previous tag.

`backend/Cargo.toml` enables LTO, one codegen unit, and symbol stripping for release builds.
A previous measurement found a roughly 40% smaller binary. The release archive is
`nas-os-<version>-linux-x64.tar.gz`, the name expected by wizard and launcher updater. Its root
contains `nas_os_user_manual_<version>.pdf`; the wizard copies it beside its installation manual.

`pnpm run manual:build` uses md-to-pdf with `PUPPETEER_EXECUTABLE_PATH`, otherwise installed Brave,
Chromium, or Chrome, falling back to Puppeteer's browser. The GitHub pipeline installs the matching
Puppeteer Chrome, and `CI` launches it without browser sandboxing. Fenced Bash blocks use the
highlighter already included in md-to-pdf, without a direct undeclared dependency.

## Optional antivirus

`apps/services/antivirus/` implements the Security Center ClamAV client, persistent settings,
serialized operations, scheduling, and privileged executor. Routes include `GET /api/antivirus`,
`PUT /api/antivirus/settings`, `POST /api/antivirus/enabled`, `POST /api/antivirus/scan`,
`POST /api/antivirus/quarantine/{quarantine|restore|delete}`, and
`POST /api/antivirus/scanned/forget`. Actions acknowledge accepted work with 204. Status includes
phase, apt progress, latest result, and quarantine.

Quick scans and custom `skipUnchanged` scans use `antivirus/scan_cache.rs`. Clean-file records in
`/var/lib/nas-os-antivirus/scan-cache.sqlite3` include path, device, inode, size, mtime, ctime, and
scan options for archives, PUA, and size limit. Traversal avoids symlinks and applies ClamAV
exclusions. Candidates use `clamscan --file-list`, with newline-containing names passed as separate
arguments. An OK result is cached only when ctime is not newer than scan start. FOUND removes a
cached entry, and results expire after 30 days. Cache failures fall back to full scanning. Reports
include `skipped` and `planned`, allowing real progress percentages; `scanned/forget` clears
history. The iframe uses `src/api/antivirus.ts` through the Security Center bridge.

The external application's manifest requires no ClamAV packages. Enabling antivirus alone asks
`system_packages` for `clamav` and `clamav-freshclam`. Apt reports actual Status-Fd progress in
`/var/lib/nas-os-app-packages/antivirus-install.progress`. Root `nas-os-antivirus.service` runs
`nas-os-axum --antivirus-job` against `/var/lib/nas-os/antivirus.request.json`. Polkit permits only
starting it. Installation does not enable it, and launcher startup does not install packages.
The helper refreshes signatures and manages FreshClam only after enablement.

Public results are stored in `/var/lib/nas-os/antivirus.result.json`. Root-private quarantine
metadata/content live in `/var/lib/nas-os-antivirus`. User filenames never become commands or
quarantine destinations. Report IDs identify files, which are rechecked by ClamAV and opened
without following links. Directory file descriptors anchor operations against path replacement.
Restore uses `create_new` and never overwrites existing files. Disabling retains signatures and
quarantined content, stops FreshClam, and prevents new jobs. Startup reconciles the service with
stored preferences and waits for running operations. Reset stops antivirus/FreshClam before
removing launcher state; reset/uninstall preserve quarantined content.

### Antivirus lifecycle and coverage

`bootScan` and `shutdownScan` default to false and remain compatible with older settings. The
executor stores a validated private settings copy, enabling the lifecycle unit only when needed.
Kernel boot ID distinguishes OS startup from feature activation. Shutdown checks
`systemctl is-system-running`; stopping the unit for an update does not scan. The launcher stops
before the final scan, filesystems/network afterward. A shared flock serializes operational and
lifecycle units. Results persist for notification at next launcher startup. Factory reset and
uninstall disable lifecycle scanning while preserving quarantine.

Antivirus tests run with the regular Rust suite, including simulated machine scenarios.
`pnpm run coverage:rust` generates the backend LCOV report. `pnpm run test:coverage` collects
frontend coverage, including Security Center. `pnpm run test:sonar` generates both reports,
collects Clippy output, and invokes the scanner. These commands are declared directly in
`package.json`, without standalone coverage or Sonar test scripts. Machine tests require local
sockets. Tests never install or execute real ClamAV.
