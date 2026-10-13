# Terminal

Two tabs: a command console, and the system log.

The **log** is the system journal followed as it is written, from its last 200 lines, read only:
a fixed `journalctl --follow` that takes nothing from the browser, running only while its tab is on
screen. The **console** is a command console on the NAS. Every window has a shell of its own, opened when the application
opens and stopped when it closes: the directory and the variables stay from one command to the
next. It is a console, not a terminal emulator: there is no terminal behind the shell, so
interactive programs (`top`, `nano`, a password prompt) do not work, and there are no colours.

The shell runs as the launcher does, the `nas-os` service user inside the sandbox of its systemd
unit: no `sudo`, and a filesystem mostly read only. It is for looking at the machine (`ls`, `df`,
`ps`, `ip addr`, `ping`) not for administering it.

- `SystemTerminal.vue`: the application and its two tabs. It opens the shell, follows the log
  while its tab is open, hands the look of the settings to both, and offers to clear the tab on
  screen, to reconnect what the launcher closed and to open the settings.
- `components/OutputPanel.vue`: the lines of either tab. For the console, what was typed, what the
  shell printed and what the page says, such as the exit code of a command that failed. Every line
  is text, never parsed as HTML. The panel follows the end while the reader is there.
- `components/CommandLine.vue`: the prompt with the current directory, the command, and the arrows
  through the history of this window. While a command runs, **Interrupt** replaces the shell with
  a new one, which starts again from `/`.
- `components/OptionsModal.vue` and `components/TerminalStepper.vue`: how the console looks:
  transparency, text, background and highlight colours, font size and line height. The
  transparency applies to the whole window as well as to the output, so up to 100% the desktop
  shows through.
- `store.ts`: the session, the log, the last 2000 lines of each, the history and the settings.
- `types.ts`: the settings, the lines and the state of the connection.

## Backend

`backend/axum/src/apps/services/nas_system_terminal.rs`, with its routes in
`apps/controllers/nas_system_terminal.rs`:

- `GET /api/terminal/log`: server-sent events, one line of the system journal each.
- `GET /api/terminal/session`: server-sent events. The first, `session`, carries the identifier
  of the shell; then one event per line of output, and a `done` event after every command with its
  exit code and the directory the shell is in. The shell is stopped when the stream closes.
- `POST /api/terminal/session/{id}/input`: `{ command }`, one line with no control character,
  answered **204**. What it prints arrives on the stream.
- `GET, PUT /api/settings/terminal`: the settings section `terminal`.

Four shells at most are open at once. Each one is a connection to `nas-os-terminal.socket`
(`NAS_TERMINAL_SOCKET`), where systemd starts a `bash` as the user named after the account of the
wizard. Without `NAS_TERMINAL_MODE=real`, which the unit of the NAS sets, no shell is opened and no
journal followed. To try it on a development machine:
`systemd-socket-activate --accept --inetd -l /tmp/nas-terminal.sock bash --noprofile --norc -s`
and then `NAS_TERMINAL_MODE=real NAS_TERMINAL_SOCKET=/tmp/nas-terminal.sock pnpm run backend:dev`. On the NAS the unit adds the service to the `systemd-journal`
group, which is what lets it read the journal; `scripts/check.sh` says whether it can.
