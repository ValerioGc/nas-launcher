# Security review: UI, backend and installation

Scope: the launcher, gateway, applications, API, reverse proxies, root helpers, systemd units and
polkit rules as of 28 September 2026. This is a code and test review, not a penetration test.

## Trust model

| Who                         | Runs as                     | Trusted with                                                                      |
| --------------------------- | --------------------------- | --------------------------------------------------------------------------------- |
| The launcher                | `nas-os`, no capability     | Its database, the packages directory, starting the units polkit allows it         |
| The root helpers            | root, started by a unit     | One job each; they re-validate what the launcher wrote, as if it were compromised |
| An installed application    | a `DynamicUser`, no rights  | Its own state directory, its socket, writing as itself on the launcher's socket   |
| The release of the launcher | root, through its installer | Everything: whoever can publish a release on the repository owns every NAS        |
| The gateway                 | a `DynamicUser`, port 80    | Serving the interface, signing in and forwarding to the launcher                  |
| The browser                 | -                           | Nothing it sends is identity: `X-NAS-User` is written by the proxy                |

## Fixes applied

| Area                 | Problem                                                                                      | What was done                                                                                      |
| -------------------- | -------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------- |
| External apps        | A provisional same-origin iframe with scripts and `allow-same-origin`: no isolation          | Served under `/apps/<id>/` behind the session; same origin as the console, accepted: see below     |
| App metadata         | Links and logo could carry arbitrary schemes                                                 | Only HTTP/HTTPS URLs are accepted; `noopener noreferrer` on external links                         |
| App registry         | A lookup on an object could find inherited properties                                        | `Object.hasOwn` before resolving an internal component                                             |
| Notifications        | Unbounded input and unbounded memory                                                         | A typed schema, bounded fields, and a log capped at 500 rows                                       |
| API                  | Mutations callable from a browser form, unbounded bodies                                     | JSON required on mutations, 32 KiB limit                                                           |
| API origin           | An application page is the same _site_ as the launcher, so `cross-site` alone let it through | Mutations only from `same-origin` (or a client that is not a browser)                              |
| Errors               | Extractor answers were not uniform                                                           | Client errors normalised into `{ error }`                                                          |
| Settings             | Background values could produce arbitrary URLs or CSS                                        | Local wallpapers on an allowlist, hexadecimal colour and bounded ranges on both sides              |
| Actions              | An invalid id or action could produce a made-up outcome                                      | The resource is looked up, actions are enumerated, system applications are protected               |
| Async UI             | An earlier answer could overwrite a newer result                                             | Requests are versioned; the search is invalidated immediately and cleaned up                       |
| Rendering            | Notification or log titles could carry markup                                                | Vue text interpolation; no `v-html` anywhere                                                       |
| Shell commands       | An SSID or an address could become part of a command                                         | Fixed program and separate arguments, checked with an SSID containing `;` and `$HOME`              |
| Refused sign-ins     | The address of an attempt could be claimed by the client                                     | It is taken from the connection, never from the body                                               |
| Access list          | The first saved rule could lock the machine out of itself                                    | Loopback always passes, an unreadable list behaves like an empty one, and so do DHCP and ICMP      |
| Access list (root)   | The root helper put what the launcher wrote straight into the `nft` ruleset                  | Only address characters reach the ruleset; what is skipped is not echoed                           |
| Access list (root)   | The old table was deleted before the new one loaded: a refused ruleset left no filter        | Replaced in one `nft` transaction; a refusal keeps the previous rules                              |
| Release token        | Attached to whatever download address a release named, and a GitLab link can be anywhere     | Sent only to the forge origins; reqwest already drops it on a cross-host redirect                  |
| Package icons        | A symlink in a package (`icon.png -> ../../launcher.db`) served the launcher database        | The path is resolved and refused when it lands outside the package                                 |
| Application logos    | An uploaded image served from the launcher's origin could be an SVG carrying script          | Format read from the bytes, 256 KiB at most, served with `nosniff` and a policy that runs nothing  |
| Application packages | Unpacked with the launcher's umask, and its state directory `0700`: no application could run | Group-readable package (`g=u-w`), state directory `0710`: the group crosses it, never lists it     |
| Application cookies  | An application shares the cookies of the console's origin, the session included              | The gateway strips the session from requests to an application and drops a cookie replacing it     |
| Application access   | An application answered anybody who reached it, signed in or not                             | The gateway serves `/apps/*` to a live session only                                                |
| Application sockets  | The socket directory was not sticky: an application could replace another one's socket       | Mode `3770`: only the owner removes a socket                                                       |
| Application loopback | Any application process could call every route of the launcher on `127.0.0.1:3000`           | No loopback for the units; a socket of their own, known by cgroup, with the contract's routes only |
| Log manager          | Its backend read and deleted the launcher's notifications through that loopback              | The page asks the shell, which answers with the session of the console                             |
| Storage service      | A root process opened the launcher's database to find the wizard user                        | It reads the user from the terminal drop-in that `terminal-user.sh` writes, as root                |
| Units                | Default capabilities, clock, kernel logs, hostname and every socket family                   | No capability, `Protect*` on all four, address families limited; exposure 5.0 → 1.7 and 4.4 → 1.7  |

The units are measured with `systemd-analyze security --offline=yes <unit file>`, which runs
without installing them.

## What is checked, and what is not

- The frontend has unit, component and end-to-end tests; the installation scripts have a suite of
  their own that parses every script, checks every polkit rule against what it must **not** grant
  and runs the root helpers against fake `nft`, `systemctl` and clock.
- The Rust tests cover the parsing of access rules and of `/proc`, the manifest, the proxy, the
  package icon containment, the token scope and the router boundary.
- The launcher listens on loopback only and enables no CORS. It goes out to the internet in one
  case: the release checks and downloads of the forges, with `NAS_PACKAGES_MODE=real`.
- A package is verified against the `.sha256` published beside it. That is **integrity, not
  authenticity**: the checksum comes from the same release. Signing the releases is the next step
  if the repositories are ever shared with somebody else.
- Unpacking is GNU `tar` as the service user, which refuses absolute names and `..`; the size of
  a download is capped at 512 MiB, the unpacked size is not.
- **The applications share the origin of the console.** They are served under `/apps/<id>/`, the one
  address that reaches the NAS wherever it is, a forwarded port included: an application's script
  can reach the console's document and call the API with the administrator's session. A host name
  or a port of its own would isolate it, and neither reaches a browser that sees the NAS through one
  forwarded port. Accepted: applications come from repositories the administrator chose.
- All the applications share the `nas-os` group so the launcher can open their sockets, so an
  application can also connect to another one's socket directly, without the proxy. Applications
  come from the catalogue, which only root writes: this is accepted until each gets a socket
  directory of its own.
- **Email and webhook are configuration and nothing else.** Before anything is really sent, they
  will need full validation, secret handling, SSRF prevention and a policy for destinations and
  redirects. The interface keeps that panel closed in the meantime.
- The in-memory state (updates and ongoing operations) is global to the process and shared
  between browsers. **It is not per-user isolation.**
- The base firewall in `nas_base` drops inbound, forwarded and outbound traffic by default. It
  allows the gateway on port 80, local SSH on port 22, essential network protocols and HTTP/HTTPS
  for the update users and application units. Per-app network permissions are still to come.
- The firewall rules were loaded and reloaded in an isolated network namespace. Connectivity on
  an installed NAS still needs checking before the configuration is rolled out remotely.
- The access list is enforced by `nftables`; the middleware of the launcher only sees loopback,
  because it listens there. The generated ruleset was reviewed by reading it and by the tests,
  **not loaded by `nft`** on this machine.
- The power rules really do power off and reboot, through logind, with a polkit rule limited to
  those two actions. SSH is not exposed by the launcher: it is administered from the console.
- `npm audit` reports no vulnerability; `cargo audit` runs weekly in `audit.yml` on both
  lockfiles. No other advisory was checked and no network attack was run.
- The gateway is the one listener on the network, on port 80. It serves only files inside the
  interface directory, asks for its session before forwarding `/api`, and forbids framing the
  console (`frame-ancestors 'none'`). It checks sign-in against its own account database. Passwords
  use Argon2id; an unknown name is checked against a decoy hash. Sessions use random tokens and
  store only their SHA-256 hashes. After five refusals in a row an address waits a minute (**429**).
  Account, session and policy queries use fixed SQL with bound values, without browser input in SQL
  syntax. **The connection is still plain HTTP**: until TLS exists, use the NAS only on a trusted
  network.
- The first configuration is the wizard's, a project of its own with a review of its own. It runs
  as root on port 80 before the gateway, like the setup page of a router: whoever reaches the NAS
  first can complete it, and it ends, handing the port to the gateway, as soon as it is done.
  From the launcher it needs only `POST /api/setup/complete` (language and theme), which it calls
  on the loopback: the gateway lets through without a session only `GET /api/setup`, and
  `complete` answers 409 once the configuration is recorded as done.
- The catalogue can be edited from the App Manager, which is what decides what the NAS downloads
  and runs. It is saved in the launcher's database, the file under `/etc` stays root's, and every
  entry is validated before anything is written: identifiers, repositories and assets become
  directories, unit names, host names and forge paths, so none of them may carry a path or a `..`.
  The token still goes only to the forges, and only for entries marked private.
- The terminal runs commands as the user named after the account of the wizard, never as `nas-os`
  and never as the administrator `nas`. Root creates it once (`terminal-user.sh`) without password
  and without `sudo`; each window connects to `nas-os-terminal.socket`, which only the `nas-os`
  group may open, and systemd starts a `bash` as that user with `NoNewPrivileges`, so neither `sudo`
  nor `su` work. A user that already exists and was not created for the terminal is refused, so the
  wizard cannot hand the shell to an account that has privileges. The wizard checks the name before
  installing anything, and the root helper checks again. Each window has a `bash` of its
  own, stopped when the window goes away, four at most. A command is one line with
  no control character, so it cannot carry a second one the page did not show, and it reaches only
  a session that is open. Output is sent as text, which the page interpolates and never parses.
  Without `NAS_TERMINAL_MODE=real` no shell is opened at all. Its log tab follows the system
  journal with a fixed `journalctl --follow` that takes nothing from the request; the service reads
  the journal through the `systemd-journal` group, which is all it gains, and the journal shows the
  whole machine to whoever has signed in, which is the administrator.
- The release token travels from the wizard, as root, straight to `launcher-token.sh`, which
  encrypts it; from the App Manager it goes to the launcher, which leaves it readable by itself only
  for `nas-os-token.service`, and the unit encrypts it and removes the file however it ends. No
  route answers with it.
- An authorisation policy, a full Content-Security-Policy and TLS are still to come
  in the gateway. The launcher itself stays on the loopback.
