# nas-os

A web desktop for a custom Debian 13 NAS, built with Vue 3, TypeScript, Pinia, and a Rust Axum
backend. The UI includes an application menu, multiple tabs, desktop shortcuts, notifications,
and two drawers for system monitoring and ongoing operations. System applications remain
separate from the shell.

## Current capabilities

Machine-changing operations each require their own switch in `/etc/nas-os/nas-os.env`.
`config/development.env` keeps them disabled during development.

- **Power.** Restart and shutdown use `systemctl`. The backend schedules power rules and sets a
  hardware wake alarm before scheduled shutdown (`NAS_POWER_MODE=real`). Rules wait up to an hour
  for application tasks. Privileged updates, package jobs, and disk operations always block power
  actions, including manual requests, which return 409 while these jobs run.
- **Networking.** NetworkManager provides connection status, wireless scanning, and connections.
  Only one connection is active at a time. Wired connectivity takes precedence over Wi-Fi, and
  adapters can be disabled except for the one in use (`NAS_NETWORK_MODE=real`).
- **Allowed addresses.** Addresses and CIDR ranges are enforced by the launcher and an nftables
  set protecting the whole NAS (`NAS_FIREWALL_MODE=real`).
- **Application packages.** A file-based catalog and optional bundled archives under `system/apps`
  support GitHub/GitLab releases, downloads, checksum verification, extraction, and updates
  (`NAS_PACKAGES_MODE=real`).
- **NAS updates.** A root service backs up databases and configuration, installs downloaded
  launcher, gateway, and wizard updates, checks the new launcher, and restarts the NAS when
  required. Failures restore the backup. The private-release token is an encrypted systemd
  credential that the interface never displays.
- **System packages.** Available Debian updates are read from apt. A root service refreshes the
  index and installs selected updates (`NAS_APT_MODE=real`).
- **Terminal.** The external `nas-system-terminal` application uses launcher services for a console
  as the local user created by the wizard, without `sudo`, and for following the system journal
  (`NAS_TERMINAL_MODE=real`).
- **Configuration.** Settings can be exported and imported after gateway-verified account-password
  confirmation. An optional, separate file password encrypts exports in the browser.
- **Reset.** After gateway password confirmation, a root service removes launcher, gateway, and
  application state, the terminal user, and the release token, then restarts the wizard. Data-disk
  files are preserved (`NAS_RESET_MODE=real`).
- **Telemetry.** The external nas-system-monitor backend collects CPU, memory, swap, load,
  temperature, processes, and sensors and provides its own monitoring widget.

SQLite preserves section settings, installed application versions, desktop shortcuts,
notifications, allowed addresses, and the operation register across restarts. The development
file is `backend/data/launcher.db`. Accounts and sessions belong to the separate
[gateway](../nas-gateway/README.md) database.

The interface retains application state when switching tabs. System applications appear during
initial provisioning. Desktop shortcuts support context menus, including Shift+F10. Light, dark,
and system themes offer selectable accents and six local backgrounds, with readable desktop
labels. Italian and English catalogs, live notification filters, notification toasts, and header
drawers expose monitoring and ongoing/completed operations with progress, pause, and stop controls.

External applications are installed and updated from GitHub or GitLab releases. Each runs in its
own process on a Unix socket. The launcher reverse-proxies it under `/apps/<id>/` and opens it in
an iframe on the NAS origin, without additional DNS or client-network configuration. One systemd
template, `nas-app@<id>.service`, manages all applications. Shell and frame communicate through
`postMessage`, carrying theme, accent, language, notifications, application-opening requests, and
notification reads. Notifications link to detailed logs in nas-log-manager. See the full
[application contract](docs/development/external_apps/APPS.md).

On the NAS, the browser reaches the gateway over HTTP or HTTPS 443. The gateway serves the UI,
authenticates the wizard-created account, and forwards signed-in requests to the launcher and
applications with the username in `X-NAS-User`. The launcher is an unprivileged service listening
only on `127.0.0.1`. Dedicated root units perform privileged operations, authorized by individual
polkit rules. Every root helper revalidates its input. See the
[security review](docs/development/SECURITY_REVIEW.md).

Installation enables a firewall denying unspecified inbound and outbound traffic. Private-network
SSH remains available. The terminal user is distinct from both `nas-os` and the Debian
administrator `nas`. Storage services discover and mount supported data partitions without
formatting them. nas-file-manager exposes only the wizard user's dedicated area, excluding system
partitions and pre-existing files outside that area. See the
[installation guide](system/packages/ui_installation/README.md).

The gateway provides a NAS CA and server certificate. Install the authority from `/auth/ca.crt`
to trust HTTPS. Optional device-certificate access, mTLS, is configured after password sign-in
through Security Center. Create and import a device certificate, open the NAS over HTTPS, and
enable mTLS after password confirmation. Each device certificate can be renewed or revoked
individually. Activation requires a valid certificate on the current connection.

Notification delivery through email/webhooks remains incomplete and is presented as unavailable.
File search is provided by external search-capable applications, such as nas-file-manager, which
must be installed and running. See the [roadmap](ROADMAP.md).

## Development

Use Node `^22.18.0 || >=24.12.0`, pnpm, and stable Rust. Local installation uses pnpm; pipelines
use `npm ci` with `package-lock.json`. `pnpm install` synchronizes the npm lockfile through its
postinstall hook, which does nothing under npm. After `pnpm add`, `update`, or `remove`, run
`pnpm run lock:npm`. Lint fails if the manifest and npm lockfile disagree.

```bash
pnpm install
pnpm run backend:dev
```

In a second terminal, start the separate gateway. Its development account/session database is
`../nas-gateway/data/gateway.db`.

```bash
cargo run --manifest-path ../nas-gateway/Cargo.toml
```

In a third terminal, start the UI.

```bash
pnpm run dev
```

Vite proxies `/api` to `http://127.0.0.1:3000` and `/auth` to the gateway on
`127.0.0.1:3003`. The backend listens only on loopback. Override ports with
`PORT=3001 pnpm run backend:dev` and `NAS_API_TARGET=http://127.0.0.1:3001 pnpm run dev`.

## NAS installation

The [wizard](../nas-wizard/README.md), released as `ValerioGc/nas-wizard`, installs this package.
Download its release, extract it on Debian 13, and run `sudo ./install.sh`. It prepares the OS
and starts browser setup for account, language, theme, networking, and token, then downloads and
installs the latest gateway and launcher.

Each launcher release publishes `nas-os-<version>-linux-x64.tar.gz` and its `.sha256`. The package
contains `ui_installation` for the launcher, UI, helpers, units, and polkit rules, and
`ui_configuration` for the token, terminal user, and data volumes. Both the wizard and launcher
updater expect this archive name.

The machine retains `/usr/local/lib/nas-os/check.sh`, `configure.sh`, and `uninstall.sh` for
verification, token configuration, and launcher removal. Full NAS removal uses
`sudo ./uninstall.sh`, the single script left in the extracted wizard package.

See [installed files](system/packages/ui_installation/README.md). The wizard owns the complete
installation manual.

## Checks

`pnpm run release:test` runs all checks and builds the release package. Individual checks are
listed below.

```bash
pnpm run build
pnpm run test:unit
pnpm run test:component
pnpm run test:e2e
pnpm run format:check
pnpm run lint
cargo fmt --manifest-path backend/Cargo.toml --check
cargo clippy --manifest-path backend/Cargo.toml --all-targets -- -D warnings
cargo test --manifest-path backend/Cargo.toml
```

Frontend tests mirror source paths under `src/tests/`. Installation/helper tests in
`src/tests/scripts/` use real Node processes and mocked system commands. Rust tests mirror their
modules under `backend/axum/tests/`.

## Documentation

- [Development and conventions](DEVELOPMENT.md)
- [External applications](docs/development/external_apps/APPS.md)
- [API guide](docs/development/API.md) and [OpenAPI schema](docs/development/openapi.json)
- [Installed NAS files](system/packages/ui_installation/README.md)
- [Release-token configuration](system/packages/ui_configuration/README.md)
- [Gateway](../nas-gateway/README.md) and [wizard](../nas-wizard/README.md)
- [User manual](docs/user_manual), included as a PDF in releases
- [Roadmap](ROADMAP.md)
- [Implementation and verification log](docs/development/IMPLEMENTATION.md)
- [Security review](docs/development/SECURITY_REVIEW.md)
