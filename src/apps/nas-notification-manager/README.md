# Notifications

- `NotificationManager.vue`: the levels and the source in a menu on the left
  (`NotificationSidebar.vue`), as in nas-log-manager; the title, the actions on the whole list, the
  search, the unread switch and the reset of the filters in its header (`NotificationToolbar.vue`).
  The filters are asked of the server as they change (there is no filter button), the text is
  searched in the page on what it shows, in its language. The source is a selector fed by
  `/api/notifications/sources`, and the source written on a row filters by it in one click. The
  list scrolls by itself, with only the thumb of its scrollbar (`minimal-scrollbar`).
  Read and remove actions, on one notification or on the whole list, next to the text of the row.
  The magnifier opens nas-log-manager, an external application, on the notification
  (`/apps/nas-log-manager/?notification=<id>`); it is disabled while nas-log-manager is not installed,
  and shown only on warnings and errors, the only ones nas-log-manager holds.
- `NotificationDropdown.vue`: the latest five **unread** notifications, the unread count, removal
  through an icon laid over the row, and a link into the application saying how many were left
  out.
- `NotificationToasts.vue`: the one place where notifications pop up on screen, mounted by
  `MainPage`. A toast is a new unread entry of the log, found by a refresh, or a problem of the page
  itself that the log cannot hold (a backend that does not answer) with a retry. A new entry of
  the log goes after 6 seconds, a warning or an error after 12: a bar along its bottom edge counts
  the time down, and is the timer itself, a CSS animation whose end closes the toast and which the
  pointer or the focus pauses. A toast without a duration stays until it is closed.
- `store.ts`: the summary shared with the header, with deduplicated refreshes, and the toasts
  (`toast`, `dismiss`): the same id replaces a toast instead of stacking it, and only the latest
  four are kept. The first refresh after loading shows no toast: the log of a whole day is not
  news. Only the unread notifications written for the first sign-in after them (`SIGN_IN_NOTICES`
  of `text.ts`, such as `discoveryOff` at the end of the wizard) are shown then, until closed, and
  marked as read so that they are not shown again.
- `text.ts`: what a notification says in the language of the interface. The launcher writes a
  `key` and its `params`; the catalogue `i18n/apps/notifications` holds `notice_<key>_title` and
  `notice_<key>_message`, a value with a `noticeValue_<value>` entry is translated too (an action,
  a step), and a parameter ending in `At` is a time. A key without a translation, or no key at all,
  shows the text the notification came with. The source is named by `source_<id>` or by the label
  of the application. A message may name a place with `{link}`: the words are
  `notice_<key>_link`, and where they lead is known by key in `NOTICE_LINKS` (an application and
  the query string it opens on), never by what the notification says. `NoticeMessage.vue` shows
  them as a button in the list and in the toasts; elsewhere the message is plain text.
- `types.ts`: the contracts of the application, including Unix timestamps in milliseconds.

The log lives in the database: a notification written before a restart is still there after it,
capped at the hundred most recent. Older records are removed on insertion and at startup. `POST /api/notifications` is the way in for the
applications and for the system, and every saved settings section leaves one behind saying which
section changed.

The launcher refreshes the summary on an API mutation and every 15 seconds while the page is
visible, so a notification recorded by an application pops up within fifteen seconds. The
filters of the application do not change the list in the menu. The payloads are text:
`AlertBanner` does not interpret HTML.
API and limits: [docs/API.md](../../../docs/development/API.md). No real delivery happens.
