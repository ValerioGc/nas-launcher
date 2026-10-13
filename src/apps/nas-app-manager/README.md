# Application manager

`AppManager.vue` composes the page and initializes the app-specific Pinia store. Its sections
read shared state and call actions directly, without passing catalogue or view props.

- `store.ts` owns the catalogue, selected app ID, view mode, system-app visibility, credentials
  status, update checks and API
  operations. Selection is derived from current rows, so refreshes also update the commands.
  Requests from an old catalogue tab cannot replace the current one. An installation is followed
  through `/tasks` until it ends, even with the manager closed: its icon shows a spinner meanwhile,
  a failure is shown in the manager, and the end refreshes the lists and the launcher `appsStore`,
  so the application appears in the menu. The message saying it started goes after five seconds.
- `components/AppList.vue` owns filtering, sorting, empty states and the shortcut context menu
  for both views, together with their styles. Switching views preserves selection and sorting
  without another HTTP request.
- `components/AppForge.vue` shows the package mode, a warning while no token is configured and,
  on the installed tab only, the update check and its errors.
- `components/ActionButtons.vue` owns the action choices and modal state. Installation and
  removal require confirmation; failed operations keep the confirmation open for retry. Installing
  and updating stay disabled without a token, and while that application is being installed.
- `components/modals/DetailModal.vue` reads metadata only when opened;
  `ConfirmationModal.vue` presents the pending action. Shared contracts live in `types.ts`.
- `components/modals/ManagerSettings.vue`, opened from the gear in the toolbar, loads and saves
  everything in it with one button at the bottom: `UpdateSchedule.vue` (the automatic update
  check and how often it runs, and what happens each time the manager opens), `AppToken.vue` (the release token, write-only) and
  `CatalogueEditor.vue` (the applications on offer). Editing an entry opens
  `CatalogueEntryForm.vue` in place of the rest, with applying and cancelling in the footer. The
  form takes the logo as an image, checked for format and size before anything is sent. Removing
  an entry requires confirmation and takes effect when the settings are saved. The
  catalogue is sent only when it changed, then the chosen logos (`PUT` or `DELETE
/api/apps/{id}/logo`), and the token last, since storing it restarts the launcher.

Tests mirror these boundaries in `src/tests/apps/nas-app-manager/`: store tests cover request ordering
and operations, component tests cover local interactions, and `AppManager.component.spec.ts`
checks the composed workflows.

## What it installs, and from where

**The catalogue is data**, not a list in the code: adding an application is an entry, not a
release. Each entry says which forge, which repository, which release asset and, optionally, the
category that files it in a folder of the menu, overriding the default from the package manifest.
The categories are media, storage, network, system and development. Its logo is an
image kept on the NAS: uploaded from the settings of the manager, or the one the installation puts
in `/usr/local/share/nas-os/logos` for the applications it knows. Once installed, the icon inside the
package takes its place. The installation puts down
`/etc/nas-os/catalogue.json`; once the catalogue is saved from the settings of the manager
(`PUT /api/apps/catalogue`, the whole list at once) the saved one, a settings section of the
database, takes its place. The launcher cannot write under `/etc`, and the database is where its
other settings already are. Every entry is checked before anything is saved: the identifier, the
repository and the asset end up in directories, unit names, host names and forge paths.

`POST /api/apps/updates` is the "check updates" button. With the automatic check on
(`/api/settings/apps`: every hour, day or week) the launcher also runs it on its own, and leaves a
notification only for a release it has not announced yet. Opening the manager reaches the
internet only when the settings ask for it (`checkOnOpen`): then it runs the same check and, with
`updateOnOpen` too, installs what it found one application at a time, as **Update all** does. The
launcher refuses `updateOnOpen` without `checkOnOpen`. It asks each repository for its latest release and compares it with what is installed, numerically, so `1.10.0`
is newer than `1.9.0`.

An installation **answers before it is done**: the package is fetched, checked against its
published checksum, unpacked and put in place in the background, and the operation is followed in
the operations widget and by the manager. The files are opened to the group of the launcher
(`chmod -R g=u-w`), since the unit of the application runs as another user in that group.

The **token** is the one encrypted systemd credential of the launcher updates, sent on every
request to the forge, for every application: nothing in the catalogue decides about it. The
manager only learns whether it is configured, and sets a new one from its settings. Without it,
installing and updating are disabled. Where this installation fetches nothing at all (no
`NAS_PACKAGES_MODE=real`) the button is disabled and says why.

## API

GET `/api/apps/installed`, `/available`, `/{id}/info`; POST `/{id}/install`, `/{id}/update`,
`/{id}/uninstall`; POST `/apps/updates`; GET `/apps/credentials`; GET, PUT, DELETE `/{id}/logo`.

The system applications are protected from removal on the backend too. The desktop shortcut of a
freshly installed application is created by the backend, which reads the "create shortcut" general
setting: the manager does not create it a second time.
