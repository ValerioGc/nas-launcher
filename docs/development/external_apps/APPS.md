# Writing an application for this NAS

Written for whoever builds an application that the launcher installs and shows. It is the contract:
what an application has to be, how it is packaged and released, and which APIs of the launcher it
may call.

**An application is a separate process, in a separate repository.** It is not a plugin, it is not
compiled into the launcher, and the launcher never imports its code. Updating it does not mean
releasing the launcher.

**You install it through the launcher.** You publish a release, an entry in the catalogue points at
it, and the application manager fetches, checks, unpacks and registers it.

---

## What an application is

|                      |                                                                                                                                                                                                                                  |
| -------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Identity**         | An `id` of ASCII letters, digits, `-` and `_`, at most 64 characters. It is the key everywhere: the catalogue, the register, the directory, the unit and the port the gateway gives you. **Choose it once and never change it.** |
| **Shape**            | One process that serves its own HTTP interface and its own API over a unix socket.                                                                                                                                               |
| **Where it lives**   | `/var/lib/nas-os/apps/<id>/`, unpacked from your release.                                                                                                                                                                        |
| **Where it answers** | `/run/nas-os/apps/<id>.sock`, and nowhere else.                                                                                                                                                                                  |
| **How it is opened** | The gateway gives you a port of your own on the address of the NAS, the launcher proxies it onto that socket, and the shell shows it in a frame.                                                                                 |
| **Who runs it**      | The launcher, through one systemd template: `nas-app@<id>.service`. See [Running it](#running-it).                                                                                                                               |
| **Who it talks to**  | The launcher, over HTTP. Nothing else on the NAS.                                                                                                                                                                                |

### The manifest

`app.system` is an optional boolean, defaulting to `false`. It classifies an application as a
system app independently of `app.category`. External system apps retain their own process,
package updates and management. The catalogue can also mark an entry with `system`.
Its `preinstall` flag selects initial installation and creates a desktop shortcut during setup,
independently of the preference for later interactive installs.

Put a `nas-app.json` at the root of your package. The launcher reads it at every start, and a
package without a readable one is a package it will not open.

```json
{
  "manifest_version": 1,
  "app": {
    "id": "fileExplorer",
    "name": "Esplora file",
    "version": "1.2.0",
    "icon": "icon.svg",
    "category": "storage"
  },
  "run": {
    "exec": "bin/file-explorer",
    "socket": "/run/nas-os/apps/fileExplorer.sock"
  },
  "ui": { "protocol": 1 },
  "provides": [{ "capability": "search", "version": 1, "endpoint": "/api/search" }],
  "permissions": { "paths": ["/srv/storage"] }
}
```

`app.id` has to be the name of the directory the package unpacks into, which is the `id` of the
catalogue entry. The launcher checks it: a package that claimed another application's id would be
handed that application's socket and that application's place in the shell.

**`run.socket` is documentation, not configuration.** The launcher derives the path from the
identifier (`/run/nas-os/apps/<id>.sock`) and connects to that and nothing else. Declare
the same path or leave the field out; anything else is written to the log and ignored.

`app.icon` is a file inside your package (a symlink that leads out of it is refused) and the
launcher serves it at `/api/apps/<id>/icon`: it is the icon the menu and the desktop draw. `svg`,
`png`, `jpg`, `webp` or `ico`, under 512 KiB.
Without one the shell falls back to its own default logo.

`app.category` optionally declares the default menu category. Allowed values are `media`,
`storage`, `network`, `system` and `development`. The catalogue entry takes precedence, including
the choice of no category. `system` groups an external application under System without making it a built-in
application or preventing its removal. Older `files`, `security` and `utilities` categories are
read as `system`; Explorer's former `files` catalogue entry is read as `storage`.

`provides` declares the services you offer the shell, each with the path that answers it. The shell
calls that path directly, at `/apps/<id><endpoint>`, so it sits outside `/internal/`: a path under
it, one with `..` or one that is not absolute is ignored.

`permissions.paths` asks for what the sandbox hides. `nas-app@.service` hides from every
application the data disks (`/srv`, `/mnt`, `/media`), the logs, the state of the launcher except
your own package, `/sys`, `/run/udev` and the processes of other users. The launcher grants, without
asking, only these paths, exactly as written:

| Path           | What you get                                                                                                     |
| -------------- | ---------------------------------------------------------------------------------------------------------------- |
| `/srv/storage` | the data disks, read and written in the `nas-storage` group; `NAS_STORAGE_ROOT`, `NAS_STORAGE_REGISTRY`          |
| `/proc`        | all of `/proc`, including process metadata allowed by Unix permissions (`ProcSubset=all`, `ProtectProc=default`) |
| `/sys`         | `/sys`, read only                                                                                                |
| `/run/udev`    | the udev database, read only                                                                                     |

A package asking for any other path is not installed and leaves nothing on disk. What was granted
is written in `app-grants/<id>` beside the database, and `nas-os-app-grants@<id>.service` turns it
into `nas-app@<id>.service.d/grants.conf` through `apply-app-grants.sh`, which knows the same list
and refuses anything else. The grant follows the manifest at every update and goes with the
application.

`permissions.network` asks for outgoing traffic beyond HTTP and HTTPS. `smtp` puts the application
in the base firewall's set `nas_apps_smtp`, which may reach TCP 25, 465 and 587 to send mail.
`sync` puts it in `nas_apps_sync`, which reaches only the local network on the ports of Syncthing
(TCP and UDP 22000, UDP 21027, with the broadcast and the multicast of the local discovery); the
application then says on its socket whether it has a sync on, `PUT /api/apps/self/sync` with
`{ active }`, and the launcher opens those ports to the local network meanwhile (set `nas_sync`,
through `nas-os-sync-ports.service`), closing them when no application has one.
`permissions.packages` asks for Debian packages on the machine, from a fixed list: `msmtp`,
`clamav`, `clamav-freshclam`, `7zip`, `samba` and `syncthing`. The launcher has them installed by
`nas-os-app-packages@<id>.service` before the application takes its place, and purged after it is
removed, unless another application still asks for them or they were on the machine before. A
package asking for any other value is not installed either.

The shell uses `search` for the file search in the global search:

- a `GET` with `q` (the text), and `type` and `ext` when the file filters are set
- an answer of `200` with a JSON array of results within 10 seconds, or the shell names you among
  those that did not answer
- each result carries `id`, `name`, `path`, `type` (`folder`, `document`, `image`, `audio`, `video`,
  `archive`), `extension`, `size` in bytes, `modified` (ISO 8601) and `target`, the query string
  your page opens on when the result is chosen (`?path=%2Fdocs`); a `target` that does not start
  with `?` opens the page as it is

The shell also uses `file-picker` version 1. A provider declares `endpoint: "/"` (or another
public UI path ending in `/`) in `nas-app.json`. When an app asks to select a file, the shell opens
the first installed provider's UI in a dialog at `/apps/<provider><endpoint>?nasPicker=<token>`;
`multiple=1` is also passed when the caller accepts several files. Optional `extensions`
(dot-prefixed extensions, up to 32) and `directory` (the initial logical path) are forwarded
as query parameters. The shell also validates returned file names against the requested extensions. The provider shows the files it
owns and sends the selection back through the frame protocol below. The selected file has a logical
path and a relative `api/...` download URL served by that provider. The shell resolves that URL
under `/apps/<provider>/` before returning it to the caller; the caller can fetch it with its
existing NAS session. A cancelled selection returns no files. With `folder=1` the provider lets
the user choose one folder instead of files: `multiple` and `extensions` are not forwarded, and the
folder comes back with its logical path and no download URL. nas-file-manager is the current
provider.

The shell also uses `network-disks` version 1, the network shares mounted on the NAS. A provider
declares an API endpoint (not ending in `/`) that answers a `GET` with a JSON array of at most 64
`{ name, mount, source, read_only }`: `mount` the absolute path the share is mounted on, `source`
the remote share or `null`. When an app asks for them, the shell starts the first installed
provider if it is stopped, reads the endpoint and answers with the list. nas-disk-manager is the
current provider, at `/api/network-disks`.

The shell also hands some of its events to `mail-events` version 1. The provider declares an
endpoint under `/internal/` (nas-mail-client uses `/internal/events`), because only the launcher
calls it, over the application's socket, starting the application when it is not running. Each
event is a `POST` of `{ key, title, message, params, at }`, the same notice the log keeps, with
`at` in Unix milliseconds: `signInRefused` (`from`, `field`), sent even when the notifications of
refused sign-ins are off; `launcherUpdateAvailable` (`version`, `current`), once per version;
`componentUpdateAvailable` (`component` `gateway` or `wizard`, `version`, `current`), once per
version; `launcherUpdated`, `nasUpdated` (`components` updated together, `version`),
`launcherRolledBack`, `launcherUpdateFailed` and `launcherUpdateInterrupted`
(`version`, `reason`); `appUpdatesAvailable` (`count`, `apps` the names), only from the automatic
check of the Application manager, once per set of new releases; `systemUpdatesAvailable` (`count`, `security`, `stable`, `other`, and
`packages`, one `name current -> version (level)` per line). While a provider is installed, the
launcher looks for a launcher update and for system upgrades every six hours, so these events do
not wait for someone to open the settings. A level is `security` for an upgrade from
`<release>-security`, `stable` for the release and its `-updates`, `other` for anything else.

The same endpoint receives `accountCode` (`to`, `purpose` `verify` or `reset`, `code` of six
digits), the code the gateway mails for the recovery address of the account. The provider sends it
to `to` at once and answers with the outcome (`2xx`, or an error with `{ error }`), since the user
waits for it. The gateway asks for it with `POST /internal/mail/code` on the launcher's loopback, a
path it never forwards from a browser; the settings learn whether the provider can send from
`GET /api/mail/status` (`{ installed, sender }`), which asks the provider's `api/status`.

An application that keeps caches, data it can make again from what it holds (previews, indexes),
declares `cache` version 1 with an endpoint under `/internal/` (nas-file-manager uses
`/internal/cache`). A `GET` answers `{ caches: [{ id, label, bytes, items }] }`, `id` an identifier
of the cache, `label` the name a person reads; `DELETE <endpoint>/<id>` empties one and answers
`2xx`, or `404` for an id it does not know. The launcher lists them all, with the archive of the
antivirus scans it keeps itself, in `GET /api/caches` (`[{ app, id, label, bytes, items }]`), and
empties one with `DELETE /api/caches/{app}/{id}`, starting the application when it is stopped.
A trash is declared apart, as `trash` version 1 with an endpoint under `/internal/`: `GET` answers
`{ label, bytes, items }`, `DELETE` empties it; the list shows it with `kind: "trash"` and id
`trash`. Never declare as a cache what cannot be made again: settings, databases of the user's data, the
state a rule depends on.

Everything under `/internal/` is for the launcher only: the proxy answers 403 to a browser asking
for it. Put your interface on one side of that line and the services you offer the launcher on the
other.

---

### Installed, by hand

**A directory with a manifest is an installed application.** The launcher reads
`/var/lib/nas-os/apps/*/nas-app.json` at every request, so unpacking your package there is
all it takes to see it in the menu: no installation, no catalogue, no release. It is how you try a
build before publishing one:

```bash
mkdir -p /var/lib/nas-os/apps/fileExplorer
tar --extract --file file-explorer-1.2.0-linux-x64.tar.gz \
  --directory /var/lib/nas-os/apps/fileExplorer
```

The directory name has to be your `id`, which is what the manifest has to declare too. At the next launcher start the manifest is also recorded in SQLite. The version comes from the
manifest; the launcher does not infer which release asset produced the files.

---

## Being opened

The launcher is a reverse proxy in front of you. A browser asking for
`http://<address of the NAS>/apps/<id>/…` reaches the gateway, which checks that the user signed
in and hands the request to the launcher, which takes the prefix off, connects to
`/run/nas-os/apps/<id>.sock` and hands your answer back. The shell then shows that address in a
frame. Nothing on the network has to know about it: it works wherever the launcher does, one
forwarded port included.

Your application shares the origin of the console: the frame is not a boundary, and the
JavaScript inside could reach the parent document and the session. That is why only applications
the administrator trusts are installed, and why your page keeps to its own paths.

What that asks of you:

- **Serve at your own root, and link relatively.** Your server sees `/` and `/assets/...`: the
  prefix is taken off before the request reaches you, and handed over in `X-Forwarded-Prefix`. The
  browser, though, is at `/apps/<id>/`, so every address your page writes has to be relative:
  `base: './'` in Vite, `fetch('api/items')` rather than `fetch('/api/items')`. An absolute one
  reaches the launcher, not you. A redirect of yours to `/login` is put back under the prefix.
- **Listen on the socket, on nothing else.** It is handed to you as `--listen`; the directory is
  there already and the permissions are the template's business, not yours.
- **Keep the path short.** A unix socket path is a fixed field of 108 bytes in the kernel: worth
  knowing when you point `NAS_APP_SOCKETS` at a development directory.
- **Keep to your frame: copy `src/services/frameGuard.ts` and call `guardFrame()` first in your
  `main.ts`.** The frame has no `sandbox`: on the origin of the console one that runs scripts and
  keeps the origin is no wall, and the browser warns about it. The guard keeps the rules it had,
  in the frame only: `window.open` gives `null`, `alert` does nothing, `confirm` gives `false` and
  `prompt` gives `null`; a link or a form to `_top`, `_parent`, `_blank` or a window that is not
  one of your frames goes nowhere (a `download` link still downloads). Do not navigate
  `top.location` or `parent.location` either. Opened on its own, your page is not touched.
- **Do not refuse to be framed.** `X-Frame-Options: DENY` or `frame-ancestors 'none'` means a blank
  window in the shell. If you set the header, allow the launcher's origin.
- **Keep your state out of cookies, and name what you store.** You share the origin with the
  console and every other application, so their cookies, `localStorage` and `IndexedDB` are the
  same as yours: prefix your keys with your `id`. The cookie of the console's session is taken out
  before a request reaches you, and a cookie of yours with its name is dropped.
- **A WebSocket goes through.** Open it on a relative address (`new WebSocket(new URL('live',
location.href.replace(/^http/, 'ws')))`): the gateway and the launcher carry the upgrade to your
  socket, with the same session check as any other request. Any other upgrade is a 501.
- **Take who is asking from `X-NAS-User`.** The launcher writes that header on every request it
  forwards and drops whatever arrived under that name, so it is the only word on the subject worth
  reading. Its value is the name of the user signed in to the NAS; a request that reaches the
  launcher without the gateway in front, as in development, says `local`.

If your application is installed but not listening, the launcher answers 503 and names you.

### Running it

The launcher starts and stops you. There is one systemd template on the NAS, `nas-app@.service`,
and your application runs as an instance of it: `nas-app@fileExplorer.service`, started when the
package is installed, restarted when it is updated, stopped when it is removed, and started again
whenever somebody presses the button in the application manager. You do not write a unit and you do
not enable anything.

What the template does with your package:

```
ExecStart   run-app.sh <id>, which reads run.exec out of your manifest and executes it
            with --listen /run/nas-os/apps/<id>.sock
Working dir /var/lib/nas-os/apps/<id>
User        one systemd makes up for this instance: it owns nothing on the machine
Writable    /run/nas-os/apps for the socket, and $STATE_DIRECTORY for you
Restart     on-failure, after two seconds
Runtime     TOKIO_WORKER_THREADS=2 and MALLOC_ARENA_MAX=2, no memory both writable and executable
```

So three things are asked of your program:

- **Accept `--listen <path>`** and bind that unix socket. The same path also arrives as
  `NAS_APP_SOCKET`, and your identifier as `NAS_APP_ID`.
- **Keep `run.exec` inside the package.** It is relative to your directory; an absolute path or one
  that climbs out of it is refused and the unit does not start.
- **Write only into `$STATE_DIRECTORY`**, which is `/var/lib/nas-app/<id>`, created for you with
  mode `0700`. Everything else on the machine is read-only for the process, `/tmp` included: it is
  private and thrown away when you stop.
- **Stay small.** An application serves one person through the launcher: a tokio runtime gets two
  workers from `TOKIO_WORKER_THREADS`, glibc two arenas, and `MemoryDenyWriteExecute` refuses a
  JIT. Heavy work belongs in `spawn_blocking`, which these do not limit.
- **Use HTTP or HTTPS for outbound connections.** The base firewall permits TCP 80 and 443 for
  application units, plus DNS and NTP. Other outbound ports need a future per-app permission.

`systemctl status nas-app@<id>` and `journalctl -u nas-app@<id>` are where your logs are: write to
stdout and stderr, not to a file of your own.

#### Working without your window

When its window closes, the shell asks the launcher to close the application
(`POST /apps/<id>/close`), which stops it. An application whose work does not wait for the person,
backups at their hour or the mails of the NAS, declares `"run": { "background": true }` in its
manifest and then says by itself whether it has work to do now:

```bash
curl -s --unix-socket "$NAS_LAUNCHER_SOCKET" -X PUT http://nas/api/apps/self/background \
  -H 'Content-Type: application/json' -d '{ "needed": true }'
```

Say it whenever the answer changes (a rule turned on or off, notifications configured) and once
when you start. While the last answer is `true`, the launcher starts you at boot and leaves you
running when your window closes; with `false`, or without the declaration, you are stopped with
your window like any other application. The request names no application: the launcher knows you
by your cgroup, and refuses it (403) when your manifest does not declare `run.background`.

The declaration also gives your unit a lower priority (`Nice=10`, `CPUWeight=50`, I/O
`best-effort` 7): what you do in background never slows down what the person is doing. You still
stop like any other unit, with `systemctl stop nas-app@<id>`.

When a session begins or ends, at the sign-in and at the sign-out, the shell closes every window
(`POST /apps/close`): the launcher stops every application its windows started, and leaves running
only those that declared work in background.

#### Sharing folders on the local network

An application granted the data disks (`permissions.paths: ["/srv/storage"]`) may share folders
on the local network with Samba, when its manifest also declares `"permissions": { "shares": true }`
and asks for the package `samba`. It sends the whole list at once, on its socket, with the path the
machine knows each folder by:

```bash
curl -s --unix-socket "$NAS_LAUNCHER_SOCKET" -X PUT http://nas/api/apps/self/shares \
  -H 'Content-Type: application/json' \
  -d '{ "shares": [{ "name": "Foto", "path": "/srv/storage/<volume>/Foto",
                     "writable": false, "protected": true }] }'
```

A name has letters, digits, spaces and `_ - .` (64 at most, none of Samba's own sections); a path
is a folder under `/srv/storage`, without `..`, `%` or a segment with a space at either end. A list
the launcher refuses is not applied (400, with the reason). The launcher writes the list in
`smb-shares/<id>` beside its database and starts `nas-os-smb-shares.service`, whose helper
(`apply-smb-shares.sh`, as root) checks every line again, writes `/etc/samba/smb.conf` (what Debian
had there goes to `smb.conf.nas-os-original`), keeps `smbd` running only while a folder is shared
and puts TCP 445 in the set `nas_shares` of the base firewall, open to the local network only. Each
share opens as the user of the account, in the group of the data disks; a protected one asks for
that user and the password of the account, which the gateway hands to Samba
(`PUT /auth/account/share-password` from the page): the launcher and the application never see it.
`GET /api/apps/self/shares` answers `{ available, user, recycle }`: whether Samba is installed, the
user a computer signs in with, and whether a share may carry its `trash`
(`"<disk>/.nas-trash/<folder>"`, the place of the folder in the trash of its own disk), where Samba
then moves what is deleted from it, keeping the tree and one version for each path. Send the list again when you start: the launcher applies it only when it
changed. Removed, an application takes its shares with it.

#### Cleaning the system

An application whose manifest declares `"permissions": { "cleanup": true }` (nas-cleanup-manager)
may measure and clean the disk of the system, one item of a fixed list at a time, on its socket:

```bash
curl -s --unix-socket "$NAS_LAUNCHER_SOCKET" -X POST http://nas/api/apps/self/cleanup \
  -H 'Content-Type: application/json' \
  -d '{ "action": "preview", "item": "rotated-logs",
        "options": { "temporaryDays": 7, "journalMb": 500 } }'
```

`action` is `overview` (no item: the size and free room of the system disk, the bytes of the
downloaded packages, the logs, the temporary files and the applications when they sit on it, and
the data disks of the registry), `preview` (`{ item, bytes, entries, more }`, deleting nothing) or
`clean` (the same, with what it freed and what went). The items are `apt-archives` (`apt-get
clean`), `apt-obsolete` (`apt-get autoclean`), `apt-orphans` (`apt-get autoremove`), `apt-configs`
(`dpkg --purge` of the packages left in state `rc`), `old-kernels` (all but the running one, any
newer one and the newest before it), `rotated-logs` (`/var/log`, the journal aside), `journal`
(`journalctl --vacuum-size`), `temporary` (`/tmp` and `/var/tmp`, older than the days given, what
programs keep there aside), `nas-backups` (the backups of the updates but the newest) and
`nas-leftovers` (releases and versions of applications an update left, after a day); the last two
are refused while an update runs. The application sends only the item and two numbers: the
launcher writes the request beside its database and starts `nas-os-cleanup.service`, where
`nas-os-axum --cleanup-job` checks it again and runs the fixed command as root. One cleanup at a
time. The same permission reads and empties the caches of the NAS, `GET /api/caches` and
`DELETE /api/caches/{app}/{id}`. Without it all three answer 403.

---

## Talking to the shell

Your page shares the origin of the console, but it does not call the launcher's API: an absolute
`/api/...` would reach the launcher, a relative `api/...` reaches your own server. What it has
instead is one message channel with the shell, `postMessage`, with the origin checked on both
sides.

Every message carries `nas: 1`. Anything else arriving in your window is not the launcher.

```js
let shell = null;

window.addEventListener('message', (event) => {
  if (event.source !== window.parent || event.origin !== window.location.origin) return;
  const message = event.data;
  if (!message || message.nas !== 1) return;

  if (message.type === 'context') {
    shell = event.origin;
    document.documentElement.dataset.theme = message.context.theme; // 'light' or 'dark'
    applyAccent(message.context.accent); // a CSS colour, or '' for the theme's own
    applyPalette(message.context.palette); // the colours of the theme, see below
    setLanguage(message.context.locale); // 'it' or 'en'
  }
});

// The launcher serves the iframe on the same origin. Standalone pages send no ready message.
if (window.parent !== window) {
  window.parent.postMessage({ nas: 1, type: 'ready' }, window.location.origin);
}
```

`palette` carries the colours of the theme chosen in the settings for that type, by role:
`background`, `foreground`, `surface`, `sunken`, `hover`, `border` and `muted`. Put each one in your
`--nas-<role>` token on the root, and take it away when it is missing or not a colour: the
applications of nas-os draw every surface from those tokens, so they follow the theme of the
launcher, and keep their own colours outside it.

The shell sends `context` when your frame loads, whenever you say `ready`, and again whenever the
theme, the theme of the type, the accent or the language changes. Two things you may send back:

| Message                                                                                  |                                                                                                                                                                                            |
| ---------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `{ nas: 1, type: 'notify', title, message?, tone?, log? }`                               | Leaves a notification. It is signed with your identifier by the shell, from the frame it arrived in, never with a field you send.                                                          |
| `{ nas: 1, type: 'open', app: '<id>', location?: '?path=...' }`                          | Opens another installed application in the shell, optionally at a query string within that app. An identifier nobody installed opens nothing.                                              |
| `{ nas: 1, type: 'open-https' }`                                                         | Only from nas-security-center, on a console served over HTTP: the launcher loads itself again over HTTPS on the same host. The frame guard forbids an application to navigate the console. |
| `{ nas: 1, type: 'describe', notifications: [...] }`                                     | Asks for notifications you read from `/api/notifications` in the language of the page. Answered with `described`, see below.                                                               |
| `{ nas: 1, type: 'shortcut', requestId, path, name }`                                    | nas-file-manager asks the launcher to save a folder shortcut. The launcher replies with `shortcut-result` after persistence.                                                               |
| `{ nas: 1, type: 'pick-files', requestId, multiple?, extensions?, directory?, folder? }` | Opens the first installed `file-picker` provider, on folders with `folder: true`. The shell answers with `pick-files-result` (files, cancelled, optional error).                           |
| `{ nas: 1, type: 'network-disks', requestId }`                                           | Any app: the network shares of the first installed `network-disks` provider. Answered with `network-disks-result`.                                                                         |
| `{ nas: 1, type: 'power-schedule', requestId }`                                          | Any app: the power schedule of the NAS, to plan work while it is on. Answered with `power-schedule-result`.                                                                                |
| `{ nas: 1, type: 'diagnostics', requestId }`                                             | nas-log-manager only: the warnings and errors of the launcher's log. Answered with `diagnostics-result`.                                                                                   |
| `{ nas: 1, type: 'diagnostic-log', requestId, notification }`                            | nas-log-manager only: the log behind one notification (its id, digits only). Answered with `diagnostic-log-result`.                                                                        |
| `{ nas: 1, type: 'remove-diagnostic', requestId, notification }`                         | nas-log-manager only: removes one notification. Answered with `remove-diagnostic-result`.                                                                                                  |
| `{ nas: 1, type: '<access request>', requestId, payload? }`                              | nas-security-center only: the access of the NAS. See below.                                                                                                                                |
| `{ nas: 1, type: '<monitor request>', requestId, payload? }`                             | nas-system-monitor only, window or widget: the telemetry of the machine and the actions on its processes. See below.                                                                       |

The requests of nas-security-center are `access-settings`, `save-access-settings`
(`{ notifyFailures }`), `access-rules`, `add-access-rule` (`{ kind, value, label }`),
`remove-access-rule` (`{ id }`, digits only), `sign-in-policy`, `save-sign-in-policy`
(`{ enabled, attempts, lockMinutes }`) and `refused-sign-ins` (the notifications `signInRefused`,
as `{ id, at, from, field }`). The devices allowed in with mTLS go through the gateway's routes:
`mtls-state`, `save-mtls` (`{ enabled, password }`), `devices`, `add-device`
(`{ name, icon, password }`), `update-device` (`{ id, name, icon }`), `renew-device`
(`{ id, password }`), `revoke-device` and `remove-device` (`{ id }`). A name takes 1 to 60
characters, an icon is one of `tv`, `desktop`, `laptop`, `phone` and `projector`. Where a password
is sent, the shell checks it with the gateway first and passes on only the code it gets back:
`add-device` and `renew-device` answer `{ device, credential: { file, fileName, password } }`, the
PKCS#12 file in base64 and its password, shown once. The certificate HTTPS answers with goes
through the gateway's routes too: `certificate` reads it, `save-certificate-names`
(`{ names, password }`, at most 20 names), `renew-certificate` (`{ password }`),
`upload-certificate` (`{ certificate, key, password }`, both in PEM) and
`remove-custom-certificate` (`{ password }`) change it, and each answers the new state. The shell
checks the payload again in `src/api/access.ts`, `src/api/securityDevices.ts` and
`src/api/securityCertificates.ts` and calls the same routes as the console; a malformed request
gets no answer.

nas-system-monitor reads telemetry from its own `api/telemetry` endpoint. Its window, widget,
preferences, CPU/process classification and hardware sensors belong to the application.
Its manifest requests `/proc` and `/sys`; the launcher grants those kernel views without
capabilities. Only `process-action` (`{ id, started, action }`, `terminate` or `restart`)
is delegated to `POST /api/system/processes/{id}` and independently authorized before the root
helper acts. Terminating a NAS application process stops its whole service and waits for it to
stop. The response data contains `{ appId }`, identified from the process cgroup; the shell
removes that application's tab and iframe after success. `appId` is `null` for a restart or a
process outside a NAS application. The legacy `system-telemetry` request forwards to the application's endpoint.
The shell opens `/apps/nas-system-monitor/?view=widget` in its drawer only while the application
is installed, with the same context as its window. The drawer is a host for the iframe.

The three requests of nas-log-manager, those of nas-security-center, those of nas-system-monitor,
`network-disks` and `power-schedule` are answered with
`{ nas: 1, type: '<request>-result', requestId, success, data?, error? }`: the shell does them with
the session of the console, so the application never needs the launcher's API for what it shows.
The `data` of `network-disks-result` is `[{ name, mount, source, readOnly }]`.
The `data` of `power-schedule-result` is `{ enabled, rules: [{ day, time, action }] }`: only the
active rules, with `day` one of `everyday`, `weekdays`, `weekends` or a weekday, `time` as `HH:MM`
and `action` one of `start`, `shutdown` and `restart`. The rules apply only while `enabled` is true.

The texts of the launcher's own notifications are translated by the shell: the API gives their
English text, a `key` and its `params`. Send what you read in `describe`, and the shell answers
`{ nas: 1, type: 'described', notifications: [{ id, title, message, source }] }`, with the title and
the text in the language of the page and the source named as the user knows it.

The shell may open you on a page of your own: nas-log-manager, for instance, is opened at
`/apps/nas-log-manager/?notification=12` from a notification. Read your query string when you start.
To open an Explorer result in another app, send `location` beginning with `?` and URL encode the
file path in it. The shell rejects locations longer than 2048 characters or with another prefix.

nas-file-manager declares the `search` capability at `/api/launcher/search`. It accepts `q`,
optional `type` and `ext`, and returns logical paths and an encoded `target` query. The launcher
starts a stopped provider through the existing lifecycle before retrying a failed search. Explorer
keeps `/api/search?query=` for its own UI. A file result opens its parent folder with `select`,
while a folder result opens the folder itself. Explorer may send `shortcut` only from its registered
iframe. The launcher stores the logical path and name using `POST /api/desktop/folders`, then sends
`shortcut-result` with the matching request ID. `DELETE /api/desktop/folders` removes the desktop
entry without deleting the folder. The desktop uses typed `{ kind, target }` keys for mixed app and
folder ordering.

For `file-picker`, the provider's frame sends
`{ nas: 1, type: 'files-picked', token, files: [{ name, path, download }], cancelled? }`. `token`
is the value of `nasPicker` in its URL; `download` must be a relative `api/...` URL. Only the
registered picker frame on the shell origin can answer. The shell returns to the requesting frame
`{ nas: 1, type: 'pick-files-result', requestId, files: [{ name, path, url }], cancelled, error? }`.
`url` is an absolute URL on the NAS, scoped to the provider; a chosen folder has no `download` and
no `url`. A missing provider or a startup
failure yields an empty `files` array and an `error`. At most one picker is open at a time, with a
maximum of 100 files in a multiple selection. The provider itself offers a Cancel action, and the
shell also has one if its frame fails to load.

An app runs inside an iframe, so the console's CSS does not style its page. Each app packages its
own copy of the shared surface rules from `src/apps/styles/_external-apps.scss` and applies
`app_surface` to its root. The four NAS apps keep this copy in `src/styles/_nas-apps.scss`.

Validate `event.source` and the same origin before processing a message. Answer to
`window.location.origin`, never to `'*'`.

---

## Packaging

A package is **a tar archive** with your application inside it. Nothing else: no installer, no
script that runs as root, no `.deb`.

```
file-explorer-1.2.0-linux-x64.tar.gz
├── nas-app.json
├── bin/file-explorer
├── icon.svg
└── …whatever else your application needs
```

Paths inside the archive are relative. Absolute paths and `..` are refused: the launcher unpacks
with `tar`, which does not allow a package to escape its own directory.

### Building and releasing

```bash
# 1. Build the two halves into one tree
npm run build                       # your interface, into dist/
cargo build --release               # your binary

# 2. Lay out exactly what the NAS will have
mkdir -p package/bin
cp target/release/file-explorer package/bin/
cp -r dist package/
cp nas-app.json icon.svg package/

# 3. One archive, named with the version, plus its checksum
VERSION=1.2.0
ASSET=file-explorer-$VERSION-linux-x64.tar.gz
tar --create --gzip --file "$ASSET" --directory package .
sha256sum "$ASSET" > "$ASSET.sha256"

# 4. Publish both as assets of a release tagged v$VERSION
gh release create "v$VERSION" "$ASSET" "$ASSET.sha256"
```

Three rules the launcher relies on:

- **The tag is the version.** `v1.2.0` and `1.2.0` are the same thing; versions are compared
  number by number, so `1.10.0` is newer than `1.9.0`.
- **The asset name carries the version.** The catalogue names it as a template,
  `file-explorer-{version}-linux-x64.tar.gz`, and `{version}` is the tag without its `v`.
- **Publish the checksum beside it**, as `<asset>.sha256`, in the `sha256sum` format. Without one
  the package still installs, but the launcher records a warning saying it could not be checked.
  With one, a package that does not match is thrown away and nothing is written.

> A checksum proves the file arrived intact. It does **not** prove who made it: whoever can publish
> the release can publish the checksum. The account that owns the repository is the trust boundary,
> so protect it.

### Where the launcher gets it

An entry in `/etc/nas-os/catalogue.json` on the NAS: which is a file, editable by hand or
by the administrator, not something compiled into anything:

```json
{
  "version": 1,
  "apps": [
    {
      "id": "fileExplorer",
      "name": "Esplora file",
      "info": "Sfoglia i file del NAS.",
      "provider": "github",
      "repository": "ValerioGc/nas-file-explorer",
      "asset": "file-explorer-{version}-linux-x64.tar.gz"
    }
  ]
}
```

`provider` is `github` or `gitlab`, and `preinstall` marks the applications the NAS installs by
itself right after the setup wizard. There is one release token, set in the setup wizard, in the
App Manager or with `sudo /usr/local/lib/nas-os/configure.sh`; systemd stores it encrypted
and passes it to the launcher, which sends it on every request to the forge, for the launcher and
for every application alike, and nowhere else.

---

## The APIs you may call

Your **process** has no loopback: the unit sets `IPAddressDeny=localhost`, because the launcher on
`127.0.0.1:3000` answers whoever reaches it. It reaches the launcher on a unix socket of its own,
whose path the unit puts in `NAS_LAUNCHER_SOCKET` (`/run/nas-os/launcher/apps.sock`):

```bash
curl -s --unix-socket "$NAS_LAUNCHER_SOCKET" http://nas/api/tasks
```

The launcher knows you on that socket by the unit systemd started your process in,
`nas-app@<id>.service`, and nothing you send can change it. Only the routes of this section are
there, any other path answers `404`, and a process that is not an application gets `403`. What you
write carries your identifier: a `source` that is not your `id` is refused with `403`, and so is a
change to an operation another application registered.

On a development machine there is no unit, so the socket refuses your process: call the same paths
on `http://127.0.0.1:3000` (see [Trying it without a NAS](#trying-it-without-a-nas)), for instance
when `NAS_LAUNCHER_SOCKET` is not set.

JSON in, JSON out. Every mutation needs `Content-Type: application/json`, **even with an empty
body**: send `{}`.

A failure is always a status plus `{ "error": "…" }`, and the message is meant to be shown. Every
endpoint of the launcher, with its fields and its errors, is in [docs/API.md](../API.md).

### Notifications: tell the user something happened

`POST /api/notifications`, from your process. From your page, where the API is out of reach, the
same thing is the `notify` message of [the frame protocol](#talking-to-the-shell).

```bash
curl -s --unix-socket "$NAS_LAUNCHER_SOCKET" http://nas/api/notifications \
  -H 'Content-Type: application/json' \
  -d '{
        "title": "Copia completata",
        "message": "30 GB copiati in /srv/backup.",
        "source": "fileExplorer",
        "tone": "success"
      }'
```

| Field     |                                                                                         |
| --------- | --------------------------------------------------------------------------------------- |
| `title`   | Up to 160 characters, not empty.                                                        |
| `message` | Up to 2000 characters.                                                                  |
| `source`  | **Your `id`**, or the launcher refuses it. It is what the user filters on.              |
| `tone`    | `info`, `success`, `warning` or `error`. It decides the colour and the icon.            |
| `log`     | Optional. The absolute path of a file that tells more, which nas-log-manager will open. |

The answer carries `id`, `at` (Unix milliseconds) and `read: false`. The log keeps the 500 most
recent and survives a restart.

Notifications are **text**: the launcher renders them as text and never as markup. Do not send
HTML, it will be shown as it is.

Use one for something the user should know about after the fact. For something that is _happening_,
use an operation instead.

### Operations: show what you are doing

The launcher keeps a register of work in progress, and a widget in the header shows it. **The
launcher does not run your work**: you register it, report where you are, and close it.

```bash
# 1. Register, when the work starts
curl -s --unix-socket "$NAS_LAUNCHER_SOCKET" http://nas/api/tasks \
  -H 'Content-Type: application/json' \
  -d '{
        "id": "fileExplorer-copy-4821",
        "source": "fileExplorer",
        "name": "Copia in corso",
        "detail": "0 di 30 GB",
        "progress": 0,
        "pausable": true,
        "cancellable": true
      }'

# 2. Report, as often as you like
curl -s --unix-socket "$NAS_LAUNCHER_SOCKET" -X PATCH http://nas/api/tasks/fileExplorer-copy-4821 \
  -H 'Content-Type: application/json' \
  -d '{ "progress": 42, "detail": "12 di 30 GB" }'

# 3. Close it, whichever way it went
curl -s --unix-socket "$NAS_LAUNCHER_SOCKET" -X PATCH http://nas/api/tasks/fileExplorer-copy-4821 \
  -H 'Content-Type: application/json' \
  -d '{ "state": "done", "progress": 100, "detail": "30 di 30 GB" }'
```

| Field                     |                                                                                                                                                                                         |
| ------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `id`                      | Yours to choose, unique on the NAS. Prefix it with your own id. Registering the same one again **replaces** it, so work that restarts finds its own row instead of leaving a stale one. |
| `source`                  | Your `id`, or the launcher refuses it. An `id` already registered by another application is refused too.                                                                                |
| `name`                    | What the user sees. Up to 160 characters.                                                                                                                                               |
| `detail`                  | One line underneath, up to 500. Where you are, in words.                                                                                                                                |
| `progress`                | 0–100, **or absent**. Leave it out when you cannot tell: the widget draws a bar with no position rather than a convincing zero. Send an explicit `null` to go back to not knowing.      |
| `pausable`, `cancellable` | What you agree to be asked for. Default `false`.                                                                                                                                        |

States are `running`, `paused`, `done`, `failed` and `cancelled`. **A closed operation cannot be
reopened**: register a new one instead of moving a row the user has already read. Closing with
`failed` also leaves a notification, because that is the outcome nobody should miss.

#### How pause and stop reach you

The launcher cannot pause your work. When the user presses pause or stop, it **writes the state**
and expects you to honour it:

```bash
# Read your own operation, on whatever rhythm your work allows
curl -s --unix-socket "$NAS_LAUNCHER_SOCKET" http://nas/api/tasks | jq '.[] | select(.id == "fileExplorer-copy-4821")'
```

- `paused` → stop working, keep your place, and keep the row as it is.
- `running` after a pause → carry on.
- `cancelled` → stop and clean up, then either `DELETE` the row or leave it as the record.

This is why `pausable` and `cancellable` default to `false`: a button the owner would ignore is
worse than a button that is not there, so the launcher only draws the ones you asked for.

`DELETE /api/tasks/{id}` removes one of yours. Pause, resume, stop and clearing the finished ones
are the user's buttons, not yours, and are not on your socket.

### Reading the machine

These are read-only and need nothing from you.

|                                   |                                                                                                                                |
| --------------------------------- | ------------------------------------------------------------------------------------------------------------------------------ |
| `GET /api/apps`                   | What is installed, including your own version once you are.                                                                    |
| `GET /api/system/telemetry`       | Compatibility forward to System Monitor’s own telemetry endpoint.                                                              |
| `GET /api/system/inotify`         | `{ maxUserWatches }`: the inotify watch limit of each user, which your sandbox cannot read in `/proc/sys`.                     |
| `GET /api/network`                | How the NAS is connected.                                                                                                      |
| `GET /api/notifications`          | The log, with filters.                                                                                                         |
| `GET /api/notifications/{id}/log` | The log behind a notification: the file it names, under `/var/log`, or the journal of the unit that wrote it around that time. |
| `GET /api/tasks`                  | The register, including your own operations.                                                                                   |

`PUT /api/apps/self/background` and `PUT /api/apps/self/shares` are the writes about yourself: see
[Working without your window](#working-without-your-window) and
[Sharing folders on the local network](#sharing-folders-on-the-local-network).

### What you must not do

- **Do not write the settings, the desktop shortcuts or the access rules.** They belong to the
  user and to the launcher. Your own configuration is yours to store.
- **Do not build a shell command out of anything a user typed.** The launcher never does, and
  neither should you: fixed program, separate arguments.
- **Do not listen anywhere but your socket.** A port on the machine is a door around the launcher,
  and nothing guards it.
- **Do not treat the launcher as an authentication service.** It does not authenticate anybody.
  `X-NAS-User` is its word about who is asking: trust it because only the launcher can reach your
  socket, and never forward what a client sent you under that name.

---

## Trying it without a NAS

The launcher runs on a development machine, and so does the whole package path:

```bash
npm run backend:dev                      # the API on 127.0.0.1:3000
npm run dev                              # the interface on 5173
```

By default the launcher **reaches no repository at all**: package operations need
`NAS_PACKAGES_MODE=real`, exactly like power, network and firewall changes need theirs. To exercise
an installation without touching the internet, point it at a forge of your own: that is what the
end-to-end suite does, in `src/tests/e2e/fixtures/forge.mjs`:

```bash
node src/tests/e2e/fixtures/forge.mjs 3002      # a forge that answers like GitHub

NAS_PACKAGES_MODE=real \
NAS_GITHUB_API=http://127.0.0.1:3002 \
NAS_CATALOGUE_PATH=./my-catalogue.json \
NAS_APPS_DIR=./apps \
cargo run --manifest-path backend/axum/Cargo.toml
```

Then install from the application manager, and watch the operation in the header. That is the same
path a real release takes, with the repository replaced.

On a development machine the launcher **starts nothing**: `NAS_APPS_MODE=real` is what makes it
ask systemd, and without it the application manager says so instead of drawing a button that
cannot work. So you start your own application by hand, on the socket the launcher derives:

```bash
NAS_PACKAGES_MODE=real \
NAS_APPS_DIR=./apps \
NAS_APP_SOCKETS=./sockets \
cargo run --manifest-path backend/axum/Cargo.toml

# your application, on the socket named after its id
./my-app --listen ./sockets/myApp.sock
```

In a browser, `npm run dev` forwards `/apps` to the backend like `/api`: open the shell and then
your application from the menu. From the shell the launcher alone is enough:

```bash
curl http://127.0.0.1:3000/apps/myApp/
```

The end-to-end suite does the same thing in `src/tests/e2e/fixtures/app.mjs`: a stand-in
application on a socket, opened at `/apps/fileExplorer/`.

## Optional antivirus client

Only the registered `nas-security-center` frame can request `antivirus-status`,
`antivirus-enable` (`{ enabled }`), `antivirus-settings` (the settings document),
`antivirus-scan` (`{ kind: 'full' | 'quick' | 'file' | 'custom', paths: string[], skipUnchanged?:
boolean }`), `antivirus-quarantine`, `antivirus-restore`, `antivirus-delete` (`{ id }`) and
`antivirus-forget`. Requests use the
existing `{ nas: 1, type, requestId, payload }` envelope and `<type>-result` reply.
The shell validates them and uses its authenticated launcher client. Paths are absolute NAS
paths; the existing Explorer picker remains limited to NAS storage.

ClamAV is optional, disabled by default and absent from this app's manifest packages. Enabling
requests `clamav` and `clamav-freshclam` through the same package helper as Email. The launcher
persists settings, schedules jobs using the NAS timezone, reports operation status and records
notifications even when the app is closed. Machine operations require `NAS_APPS_MODE=real`.
