# Settings

The sections are mounted with `AppSections`, keeping their drafts when the panel changes.
`components/SettingsTabLayout.vue` provides the title, the container (`div`, `section` or `form`),
the loading and the error with a `retry` event. The default slot holds the controls of the tab; the
`footer` slot holds its actions. The events of the form are forwarded to the container.
`components/SettingsStatus.vue` centralises the waiting and error messages for the layout, the
network and the updates. It takes `loading` and `error`; `retryable` adds the retry button that
emits `retry`, leaving the loading of the data to the calling component. Waiting is a spinner
(`CommonSpinner`) and an error is a banner (`AlertBanner`), the same two everywhere.
`components/SettingsSaveFooter.vue` shares the save button and the confirmation message between
General and Access. Data, API calls and the state of the save stay in the individual tabs.
`styles/_settings.scss` arranges the shared controls into groups and aligned fields, without
touching the other applications. The navigation stays visible; the content scrolls separately. On a
phone the tabs are laid out on two rows and the fields in one column.
`store.ts` keeps the general preferences the backend confirmed; the forms do not change the applied
theme before the save. `background.ts` builds the style from bounded values and local images, and
works out the ink the desktop labels have to be written in for them to stay readable.

GET/PUT `/api/settings/general`, `/access`, `/power`; updates on `/api/system/updates`; the list of
allowed addresses on `/api/access/rules`. The notification channels belong to the notifications
application. Types in `types/`.

**What is real and what is not.** Theme, language, accent colour and background are applied by the
launcher itself. The network section reads, scans and joins for real. The power rules are executed
by the scheduler in the backend. The list of allowed addresses is enforced by the launcher and, with
`NAS_FIREWALL_MODE=real`, by `nftables`. The updates are the one section that still lives in memory
and resets when the backend restarts. No SSH rule is applied from here.

The Recovery and reset tab (`RecoverySettingsTab.vue`, the last one) holds the export of the
configuration, shown with a disabled button and a note that says it is still to be built: there is
no API behind it yet. The reset that brings the NAS back to the wizard is in the roadmap.

The Updates tab shows `components/LauncherUpdates.vue` first, with the current and available
versions and its own GET/POST `/api/launcher/updates` for the launcher software. Underneath are the
system packages, with their own selection and API. Both operations are simulated in memory.
