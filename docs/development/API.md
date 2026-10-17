# API

Base: `/api`. JSON UTF-8. Rust Axum backend, loopback `127.0.0.1:3000` (`PORT` configurable).
The controllers extract requests and responses; services, catalogues and fixtures hold what is
still simulated. No authentication: the gateway is in front of it.

The processes of the applications have no loopback. They reach a subset of these routes on the
unix socket `NAS_LAUNCHER_SOCKET` (`/run/nas-os/launcher/apps.sock`), where the launcher knows them
by their unit and lets them write notifications and operations only as themselves: see
[APPS.md](external_apps/APPS.md#the-apis-you-may-call).

**What survives a restart** lives in SQLite: settings (`/settings/*`, one row per section),
installed applications, desktop shortcuts, the notification log, the list of allowed addresses and
the register of ongoing operations. **What does not** is the updates section, which is shared
between every client of the same process and resets when it restarts.

Mutating routes leave a notification behind.

Every route, with its parameters, its bodies and its answers, is in
[openapi.json](openapi.json), generated from the handlers of the launcher, and the interface reads
the same shapes out of `src/types/api.ts`. After changing a route, `pnpm run api:generate` writes
both again; `cargo test` and the pipeline fail while they are out of date. A failure answers
`4XX` or `5XX` with `{ "error": "..." }`. This file keeps what the document cannot say: how the
routes behave.

`GET /health` is outside the API prefix.

What an application may call, and how, is in [../APPS.md](external_apps/APPS.md).

## Notifications

The log is a table, so **a notification written before a restart is still there after it**, capped
at the 100 most recent. Older entries are removed on insertion and at startup.

Filters can be combined: `tone=info|success|warning|error`, `source`, `read=true|false`, `q`,
`limit=1..500`. The most recent first. `at` is a Unix timestamp in milliseconds.

`q` is applied in the launcher, not in SQLite: the database compares ASCII, so `Però` would not
match `però`. The other filters are applied by the query.

The header of the shell asks for `read=false&limit=500` every fifteen seconds: the unread ones are
all it shows. The whole log is read only by the notification manager.

```json
{
  "title": "Backup completato",
  "message": "Operazione di esempio",
  "source": "nas-backup-manager",
  "tone": "success"
}
```

Creating one also returns `id`, `at` and `read: false`. Title at most 160 bytes, message 2000,
source 64 with ASCII alphanumerics, `_` and `-` only.

Three optional fields go with it. `key` names the text the interface translates, and `params`
(at most 12, each value up to 500 bytes) fills in its `{name}` placeholders; `title` and `message`
stay the text shown when the interface has no translation for the key, and the only text for
whoever reads the log without the interface. The launcher writes a key on every notification of
its own, with its English text. `log` is the absolute path of a file that tells more, without `..`
segments, which nas-log-manager will open. A value ending in `At` is a time in milliseconds, and
the interface formats it. **This is the way in for the applications
and for the system**: whoever has something to report posts it here, and it lands in the same log
the shell reads. It is not an authorisation boundary yet.

`GET /notifications/{id}/log` is what nas-log-manager shows behind a warning or an error with a
`log`: the last 256 KiB of that file, which has to resolve under `/var/log` (**400** otherwise,
**503** if it cannot be read), at most the last 1000 lines, as `{ "kind": "file", "origin":
"<file>", "lines": [...] }`. Another tone answers **400**, a notification without `log` **404**:
the notification center shows the magnifier only when there is a file to open.

`GET /notifications/sources` returns the sources that appear in the log, which is what the filter
selector of the application offers instead of asking for an identifier that has to be known in
advance.

Every saved settings section leaves one behind, saying which section changed.

## Settings

Read the object with GET, change its fields and send it back whole with PUT. Unknown fields are
refused. `general` holds:

- `theme`, `language`, `createShortcut`, and `accent`: `#RRGGBB`, or empty to keep the accent of
  the theme.
- `background.mode`: `image` or `gradient`; `background.image`: one of the local
  `/wallpapers/*.svg` files, listed once in the backend and offered by the interface.
- `background.blur`: 0–30; `intensity`: 0–100; `color`: hexadecimal `#RRGGBB`.
- `background.direction`: 0–360; `size`: 10–100.
- `background.pattern`: `halo`, `dots`, `multiple`, `linear`, `rings`, `grid`, `stripes`.

Delivery: `emailEnabled`, `email`, `webhookEnabled`, `webhook`. An enabled webhook has to be
HTTPS. These are preferences and nothing more: **no outgoing request is made**, and the interface
keeps the panel that edits them closed for that reason.

Schedule: `{ enabled, rules: [{ day, time: "HH:MM", action, active }] }`, at most 32 rules.
`action`: `start`, `shutdown`, `restart`. The rules are executed by the scheduler in the backend.

Access: `{ notifyFailures }`. SSH and its port are not here:
they are configured on the machine. The allowed addresses **are** here, on `/access/rules`.

The widgets share `/settings/widgets`: the interval is changed from the gear of the widget and
holds for both the processes and the operations.

The fields with a fixed set of choices (`theme`, `language`, `background.mode`,
`background.pattern`, `day`, `action`, `tone`, `kind`) are types: a value outside the list is
refused while the body is being parsed, and the answer says which values exist, for example
`{ "error": "theme: unknown variant \`viola\`, expected one of \`light\`, \`dark\`, \`system\`" }`.

Domain errors have status 400/404 and a body `{ "error": "..." }`. The frontend shows a
translatable message and offers to try again. The maximum request body is 32 KiB.

Mutations require `Content-Type: application/json` (even with an empty body: `{}`), or one of
`image/svg+xml`, `image/png`, `image/jpeg`, `image/webp` for the logo of an application: none of
them is a type an HTML form can send. The middleware refuses with 403 a mutation whose
`Sec-Fetch-Site` is anything but `same-origin` or `none` (a page on another site, or on another
port of the same host, is refused) and any other content with 415. A
malformed body, one with unknown fields, or one with a value outside the list answers 400 with the
same `{ "error": "..." }` body.

## Access control

The list says who may reach the NAS. **An empty list allows everyone**: the restriction is switched
on by adding the first rule, not by installing anything.

```json
[{ "id": "1", "kind": "range", "value": "192.168.1.0/24", "label": "Casa", "createdAt": 0 }]
```

`kind` is `ip` (an exact address) or `range` (CIDR); any other kind answers **400**. The value is
normalised on the way in (`10.0.0.0 / 8` and `10.0.0.0/8` are the same rule) and a value that is
not an address answers **400** with what is wrong with it.

It is enforced in two places, because neither would be enough:

- **In the launcher**, by a middleware: a request from an address that is not on the list gets
  **403**. Loopback always passes, and a list that cannot be read behaves like an empty one: a
  database hiccup must not lock the machine out of itself.
- **In the firewall**, where `nftables` drops the packet before it becomes a request, covering the
  rest of the NAS and not only the launcher. It needs `NAS_FIREWALL_MODE=real` and the unit
  installed by `system/packages/ui_installation/`; without them the list is still saved and still enforced by the
  launcher.

`POST /access/failures` takes `{ "field": "username" | "password" }` and records a refused sign-in
with the address the request came from (taken from the connection, never from the body) and the
time. It answers `{ "recorded": false }` when the access settings do not ask for it.

## Network

`GET /network` returns `{ connection, wifi, wiredAvailable, canConnect }`. `connection.kind` is
`lan` or `wifi`; on a cable `ssid`, `signal` and `security` are `null`, and `speed` is the
negotiated rate where the link exposes one. Every network in the list carries `ssid`, `signal` as a
percentage, `security` (`open`, `WPA2`, `WPA3`) and `known`, which says whether it has been used
before.

`canConnect` says whether changing network would really do anything on this installation.

**No made-up data:** the connection and the list are the machine's own, with default route, address,
MAC and DNS from `ip` and from sysfs, SSID, signal and protection from NetworkManager. The list has
one entry per name (the same network seen on two bands stays one, with the stronger reading) and
leaves out hidden networks, which cannot be offered in a list. Where the network cannot be read
(no default route, or no `ip`) the answer is **503** `the network of this machine cannot be read`,
rather than an invented address.

`POST /network/scan` asks the antenna for a new search (`nmcli device wifi rescan`) and answers
with the updated state. A search refused for being too close to the previous one is not an error:
the list that follows is recent anyway.

`POST /network/wifi` wants `{ ssid, password? }` and `POST /network/wired` goes back to the cable.
They really do connect, through NetworkManager, but only where the installation allows it:

- without `NAS_NETWORK_MODE=real` the answer is **503** `network changes are disabled on this
installation`: that is every development copy, and it is there so nobody moves the Wi-Fi of the
  machine they are programming on;
- a password shorter than 8 characters or longer than 64 is refused with **400** before anything
  leaves the process, and an empty SSID or one longer than 32 characters with **400**;
- if the network refuses the password the answer is **400** `the network refused the password`,
  while every other refusal from `nmcli` becomes **503** with the original reason.

The command is never a composed string: fixed program and separate arguments, so an SSID with
spaces or semicolons stays an SSID.

### Leaving a network

`POST /network/disconnect` leaves the wireless network, and refuses to leave the one the request
arrived through: a NAS that disconnects its own way in cannot be told to connect again, which is
the same reason the polkit rule grants no `enable-disable-network`. The state carries
`canDisconnect`, true only where there is a wireless connection that is **not** the one carrying
the default route, and the interface draws the button from it.

| Situation                          | Answer                              |
| ---------------------------------- | ----------------------------------- |
| A second wireless connection       | **200** with the new state          |
| It is the connection in use        | **400** it cannot be left from here |
| No wireless connection at all      | **400** there is nothing to leave   |
| `NAS_NETWORK_MODE=real` is missing | **503** naming the variable         |

### One way in at a time

`wiredConnected` is true when a cable is plugged into a wired card that has an address, whoever
manages it (NetworkManager or `/etc/network/interfaces`, read from sysfs and `ip`). Meanwhile
`POST /network/wifi` answers **409** `the network cable is connected`: the list of the wireless
networks stays there to see, and the page says why it joins none.

### Network cards

`adapters` lists the wired and wireless cards NetworkManager knows, each with `interface`, `kind`
(`lan` or `wifi`), `state` (`connected`, `disconnected`, `unavailable`, `unmanaged`), `enabled`,
`inUse` (it carries the default route) and `canToggle`. `POST /network/adapter` wants
`{ interface, enabled }`:

- turning off saves the card in the `network` settings section and disconnects it
  (`nmcli device disconnect`); the card in use answers **400**, and so does a card left to
  `/etc/network/interfaces` (`unmanaged`), which NetworkManager does not touch;
- turning on removes it from the list and tries `nmcli device connect`, which may find nothing to
  join: that is not a failure;
- a Wi-Fi card turned off is not asked for networks (`wifiProblem: "disabled"`), and
  `POST /network/wifi` and `POST /network/wired` refuse a card turned off with **400**.

A request of `POST /storage/encryption` or `POST /storage/network` that carries a password or a
key, or gives one back (a format with a password, unlock, password, recovery key, header backup,
the automatic key with a password, saving a network disk with its password, trying one) needs
`x-nas-secure: 1`, which only the gateway sets, on a request that came over TLS or from the monitor
of the NAS: otherwise **403** `a password or a key travels only over HTTPS`.

Every 30 seconds the launcher checks the cards turned off: one that came back while another card
carries the default route is disconnected again, and one is brought back when the NAS has no
default route at all, so it can be reached again. Only `network-control` is used, the polkit rule
grants nothing more. From a console, `nmcli device connect <card>` brings a card back until the
page turns it on, or until the next check if another card carries the network.

## Desktop

Every icon is a row of `desktop_shortcuts`, so **it stays after a restart**:

```json
[{ "kind": "app", "target": "nas-notification-manager", "name": null, "position": 0 }]
```

`kind` tells applications from folders, which will arrive with their path in `target` and their
name in `name`; for an application the name comes from the register, so a name that later changes
is not left written here. `position` is the order of insertion.

A freshly installed NAS starts with the system applications already on the desktop: migration
`0004` puts them there, and it runs once only, so an icon that was removed does not come back on
the next restart.

Adding the same icon twice creates no duplicate and does not move it. An application that is not
installed answers 404, and removing an icon that is not there answers 404: the interface tells
"removed" from "was not there". Removing an application takes its icon with it.

## Applications

`/apps` lists the system applications first: compiled into the launcher, `system: true`: they are
always there, and installing or removing one answers 400. Then come the installed ones, read from
the register in the database, each with the `version` that is installed.

An installation also puts the application on the desktop when the `createShortcut` general setting
asks for it: the backend does that, inside the installation, so one tick does not produce two
writes.

Disks, backups and the event log are not served from here any more: they are external
applications, with their own process.

### The catalogue

**What can be installed is a file, not a list in the code**: adding an application should not be a
release of the launcher. It is read from `NAS_CATALOGUE_PATH`, then `/etc/nas-os/catalogue.json`,
and (only in a debug build) the sample that ships with the repository.

```json
{
  "version": 1,
  "apps": [
    {
      "id": "fileExplorer",
      "name": "Esplora file",
      "info": "Sfoglia i file del NAS.",
      "provider": "github",
      "repository": "example/nas-file-explorer",
      "asset": "file-explorer-{version}-linux-x64.tar.gz"
    }
  ]
}
```

`provider` is `github` or `gitlab`, `repository` is `owner/name` the way both forges write it, and
`asset` is the name of the release asset with `{version}` standing for the tag without its `v`.
`preinstall: true` marks an application the NAS installs on its own right after the setup wizard.
`category` files it in a folder of the menu: `media`, `files`, `storage`, `network`, `security`,
`utilities` or `development`, or `null` for none; `/api/apps` answers with the category of each
installed application, and `null` for the system ones, which have a folder of their own. The version
of a system application is the one in its `src/apps/<id>/metadata.json`, not the launcher's.
A catalogue saved from the App Manager takes the place of the file; its entries are refused
(**400**) when an identifier is not a plain word or is a system application, a repository is not
`owner/name` or has a `.` or `..` segment, an asset carries a path, or two entries share an
identifier. `GET` and `PUT /apps/catalogue` answer with a `logo` for each entry: where the launcher
serves it, or empty. The `logo` an older catalogue carries, an address on the internet, is read and
ignored.
Every request to GitHub or GitLab carries the release token when one is stored, and a download
address outside their origins never does. `credential` and `private`, which older catalogues carry,
are read and ignored. A
missing or unreadable file is an empty catalogue, not a failure: a NAS with no catalogue still runs
everything it already has.

`GET /apps/available` answers with what the catalogue offers and what is installed:

```json
[
  {
    "id": "fileExplorer",
    "name": "Esplora file",
    "info": "…",
    "repository": "example/nas-file-explorer",
    "installedVersion": null,
    "latestVersion": null,
    "updateAvailable": false
  }
]
```

`latestVersion` is `null` until somebody asks. **Reading this page reaches no repository.**

### Checking, installing, updating

`POST /apps/updates` is the one moment the NAS talks to the internet, and it happens because a
button was pressed or a timer fired. It asks each repository for its latest release, compares the
tag with the installed version (numerically, so `1.10.0` is newer than `1.9.0`) and answers with
the same shape, `latestVersion` and `updateAvailable` filled in. A repository that cannot be
reached leaves its entry without a latest version rather than failing the whole answer.

`POST /apps/{id}/install` (and `update`, which is the same thing over what is there) **answers
immediately**:

```json
{ "id": "fileExplorer", "action": "install", "accepted": true, "task": "install-fileExplorer" }
```

The work runs in the background and reports into the register of ongoing operations, because a
download of several hundred megabytes is not something to hold an HTTP request open for. The steps
are: look up the release, fetch the asset, **check it against the `<asset>.sha256` published beside
it**, unpack it with `tar` into a directory of its own, and only then rename it over the one that
was there: so a download that fails halfway leaves the working copy exactly where it was. A
release that publishes no checksum is installed with a warning in the log, not silently.

`POST /apps/{id}/uninstall` removes the row, the desktop icon and the package on disk. With
`?removeData=true` it also deletes the state directory of the application (`systemctl clean`).

Without `NAS_PACKAGES_MODE=real` none of this reaches anywhere: every one of these routes answers
**503** saying so, which is what every development copy does.

### The token

There is one release token, the encrypted systemd credential `nas-os.launcher-token`, for the
launcher's own updates and for every application alike. `GET /apps/credentials` reports only its
presence and whether package operations are enabled. The wizard stores it, as root, through
`ui_configuration`; later `PUT /apps/credentials` from the App Manager replaces it, and so does
`sudo /usr/local/lib/nas-os/configure.sh` on the machine; no route ever answers with it. With
`NAS_TOKEN_MODE=real` the launcher writes it, readable by itself only, to
`/var/lib/nas-os/launcher-token.new` and starts `nas-os-token.service`, which encrypts it with
`launcher-token.sh`, removes the file and restarts the launcher. A development copy writes it to
`NAS_LAUNCHER_TOKEN_PATH` instead.

### What counts as installed

`/apps` and `/apps/installed` answer with the built-in applications followed by everything
installed, and **installed means two things at once**. The register holds what an installation
wrote: the name and the description of the catalogue entry, and the version of the release it
fetched. The directory under `NAS_APPS_DIR` holds what is actually there to open.

A package copied into that directory by hand is an application: it appears with the name and the
version of its `nas-app.json`, and with the catalogue's description when the catalogue knows the
id. A row whose directory has gone stays in the list, because removing it is what is left to clean
up after it. Where the two disagree on the version, the directory wins: that is what will be
opened, and it is also the version the update check compares against.

`GET /apps/{id}/icon` serves the file `app.icon` names inside the package. It is a file a third
party wrote, served from the launcher's own origin, so the path has to stay inside the package, the
extension has to be one of `svg`, `png`, `jpg`, `webp` or `ico`, the file has to be under 512 KiB,
and the answer carries `X-Content-Type-Options: nosniff` and a content policy that allows nothing to
run: an SVG inside an `img` never executes, one opened directly would. An application without an
icon answers **404**, and the shell draws its default logo.

`/apps/{id}/logo` is the logo of a catalogue entry, which a private repository cannot link to: an
image uploaded from the App Manager (`PUT`, the image as the body, at most 256 KiB, its format read
from its bytes: SVG, PNG, JPEG or WebP), kept in the table `app_logos`, or else the file
`<id>.svg|png|jpg|webp` in `NAS_LOGOS_DIR`, `/usr/local/share/nas-os/logos` on the NAS, which the
installation fills with the logos of the applications it knows. It is served with the same headers
as the icon. An uploaded logo belongs to an entry of the catalogue (**404** otherwise) and goes with
it when the entry is removed. The shell draws, in order, the icon of the package once installed,
the logo, and its default one; the address of an uploaded logo carries `?v=` with the time it
changed.

### Running one

An installed application is a process, and the process is a systemd unit: `nas-app@<id>.service`,
one instance of the template the installation puts down. The launcher does not run it, it asks
(`POST /apps/{id}/start`, `/stop` or `/restart`) and answers with what came back:

| Situation                        | Answer                                               |
| -------------------------------- | ---------------------------------------------------- |
| Done                             | **200** `{ id, action, accepted: true, task: null }` |
| Not installed                    | **404** `unknown application: <id>`                  |
| A system application             | **400** they are part of the launcher                |
| Any verb but those three         | **400** `invalid application action`                 |
| `NAS_APPS_MODE=real` is missing  | **503** naming the variable                          |
| systemd refused, or did not come | **503** with what `systemctl` said                   |

Every application listed by `/apps` carries `running`, which is that unit being active. A system
application is part of the launcher, so it is always `true`. Where the launcher does not run
applications at all, everything installed reads as not running and `/apps/credentials` says
`appsManaged: false`, which is what the interface draws its buttons from.

An installation and an update restart the unit when they finish, and an uninstall stops it before
the package is taken off disk.

### Opening one

An installed application is not served under `/api`: it answers under `/apps/<id>/`, on the
launcher's own address, which the launcher proxies onto `/run/nas-os/apps/<id>.sock`,
`NAS_APP_SOCKETS`. The socket path is **derived from the identifier**, never read from the
package's manifest. The application receives its own paths, without the prefix, and the prefix in
`X-Forwarded-Prefix`; a `Location` it answers with that starts with `/` is put back under the
prefix. `/apps/<id>` without the final slash answers **308** towards `/apps/<id>/`.

| Situation                   | Answer                                                     |
| --------------------------- | ---------------------------------------------------------- |
| The application answers     | What it answered, without cookies bound to a `Domain`      |
| Not installed               | **404** `this application is not installed`                |
| Installed, not listening    | **503** `<id> is not answering: it is not running`         |
| Any path under `/internal/` | **403** `this path is not served to the browser`           |
| A WebSocket upgrade         | Carried to the application, both ways, once it answers 101 |
| Any other upgrade           | **501** `this proxy carries WebSocket upgrades only`       |

The shell shows that origin in a frame, without a sandbox (on the origin of the console it is no
wall, and every application keeps its rules with its own `frameGuard.ts`), and the one channel
between the two is
`postMessage`: the shell sends the theme, the accent and the language, the application may ask for
a notification or for another application to be opened. Both sides check the origin, and a
notification is signed with the frame it arrived in rather than with anything the message said.
[APPS.md](external_apps/APPS.md) has the messages.

Two things travel differently through the proxy than through the rest of the API. Hop-by-hop
headers are stripped in both directions, and `X-NAS-User` is **written** by the launcher: whatever
arrived under that name is dropped, because a browser that could name its own user would be every
user. Its value is the user the gateway signed in, or `local` when nobody named one. A `Set-Cookie` with a `Domain` attribute is dropped from
the answer: the applications share the host of the launcher, and such a cookie would reach the
launcher and every other application. The 32 KiB body limit of the API routes does not
apply here: the proxy sits outside it.

## Ongoing operations

A register, not a scheduler. **The launcher does not run the work**: an application that copies,
indexes or downloads registers its operation here, reports progress while it goes, and closes it at
the end. The widget reads the register and shows it.

```json
{
  "id": "backup-nightly",
  "source": "nas-backup-manager",
  "name": "Copia notturna",
  "detail": "12 di 30 GB",
  "progress": 42,
  "state": "running",
  "pausable": true,
  "cancellable": true,
  "createdAt": 0,
  "updatedAt": 0
}
```

`POST /tasks` registers one. The identifier is chosen by the caller (`backup-nightly`, not a
number) and registering the same one again **replaces** it: an application that restarts its own
nightly backup finds its row rather than leaving a stale one behind. `id` and `source` are
identifiers (ASCII alphanumerics, `_` and `-`, at most 64); `name` is free text up to 160
characters and `detail` up to 500.

**`progress` may be absent**, and that is a statement: the owner cannot tell how far along it is,
and the interface draws a bar with no position instead of a convincing zero. Sending an explicit
`null` in a `PATCH` is how an owner says it no longer knows.

`PATCH /tasks/{id}` is the owner reporting: `name`, `detail`, `progress` and `state`, each
optional, and a field left out is not cleared. The states are `running`, `paused`, `done`, `failed`
and `cancelled`; **a finished operation cannot be moved again**, so an owner that starts over
registers it again rather than reopening a row the user has already read. A move to `failed` also
leaves a notification, because that is the one outcome the user must not miss.

`POST /tasks/{id}/pause|resume|stop` is what the **user** asks for from the widget. The launcher
cannot pause anything, so it writes the state and the owner, which rereads its own rows, honours
it. `pausable` and `cancellable` say what the owner agreed to be asked for: an action it did not
agree to answers **400**, and the interface does not draw the button at all: better a missing
button than one that is ignored.

The register keeps at most 30 finished operations and sweeps them up after a day. Running and
paused operations do not count toward the limit and are never removed by retention. A new
completion replaces older finished operations with the same source, name and detail. Cleanup
also runs at startup for previously stored records.
`POST /tasks/finished` clears them now; `DELETE /tasks/{id}` removes one whatever state it is in.

**Nothing registers an operation yet.** The facility is there and the widget is real; the first
caller will be the package installer.

## Machine telemetry and privileged process control

System Monitor owns the collection, types and sampling of CPU, memory, processes and sensors.
Its window and widget read `api/telemetry` from the application's own backend, served below
`/apps/nas-system-monitor/`. Its manifest requests read access to `/proc` and `/sys`.

`GET /system/telemetry` is a compatibility forward to that application's `/api/telemetry` on
its Unix socket. It collects no data and returns 404 when System Monitor is not installed.
The response contract belongs to nas-system-monitor's DEVELOPMENT.md.

`POST /system/processes/{id}` with `{ "action": "terminate" | "restart", "started": 81234 }`
terminates the process (SIGTERM) or restarts the service it runs in, and answers **204**. The
launcher has no permission of its own for this: it starts `nas-os-process@<action>-<id>-<started>`,
whose helper checks everything again as root. A process that ended, or whose id now belongs to
another process, is **404**; terminating a process of a unit that keeps the NAS working (launcher,
gateway, D-Bus, polkit, `systemd-*`, NetworkManager, ssh, the first process) or restarting what is
not a service is **403**; a thread of the kernel is neither. Without `NAS_PROCESSES_MODE=real` the
answer is **503**.

## Machine actions

`POST /system/actions/restart` and `/system/actions/shutdown` go through `systemctl`, which asks
logind: no shell, fixed program and a verb chosen from an enum. The answer is
`{ "accepted": true, "simulated": false }` when the machine really is going down.

**The real action runs only with `NAS_POWER_MODE=real`**, which the systemd unit of the NAS sets.
Without that variable (development, tests, a copy started by hand) the backend answers
`"simulated": true` and touches nothing. If the permissions are missing, `systemctl` is refused and
the answer is 503 with the reason, plus an error notification.

`GET /system/power` answers `{ "simulated": true }` where that variable is missing. It is for the
schedule tab, which without this answer would say the rules are simulated even on a NAS where they
really power off.

### Scheduled rules

The rules saved in `/settings/power` are not a reminder: a scheduler inside the backend executes
them. It rereads the rules every minute (and straight away on every save, so a rule set for thirty
seconds from now still fires) and for each one asks `systemd-analyze calendar` when it triggers,
instead of working out weekdays and daylight saving by hand.

The power-on is not executed: with the machine off, nothing runs. It is set **before** powering
off, by writing the time to `<StateDirectory>/wakealarm` and starting `nas-os-wake.service`,
which as root copies it into the hardware clock alarm. If that unit is missing or does not answer,
the shutdown happens anyway and a notification says what was missing. All of this only with
`NAS_POWER_MODE=real`: elsewhere the schedule records the notification and touches neither the
machine nor the clock. The installation detail is in `system/packages/ui_installation/README.md`.

**Gateway authentication** lives in the gateway, a project of its own (`../nas-gateway`), which keeps
the account and the sessions in a database of its own. The wizard creates the account there, with
`nas-os-gateway account create` on the machine: no route of the gateway creates one. `POST /auth/login` checks the credentials against the Argon2id hash and opens a
session, the opaque HttpOnly `nas_session` cookie; wrong ones are a **401**. After the refusals the
sign-in policy allows from the same address, the gateway answers **429** until its minutes are over.
`GET` and `PUT /auth/policy`, with a session, read and change that policy,
`{ enabled, attempts, lockMinutes }`: attempts 1 to 20, minutes 1 to 1440, **400** outside them;
the default is on, 5 attempts, 1 minute. A session ends after 30
minutes without requests or 12 hours after the sign-in. `GET /auth/session` answers
`{ authenticated, username }` and `POST /auth/logout` revokes it. Without an account every sign-in
is a **401**. The sign-in answers `{ username, device }`, where `device` is the name of the device
whose certificate the connection showed, or `null`.

**HTTPS and mTLS** are the gateway's too. It holds the authority of the NAS and its own server
certificate, serves HTTPS on 443 and HTTP on 80 without an automatic redirect. Session cookies
are Secure only on TLS connections. With mTLS on, a
browser without the certificate of a registered device gets a **403** page before the sign-in. The
routes, `/auth/ca.crt`, `/auth/mtls` and `/auth/devices`, are in the gateway's README; the console
does not call them, the security center reaches them through the shell
(`src/api/securityDevices.ts`). Issuing a certificate and switching mTLS take the password once
more, checked with `/auth/confirm`.

On the NAS the gateway is the entry, on port 443: it serves the built interface, answers `/api/*`
with **401** `sign in first` without a live session and forwards it to the launcher with one,
writing the user in `X-NAS-User` after dropping whatever the browser sent under that name. It
does the same for `/apps/*`, taking the session cookie out of the request and dropping any
`Set-Cookie` that would replace it. A launcher that does not answer is a **502**. Two API routes pass without
a session, because they are needed before anybody has signed in: `GET /api/setup`, which decides
between the page that waits for the first configuration and the sign-in, and
`/api/access/failures`, which reports a refused sign-in. `POST /api/setup/complete` is not among
them: the wizard calls it on the loopback, without the gateway. In development Vite
forwards `/api` to the launcher directly, so the API answers without a session there.

**Signing out is not here.** The session belongs to the authentication gateway: the frontend calls
its logout route, configured in `vite.config.common.ts` (`NAS_GATEWAY_LOGOUT`, `/auth/logout` by
default). To the launcher, `/system/actions/logout` answers 400.

## Launcher updates

The launcher updates itself **through the same machinery as an application package**: the same
forge client, the same checksum, the same operation in the widget. Where it looks is a line of the
systemd unit (`NAS_LAUNCHER_REPOSITORY`, with `NAS_LAUNCHER_PROVIDER` and `NAS_LAUNCHER_ASSET`)
so changing it is not a release.

```json
{
  "components": [
    {
      "component": "launcher",
      "currentVersion": "1.0.0",
      "availableVersion": "1.2.0",
      "repository": "ValerioGc/nas-launcher"
    },
    {
      "component": "gateway",
      "currentVersion": null,
      "availableVersion": "0.1.15",
      "repository": "ValerioGc/nas-gateway"
    }
  ],
  "tokenConfigured": true
}
```

The launcher, the gateway and the wizard are checked and updated together: a component lists
only when it is installed, and `currentVersion` is `null` when it cannot say its version.

`GET /launcher/updates` answers with what is already known and reaches nowhere.
`POST /launcher/updates/check` asks the repository of every component. `POST /launcher/updates`
starts the update of every component with a newer release and answers with its operation, like an
application install does.

The last step is the one that differs, because **a running service cannot replace itself**: the
release packages are fetched, checked (and here a missing checksum is refused outright, not warned
about) and left in the state directory, then `nas-os-update.service` is started without waiting.
That unit runs as root: it stops the launcher (and the gateway, when it changes), backs up the
databases, the configuration and the installed files, installs the wizard, the gateway and the
launcher with the installer of each package, waits for them to answer `/health` and reboots the
machine when the launcher changed; on any failure it restores the backup and restarts the previous
launcher and gateway instead. The polkit rule installed by `system/packages/ui_installation/` allows exactly
that one unit and exactly `start`.

The operation of the update goes through `downloading-<component>` for each component, then
`installing` (done, 100): from there on the launcher is stopped and the machine reboots, so the
interface waits for it to answer again. `tokenConfigured` says whether the credential of a private
repository is there; the token itself is never returned and never accepted by the API.

## System packages

```json
{
  "packages": [{ "id": "bash", "current": "5.2.37-1", "version": "5.2.37-2" }],
  "running": null,
  "manageable": true
}
```

`GET /system/updates` reads the upgrades with `apt list --upgradable`, which needs no privilege, and
answers **503** where apt is not there. `running` is `refresh`, `upgrade` or `null`; `manageable` is
false without `NAS_APT_MODE=real`, and then the two writes answer **503**.

`POST /system/updates/refresh` refreshes the index and `POST /system/updates` with `{ "ids": [...] }`
installs the upgrades of those packages. Both answer at once with the same body, `running` set, and
the job goes on in `nas-os-apt@refresh.service` or `nas-os-apt@upgrade.service`, as root, and in the
register of operations. One job at a time: a second one answers **409**. A package apt does not
offer as an upgrade answers **400**.
