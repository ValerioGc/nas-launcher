# Global search

A fixed-size modal with a row of tabs (Everything, Applications, Settings, Files) above the filters.
Each tab shows only its own filters: the origin of an application, the section of the settings, the
kind and extension of a file. The reset button clears them and keeps the text searched for.

Applications and settings entries are found by the shell, without a request: the applications from
`appsStore`, the settings from the index in `nas-system-settings/search.ts`. Choosing a settings entry
opens the settings on its section.

Files are searched by the applications that declare the `search` capability. The launcher only
lists them (`GET /api/capabilities/search`); the browser calls each one directly at
`/apps/<id><endpoint>`, all at once, with a 10 second timeout each. The answers that arrive are
shown, and the providers that failed are named. The contract is in
[APPS.md](../../../docs/development/external_apps/APPS.md).

A file opens in the application that found it, on the query string of its `target`.

The request leaves after 250 ms, through the shared `useDebounced`, and earlier answers are
discarded as soon as the query, the tab or a file filter changes. On the Files tab the results are
a table with six sortable columns; elsewhere they are a list grouped by kind.
