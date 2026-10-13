# What has been built, and when

Each entry is a batch of work, with what it left behind. The order to build the rest in is in
[ROADMAP.md](../../ROADMAP.md); the contracts are in [API.md](API.md) and the review in
[SECURITY_REVIEW.md](SECURITY_REVIEW.md).

## 3 October 2026: HTTPS and real mTLS

The gateway holds the authority of the NAS and its own server certificate, renewed when the names
of the machine change, and serves HTTPS on 443 alongside HTTP on 80. mTLS is
optional: it is configured in the security center after password sign-in. The wizard creates
only the account and the HTTPS authority. The security center lists the devices
by the fingerprint of their certificate, issues one for a new device as a PKCS#12 file with a
password shown once, renews, revokes and removes them. A renewed certificate is pending until the
first sign-in made with it. The sign-in no longer sends a placeholder certificate header and
answers the name of the device; the shell passes the requests of the security center on to the
gateway, checking the password first where a certificate is issued or mTLS switched.

## 28 September 2026: base firewall

`os_configuration` now installs an nftables table with default drop policies for inbound,
forwarded and outbound traffic. It admits the gateway, local SSH and network services needed by
the NAS. System updates, launcher downloads and application units can reach HTTP and HTTPS. The
application unit registers its cgroup with systemd `NFTSet`; reloading the rules keeps that set.
The installation check reports whether the base firewall is active, and the full uninstall removes
it before uninstalling nftables. The remaining network permission work is in the roadmap.

The gateway and launcher also check that the wizard's account name can belong to a new local
terminal user before completing setup. The browser rejects reserved names and sends a rejected
name back to the account step. The root helper checks again before creating the user.

## 17 September 2026: UI and mock backend

The launcher took its shape: the shell, the internal applications, the widgets, and a backend that
answered everything from memory.

1. [x] Axum controllers and services reorganised, typed APIs, mock data in memory.
2. [x] Shared HTTP requests, IT/EN translations, themes and a shared style for the applications.
3. [x] Open applications in tabs, state kept between them, desktop shortcuts through the API.
4. [x] Notifications, header menu, filters and mock delivery preferences.
5. [x] Settings and the existing applications wired to the backend; the desktop made personal.
6. [x] System and process widgets, with sorting and simulated actions.
7. [x] Tests reorganised, frontend and backend checked, security review written.

Decisions taken then and still standing: the backend is Rust **Axum** and the frontend uses
`fetch`, with no Axios; the i18n catalogues are TypeScript with checked keys, with no library; the
applications share only UI primitives, the HTTP client and the frame, while domain types stay in
the applications; the widgets own their folder, types and logic, and the launcher only mounts them.

## 19 September 2026: real system, and the interface around it

The batch that turned most of what was simulated into something that acts, and tidied what was
around it.

1. [x] **Notifications for real.** The log moved from memory to the database, capped at 500 rows,
       with `POST /api/notifications` as the way in for applications and system, a source selector,
       filters that apply as they change, and a line left behind by every saved settings section.
2. [x] **Access control.** A list of allowed addresses (exact or CIDR range)
       enforced by a middleware in the launcher and by an `nftables` set applied through a systemd
       unit. A refused sign-in leaves a notification with the address, the field that was wrong and
       the time. `logAccess` was removed, from the interface and from the saved documents.
3. [x] **Real telemetry.** The process widget reads `/proc` and `/sys`: processor as a difference
       between two readings, memory, swap, load, uptime, temperature where a sensor reports one,
       and every process. No dependency was added. A right click on a process terminates it or
       restarts its service through `nas-os-process@.service`; the units the NAS needs are only
       restarted.
4. [x] **The interface.** The settings link moved to the foot of the application menu; the close
       button of a tab came back (it was an `<img>` of an SVG with no intrinsic size) and now
       follows the theme; the active tab joins its window; an icon replaced the "reset filters"
       label; the application manager has bigger icons, a framed board and sortable columns; the
       network tab says which wait is happening and shows its errors where they happened; the
       resources are three animated gauges; the scrollbars are styled everywhere.
5. [x] **Appearance.** An accent colour that reaches the applications, four more wallpapers, and a
       readability helper that keeps the desktop labels legible over whatever is behind them.
6. [x] **Refactoring.** A shared sort utility, a shared debounce, a shared spinner mixin, one
       shortcut context menu instead of three, and the duplicate shortcut creation removed from the
       application manager. Two bugs found along the way: the menu sidebar styles never matched
       their markup, and `/*` is not a catch-all in Vue Router 4.
7. [x] **Language.** Comments, test names, Gherkin, SVG titles, system scripts and the documents
       under `docs/` are in English. `README.md`, `DEVELOPMENT.md` and `ROADMAP.md` stay in Italian
       on purpose, and so do the Italian translations in `i18n/*/it.ts`.
8. [x] **Tests.** 259 frontend tests and 5 end-to-end scenarios, with 93% of the lines and 86% of
       the functions covered, up from 80% and 68%.

9. [x] **The register of ongoing operations.** The tasks widget stopped being a fixture: an
       application registers its own long operation, reports progress while it goes and closes it,
       and the widget shows it. The launcher does not run the work, so asking for a pause writes
       the state and the owner honours it: and an owner that cannot be paused says so, which is
       why that button is not drawn. Nothing registers an operation yet: the first caller will be
       the package installer.

## 19 September 2026, later: packages, and the first caller of the register

1. [x] **Tests for the installation scripts.** 75 of them, in a `scripts` project of their own:
       every script is parsed, every polkit rule is checked against what it must **not** grant,
       `install.sh --dry-run` is walked end to end, and the two root helpers are exercised against
       a fake `nft`, a fake clock and a fake `systemctl`. The first thing they caught was a file in
       `files/` that nothing installed.
2. [x] **Application packages, for real.** The catalogue left the binary and became
       `/etc/nas-os/catalogue.json`. The launcher asks GitHub or GitLab for the latest
       release, downloads the asset, checks it against the `.sha256` published beside it, unpacks
       it with `tar` into a directory of its own and renames it over the one that was there. The
       token for private releases is the same encrypted systemd credential used for launcher updates and lives outside the database. None of it happens
       without `NAS_PACKAGES_MODE=real`.
3. [x] **The launcher updates itself** through the same machinery, looking at the repository named
       in its unit. The last step is a root unit, because a running service cannot replace its own
       binary; a missing checksum is refused outright there rather than warned about.
4. [x] **The register of ongoing operations has its first caller.** Every installation and every
       update registers itself, reports where it got to (fetching, verifying, unpacking) and
       closes with the version or with the reason it could not.

Verified against a forge on loopback that serves a package it builds itself: the end-to-end
scenario installs it through the interface, watches the operation finish, and checks the version in
the register. A deliberately wrong checksum was refused, leaving nothing on disk and nothing in the
register.

Two dependencies were added, the first since the project started: `reqwest` with `rustls` for
HTTPS, and `sha2` for the checksum. Unpacking is the system `tar`, and hashing is in-process
because verifying a package is not something to read back from another program's output.

What is still simulated: the system updates from apt, and the delivery of notifications.

## 19 September 2026, later still: an application that opens

1. [x] **The manifest is read.** `services/manifest.rs` reads the `nas-app.json` of every unpacked
       package, refuses one whose `manifest_version` it does not know and one whose `app.id` is not
       the name of its own directory: the directory is the identity, and a package that claimed
       another one's id would be handed that application's socket. The socket path is **derived**
       from the identifier and never taken from the file: a package that could name its own socket
       could name `/run/dbus/system_bus_socket`.
2. [x] **A reverse proxy onto the application's unix socket.** `http/proxy.rs` routes by host name:
       `<id>.nas.internal` goes to `/run/nas-os/apps/<id>.sock`, `nas.internal` stays the
       launcher. Routing by path would have put every application on the launcher's own origin,
       where an iframe is not a boundary at all; with one origin each the wall is the browser's
       same-origin policy. The connector is thirty lines over `tokio::net::UnixStream` rather than
       a crate in the one path between the shell and somebody else's code.
3. [x] **The shell opens it.** `AppHost` renders a sandboxed iframe on the application's own origin,
       computed from `NAS_APP_ORIGIN` at build time; the "no interface yet" view is gone, and so is
       the string that said so.
4. [x] **The installation knows where the sockets live.** The unit creates
       `/run/nas-os/apps` and keeps it across a restart of the launcher, because the
       applications are not restarted with it, and `check.sh` reports the directory, how many
       applications are listening and whether the names resolve from this machine.

Verified end to end: the packages scenario now installs the application, opens it from the menu,
reads the page that came off the socket inside the frame, checks that the user the application is
told about is the launcher's word and not the client's, and, after the uninstall, that the host name
answers 404. By hand against a backend and a fixture application: `/internal/*` refused with 403, a
connection upgrade with 501, an application that is not listening with 503, a mixed-case host name
resolved, and a 100 KB body carried through: the API's 32 KiB limit sits inside the proxy layer,
not in front of it.

Three dependencies were added, all from the hyper family already pulled in by Axum: `hyper` with
its client, `hyper-util` for the tokio adapter and `http-body-util`, which was already there for
the tests.

What is still missing on this path: nothing starts the application's process (that unit is written
by hand for now) and there is no `postMessage` protocol between the shell and the frame yet.

## 19 September 2026, last: the applications run, and the frame talks back

1. [x] **The launcher starts and stops the applications.** One systemd template for all of them,
       `nas-app@<id>.service`, and one polkit rule granting `start`, `stop` and `restart` on a unit
       of that shape: not `enable`, because an application is started by the launcher and not by
       the boot. The template does not know what to execute either: `run-app.sh` reads `run.exec`
       out of that application's manifest, refuses a path that leaves the package, and hands the
       program the socket to listen on. One unit and one rule, however many applications are
       installed.
2. [x] **An installation ends with the process running.** Installing or updating restarts the unit
       (the version on disk is not the version running until it does) and removing a package
       stops it first. A unit that does not come up is a notification, not a failed installation:
       the package is there and the manager says so.
3. [x] **The manager shows the state and can change it.** Every application carries `running`, and
       the button offers the opposite of what is the case. Where the launcher does not run
       applications at all (a development copy, `NAS_APPS_MODE` unset) neither the state nor the
       button is drawn, rather than showing a state nobody is keeping.
4. [x] **The shell and the frame talk.** `useAppBridge` sends the theme, the accent and the
       language when a frame loads, when the application says `ready` and whenever any of the three
       changes; the application may ask for a notification or for another application's window. The
       origin is checked in both directions: out, always the application's own and never `*`; in,
       the origin **and** the frame's own window, so one application cannot speak for another.
       A notification is signed with the frame it arrived in, never with a field of the message.

Verified against a fake `systemctl` first in `PATH`, never against this machine's own: installing
restarted the unit, start, stop and restart each did one thing, uninstalling stopped it, and the
list of units ended empty. The end-to-end scenario goes further and uses a real browser across two
origins: the fixture application receives the theme and the language through the frame, asks for a
notification, and the log shows it signed `fileExplorer`.

What is left on this path: the menu still lists what the register says rather than what is unpacked
on disk, and there is no file-picker message yet, because there is no application to provide one.

## 19 September 2026, A1: a directory is an application

1. [x] **The list comes from the disk as well as from the register.** `/apps` merges the rows an
       installation wrote with the manifests under `NAS_APPS_DIR`: a package copied in by hand
       appears with the name and version of its `nas-app.json`, a row whose directory has gone stays
       so it can be removed, and where the two disagree on the version the directory wins: it is
       what will be opened, and what the update check has to compare against. `installed_versions`
       merges the same way, so a hand-copied application stops being offered as available.
2. [x] **A directory is proof of installation.** `is_installed` asks the package directory before
       the register, so the desktop shortcut, the details, start and stop, and the removal all work
       on an application nobody installed through the manager.
3. [x] **The icon comes out of the package.** `GET /apps/{id}/icon` serves the file the manifest
       names, with the path kept inside the package, the extension limited to the image formats,
       512 KiB as the ceiling, and `nosniff` plus `default-src 'none'; sandbox` on the answer,
       because an SVG opened directly is not a picture but a document. The shell draws it through
       one helper, `appIcon`, so menu, desktop, tabs and the manager all follow.

Verified in a browser end to end: a scenario writes a manifest and an icon into the applications
directory, finds the application in the menu with its own icon, reads the icon through the API with
its headers, then deletes the directory and finds it gone. Five unit tests cover the merge itself,
which is where the rules live.

## 20 September 2026: a real application, and the corners around it

1. [x] **Where a package goes in development.** The applications directory is the one beside the
       database, so `npm run backend:dev` finds what is unpacked into `backend/data/apps` without
       being told; the socket directory now follows the same rule in a debug build, because a
       development copy cannot write in `/run` and every application would otherwise need
       `NAS_APP_SOCKETS` spelled out.
2. [x] **Empty states that say something.** One component, `CommonEmpty`, drawn inside the surface
       that would hold the list instead of as a line of text under it: the application manager
       keeps the height of its board whether it holds twenty applications or none, the notification
       dropdown says what would put something there, and the operations widget drops the "0 running"
       count when the register is empty: a count of nothing on top of an empty panel is noise.
3. [x] **Settings moved into the category column** of the application menu, at its foot, instead of
       a strip across both columns. The tabs of the open applications got room between the name and
       the close button, which were touching, and a minimum width.
4. [x] **Leaving a wireless network.** `POST /network/disconnect`, with the guard that is the point
       of it: the interface the request arrived through is never disconnected, because nobody would
       be left to connect it again. `canDisconnect` says whether there is anything to leave, and the
       button exists only when there is.

The router tests no longer depend on the machine they run on: the applications directory they
answer against is one that is not there, so the list is the four built-in applications whatever the
developer happens to have unpacked.

## 24 September 2026: packaging, toasts and real updates

1. [x] **One release package.** Install files moved to `system/`, the root helpers next to the Rust
       module that starts them, and `npm run release:package` builds
       `nas-os-<version>-linux-x64.tar.gz`: the name the launcher looks for when it updates
       itself. The pipeline only verifies, builds and publishes; `npm run release:test` is the
       local gate.
2. [x] **The interface.** One table component for processes, updates and the application list,
       one spinner, value placeholders instead of a page spinner in the network settings, compact
       alerts for saving, hardware-address rules removed, and every page and answer kept out of
       search engines.
3. [x] **Toasts.** A notification that arrives, and an error of the page itself, pop up on screen;
       what the page's own actions just caused does not. The header menu lists only the unread.
4. [x] **Real system updates.** The list comes from `apt`, refreshing and installing run in
       `nas-os-apt@.service` as root, one job at a time, in the register of operations.
5. [x] **The launcher updates itself safely.** A root unit backs up database, configuration and
       installed files, runs the package's own installer, checks that the new launcher answers
       and reboots; on a failure it restores the backup. The interface follows it in a modal.
6. [x] **One token, encrypted.** The token of the private releases (the launcher's and the
       applications') is a systemd credential set on the machine, never through the interface.
7. [x] **Preinstalled applications.** Archives in `system/apps` travel in the package and are
       registered at the first start.

## 25 September 2026: gateway mock, release fixes, performance and security

1. [x] **Sign-in through a mock gateway** (`gateway/`): a cookie session and a guarded console,
       to exercise the flow. Not a security boundary.
2. [x] **Release.** The changelog no longer blocks the pipeline (without an entry the notes are
       the commits since the previous tag) and the manuals build on the CI runners.
3. [x] **Performance.** The header polls only the unread notifications, and every poll catches up
       as soon as the page is visible again; the telemetry pass runs entirely on the blocking pool
       and no longer holds the whole session while it reads `/proc`; the release binary is built
       with `lto` and without symbols, 41% smaller.
4. [x] **Security.** The token goes only to the forges; a package icon that resolves outside the
       package is refused; the root firewall helper filters its input and replaces the table in
       one transaction; the proxy drops cookies bound to a `Domain`; mutations are accepted only
       from the launcher's own origin; the socket directory is sticky; the units lost every
       capability, with an exposure of 1.7 instead of 5.0 and 4.4. The details are in
       [SECURITY_REVIEW.md](SECURITY_REVIEW.md).
5. [x] **Documentation.** The installation manual covers installing, checking, configuring,
       updating and removing the launcher; the release process is in `DEVELOPMENT.md`.

## 27 September 2026: applications by path, the log behind a notification, a real sign-in

1. [x] **Applications by path.** Every application is served under `/apps/<id>/` on the origin of
       the launcher: no port and no host name to configure on the machines that browse the NAS.
2. [x] **The log behind a notification.** `GET /api/notifications/{id}/log` and the magnifier that
       opens nas-log-manager on it.
3. [x] **A real sign-in.** The wizard creates the account in the gateway's own database (Argon2id);
       sessions are rows there, with idle and absolute expiry, and survive a restart. The gateway
       names the signed-in user to the launcher and the applications in `X-NAS-User`. Only the
       client certificate is still a mock.
4. [x] **Release v0.1.5.**
5. [x] **The operations widget in two tabs.** Running (the default) and finished, each from the
       most recent operation to the oldest.
6. [x] **nas-backup-manager in the catalogue.** The scaffolding of the application (package, release
       pipeline, a first page that follows the shell's theme and language) and its catalogue entry.
       The external applications now live in repositories of their own, outside this one.
7. [x] **Common end-to-end steps.** `steps/base_page.ts` goes to a page, clicks, fills and checks
       elements by `data-testid`.
8. [x] **Documentation.** The roadmap keeps only what is missing; history and the reasons behind
       choices already made left the docs, and so did `PLAN.md` and `ARCHITECTURE_FUTURE.md`. No
       file carries the em dash any more, and a test keeps it that way.
9. [x] **Migrations squashed.** One migration for each element of the schema, seven in all, with
       short comments. The launcher no longer keeps a `users` table nor answers
       `/internal/sign-in`: the account lives only in the gateway. An installation made before has
       to be reinstalled.
10. [x] **The sign-in lock in the settings.** On or off, how many refused attempts and for how many
        minutes, kept by the gateway and changed from the Access tab.
11. [x] **Desktop icons in any order.** Dragged with the pointer, or moved with Alt and the arrows;
        the order is saved.
12. [x] **WebSockets reach the applications.** The gateway and the launcher carry the upgrade to
        the socket of the application; any other upgrade is still refused.
13. [x] **Processes, all of them, and what to do with one.** The process widget lists every process;
        a right click terminates one or restarts its service through `nas-os-process@.service`,
        and the units the NAS needs are only restarted.
14. [x] **Copies of the databases.** The launcher and the gateway keep a copy made with
        `VACUUM INTO` while they run; the gateway's installer copies its database before an update.
15. [x] **The CSS for the browsers supported.** `autoprefixer` and `cssnano` read one
        `browserslist`, the default target of Vite.
16. [x] **Recovery and reset.** The configuration file carries the desktop and the settings of
        the applications chosen; with the App Manager, the import installs and removes
        applications to match the file.
