# Installing on the NAS

These scripts run the launcher on a real Debian 13 machine. Development does not require them.
The separate wizard (`../nas-wizard`) first prepares the OS and gateway, then downloads this
package and runs `install.sh` and `../ui_configuration/install.sh`. Launcher updates rerun the
installer from the newer package. These scripts provide the permissions and services needed for
real machine operations.

## Package contents

`npm run release:package`, also used by the pipeline, writes `release-assets/`. The package
includes this directory's files, launcher binary, compiled UI, and privileged helpers copied
from beside their Rust callers.

| File                               | Purpose                                                                         |
| ---------------------------------- | ------------------------------------------------------------------------------- |
| `install.sh`                       | Install/update user, binary, UI, permissions, and units                         |
| `scripts/check.sh`                 | Verify installation without rebooting or shutting down                          |
| `scripts/uninstall.sh`             | Remove installed software; retain data unless `--purge` is used                 |
| `files/nas-os.service`             | Hardened launcher unit with real-mode settings                                  |
| `files/10-nas-os-power.rules`      | Launcher-only restart, shutdown, and wake-alarm authorization                   |
| `files/20-nas-os-network.rules`    | NetworkManager scan, connection, and profile permissions                        |
| `files/30-nas-os-firewall.rules`   | Only start the address-filter unit                                              |
| `files/40-nas-os-update.rules`     | Only start the NAS-update unit                                                  |
| `files/50-nas-os-apps.rules`       | Start, stop, and restart valid `nas-app@<id>` instances                         |
| `files/nas-os-wake.service`        | Root hardware wake-alarm helper                                                 |
| `files/nas-os-firewall.service`    | Root allowed-address filter helper                                              |
| `files/nas-os-update.service`      | Replace downloaded NAS components                                               |
| `files/nas-app@.service`           | Installed-application process template                                          |
| `files/nas-os-app-grants@.service` | Root application-path grant helper                                              |
| `files/55-nas-os-app-grants.rules` | Only start valid grant instances                                                |
| `files/60-nas-os-apt.rules`        | Only start apt refresh/upgrade instances                                        |
| `files/nas-os-apt@.service`        | Root apt index/update template                                                  |
| `files/65-nas-os-processes.rules`  | Only start valid process-action instances                                       |
| `files/nas-os-process@.service`    | Root process termination/service-restart template                               |
| `files/70-nas-os-token.rules`      | Only start the release-token helper                                             |
| `files/nas-os-token.service`       | Encrypt tokens supplied by setup or App Manager                                 |
| `files/75-nas-os-reset.rules`      | Only start factory reset                                                        |
| `files/nas-os-reset.service`       | Clear NAS state and restart the wizard as root                                  |
| `files/catalogue.json`             | Initial installable-application catalog                                         |
| `files/logos/*`                    | Default known-application logos installed under `/usr/local/share/nas-os/logos` |
| `apps/*.tar.gz`                    | Optional packages copied from `system/apps` during build                        |
| `bin/nas-os-axum`                  | Launcher binary added during packaging                                          |
| `ui/`                              | Compiled interface added during packaging                                       |
| `helpers/*.sh`                     | Unit helpers copied from `backend/axum/src/`                                    |

Helper source locations are listed below.

| Helper                       | Module directory under `backend/axum/src/`          |
| ---------------------------- | --------------------------------------------------- |
| `wake-alarm.sh`              | `apps/services/nas_system_settings/power/`          |
| `apply-access-rules.sh`      | `apps/services/nas_system_settings/access/`         |
| `run-app.sh`                 | `services/lifecycle/`                               |
| `apply-app-grants.sh`        | `services/grants/`                                  |
| `install-launcher-update.sh` | `apps/services/nas_system_settings/updates/`        |
| `apt-updates.sh`             | `apps/services/nas_system_settings/system_updates/` |
| `launcher-token.sh`          | `services/packages/`                                |
| `terminal-user.sh`           | `services/terminal/`                                |
| `apply-hostname.sh`          | `apps/services/nas_system_settings/`                |
| `factory-reset.sh`           | `apps/services/nas_system_settings/reset/`          |

## Usage

Normally the wizard installs the launcher. For manual updates on a wizard-configured NAS, build
the binary/UI/manual before packaging, then verify and install the release.

```bash
npm run release:package
sha256sum -c nas-os-<version>-linux-x64.tar.gz.sha256
mkdir nas-os && tar -xzf nas-os-<version>-linux-x64.tar.gz -C nas-os
cd nas-os/packages/ui_installation
sudo ./install.sh
sudo /usr/local/lib/nas-os/check.sh
```

Before building, optional archives may be placed in `system/apps/<id>.tar.gz`. Each archive must
contain root `nas-app.json` with the same ID. The release includes them. Installation temporarily
stages them in the state directory and extracts as `nas-os` only when that application is absent.
At first startup, the launcher records discovered manifests in SQLite and App Manager shows them
as installed.

The catalog includes private `nas-picture-manager` from `ValerioGc/nas-picture-manager`; it need
not be bundled. Its NAS picker requires nas-file-manager and data-volume access. Existing NAS
installations should update launcher, Explorer, and Picture Manager together. Existing catalogs
are preserved. Replace an obsolete `picture-manager` entry with ID `nas-picture-manager`,
repository `ValerioGc/nas-picture-manager`, and asset
`nas-picture-manager-{version}-linux-x64.tar.gz`. Install using the new ID; updates do not rename
applications. Published asset names must match the catalog's `asset` template.

An already compiled binary can also be supplied as `sudo ./install.sh /tmp/nas-os-axum`.

## Installed behavior

1. Create system service user `nas-os` without shell or password. The launcher never runs as root.
2. Install `/usr/local/bin/nas-os-axum` and UI under `/usr/local/share/nas-os/ui`. The separately
   installed gateway serves the UI on HTTP/HTTPS, authenticates users, and forwards to the
   loopback launcher.
3. Grant only the required restart/power-off actions. Do not grant `ignore-inhibit` variants;
   existing shutdown inhibitors remain respected.
4. Configure `StateDirectory=nas-os` with `0710` group traversal for application packages, keeping
   launcher files private. Read `/etc/nas-os/nas-os.env`, including real-mode switches, and enable
   hardening such as `ProtectSystem=strict`, `NoNewPrivileges`, and syscall filtering.
5. Install `nas-os-wake.service` and its helper under `/usr/local/lib/nas-os/`, without enabling
   the unit at boot.
6. Authorize wireless scanning/connection and enable real network changes through the environment.
7. Install `nas-os-firewall.service` and its helper with real firewall mode. The launcher starts
   it on rule changes; it is not boot-enabled. The helper validates address characters and
   replaces the table atomically.
8. Preserve `/etc/nas-os/catalogue.json`, install default logos, prepare `/var/lib/nas-os/apps`,
   and import bundled applications. Existing package trees become readable to `nas-os` group
   with `chmod -R g=u-w`, allowing dynamic application users to read older private extractions.
   `check.sh` reports inaccessible files.
9. Install `nas-app@.service` and `run-app.sh` for application lifecycle.
10. Install `nas-os-update.service` and `nas-os-apt@.service`, started only for launcher requests.
11. Install `nas-os-token.service` and `launcher-token.sh` for App Manager tokens. Initial setup
    stores the token through root-run `ui_configuration`.
12. Prepare external `nas-system-terminal` support. The application owns WebSocket/PTY behavior.
    Real terminal mode and `systemd-journal` membership allow launcher journal reads. Root
    `terminal-user.sh` creates the account-named user without password or sudo and enables
    `nas-os-terminal.socket`. Each connection runs the application's `--shell` command through
    `nas-os-terminal@.service` as that user. Supplementary `nas-os` group access is limited to
    the session and does not change account group membership. `su - nas` can authenticate with
    the Linux administrator password and use that account's existing sudo permissions. Only the
    dedicated terminal unit uses `NoNewPrivileges=no`; launcher/application HTTP hardening
    remains enabled. `KillMode=control-group` cleans up children. The launcher retains preferences,
    journal, and user configuration and removes obsolete terminal polkit units.
13. Keep `check.sh`, `uninstall.sh`, and `configure.sh` (copied from ui_configuration) in
    `/usr/local/lib/nas-os/` after the package directory is removed.

Launcher/application units have no capabilities and restrict clock, kernel logs, hostname, and
socket families. Inspect them before installation with
`systemd-analyze security --offline=yes files/nas-os.service`.

## Opening applications

Applications run independently on Unix sockets and appear under `/apps/<id>/` on the same
origin/port as the launcher, protected by the gateway session. No extra client network setup is
required, including for a single forwarded NAS port.

`RuntimeDirectory=nas-os/apps` creates `/run/nas-os/apps`, and `RuntimeDirectoryPreserve=yes`
keeps sockets across launcher restart because application processes are not restarted with it.
`NAS_APP_SOCKETS` selects the socket directory.

`nas-app@.service` runs one application per instance. `run-app.sh` reads the manifest executable.
The template is never enabled globally; the launcher starts instances after installation or user
requests. `50-nas-os-apps.rules` permits only start/stop/restart for valid application unit names.
`NAS_APPS_MODE=real` enables this behavior.

Applications use `DynamicUser=yes` and supplementary `nas-os` group membership. Setgid socket
directories and `UMask=0007` permit launcher access; sticky mode `3770` prevents applications from
removing another socket. Application units use two Tokio workers and two glibc arenas through
`TOKIO_WORKER_THREADS=2`/`MALLOC_ARENA_MAX=2` and prohibit writable/executable memory with
`MemoryDenyWriteExecute`. Launcher and gateway use four workers rather than core-scaled defaults
that retain unnecessary memory.

The template hides `/srv`, `/mnt`, `/media`, logs, launcher state except the current package,
`/sys`, `/run/udev`, and other processes. The launcher permits only allowlisted manifest paths,
including `/srv/storage` and read-only `/proc`, `/sys`, and `/run/udev`. Unsupported requests reject
installation. Polkit rule 55 permits starting `nas-os-app-grants@<id>.service`, whose root helper
writes `/etc/systemd/system/nas-app@<id>.service.d/grants.conf`.

Explorer/Backup storage access adds group `nas-storage`, `BindPaths=/srv/storage`,
`NAS_STORAGE_ROOT=/srv/storage`, and `NAS_STORAGE_REGISTRY=/etc/nas-os/storage-volumes.tsv`.
`nas-storage` can traverse published paths; raw `/srv/nas-volumes` mounts remain root-only.

Process controls start `nas-os-process@<terminate|restart>-<pid>-<start>.service` through polkit
rule 65. Root `act-on-process.sh` rechecks process start time, protects PID 1 and kernel threads,
and allows only restart for essential launcher, gateway, D-Bus, polkit, systemd, NetworkManager,
and SSH units. Real process mode is required.

### Local storage

Root `nas-os-storage.service` discovers data filesystems by UUID, excludes system filesystems,
mounts each under `/srv/nas-volumes/v-<uuid>`, and bind-publishes only `.nas-users/<user>` under
`/srv/storage/v-<uuid>`. The user comes from the terminal unit's
`/etc/systemd/system/nas-os-terminal@.service.d/user.conf`, written by `terminal-user.sh`, and is
accepted only with the `nas-os terminal` account comment. Only the launcher opens its database.
A timer and udev repeat reconciliation after boot/hotplug. ui_configuration reconciles again after
creating the user because installation-time reconciliation could not publish its area yet.

Files outside that area are neither exposed nor modified. Unsupported/unformatted partitions
are excluded. The registry contains ready volumes only; inspect mount failures with
`journalctl -u nas-os-storage.service`. Unmounted placeholder directories under `/srv/nas-volumes`
and `/srv/storage` use mode `000`, preventing disconnected disks from becoming writable system-disk
directories. Registry changes reapply Samba shares; `apply-smb-shares.sh` excludes unmounted disks.

`services/storage/inventory.rs` provides one disk classification for reconciliation, root units,
and `GET /api/storage/disks`, exposed to nas-disk-manager through `disk-inventory`. System disks
cannot be erased in any part, even with a mountable data partition. Destructive requests repeat
the whole-disk WWN/serial/size identity for root revalidation.

nas-disk-manager explicitly initializes empty whole disks through fixed
`nas-os-storage-initialize.service`. Only that application may request it. Root code verifies
writable status, identity, and absence of partitions, mounts, filesystems, and signatures, creates
a GPT/ext4 partition, then reconciles. The wizard installs `parted` and `e2fsprogs`. Installation
never initializes a disk automatically.

### Encrypted data disks

LUKS2 operations use `nas-os-storage-crypt.socket`. Only group `nas-os-crypt`, containing the
launcher rather than normal applications, can open it. Connections start root
`nas-os-storage-crypt@.service` with `nas-os-axum --storage-crypt`. Requests, including passwords,
travel through the socket. Command-lifetime key files stay in private runtime tmpfs, mode `0700`,
never command arguments or journal output.

Operations include formatting internal data disks, unlocking/locking, password changes,
automatic/recovery keys, and header export. Identity is revalidated, and system/LVM/RAID disks are
excluded. Slots are 0 for password, 1 for automatic key, and 2 for recovery key. Automatic keys
use encrypted systemd credentials `/etc/credstore.encrypted/nas-os.luks-<uuid>`, bound to machine
key and TPM when present. Reconciliation unlocks them without crypttab, so a failure cannot block
boot. User-locked volumes remain in `/var/lib/nas-os/storage-held` across restarts. Reconciliation
records available keys in `/var/lib/nas-os/storage-crypt.json`.

Encryption is enabled by default, including on an unencrypted system. Set `NAS_ENCRYPTION_MODE=off`
in `/etc/nas-os/nas-os.local.env` to disable it. The wizard records LUKS/TPM status in
`/var/lib/nas-wizard/security`; the launcher reports `systemUnprotected` once and exposes
`GET /storage/security` through `disk-security` to explain automatic-key protection in Disks.
Uninstall preserves automatic keys and prints manual unlock instructions.

### Network disks and maintenance

The same privileged socket handles SMB network disks. Server/share/user/domain configuration is
stored in `/etc/nas-os/network-disks.json`; the password uses encrypted credential `nas-os.smb-<id>`.
`mount.cifs` from `cifs-utils`, requested by Disks, reads a temporary runtime credentials file that
is deleted after mounting. After local publication, reconciliation mounts boot-connected shares
under `/srv/storage/n-<id>` unless held disconnected. Each mount has a 20-second timeout, with
30 seconds overall. Explorer, Backup, and Sync see them as `network` volumes. Kernel TCP 445 is
limited to local server addresses in base-firewall sets `nas_smb_servers`/`nas_smb_servers6`.
Samba does not re-share network disks. Network disks are enabled by default;
`NAS_NETWORK_DISKS_MODE=off` disables them. Full removal deletes their passwords.

nas-disk-manager may rename writable ext2/ext3/ext4 volumes through fixed
`nas-os-storage-rename.service`. Root code revalidates device, filesystem, and data-area ownership
before `e2label`, rejecting labels used by fstab mounts to avoid breaking the next boot.
`nas-os-storage-smart.service` similarly validates data disks and runs read-only `smartctl`.
Browsers never access devices directly.

External removable/USB/card-reader/FireWire disks are ejected through
`nas-os-storage-eject.service`. It rechecks external status and absence of system filesystems,
unmounts Explorer views before raw and other mounts, detaches `/sys/block/<disk>/device/delete`,
and republishes remaining volumes. Filesystem UUID entries in `storage_locks` prevent ejection
of marked disks and persist across reconnection. Application removal deletes its grant drop-in
without altering mounts, registry, or user data.

## Scheduled power-on

A powered-off machine cannot run a schedule. Before scheduled shutdown, the launcher looks for
the next power-on rule, writes its time to `/var/lib/nas-os/wakealarm`, and starts
`nas-os-wake.service`. Root copies it to `/sys/class/rtc/rtc0/wakealarm`, then shutdown proceeds.
The launcher never writes the hardware clock directly. Polkit allows only starting the wake
unit, not stopping or enabling it.

Wake-alarm failure still permits shutdown and creates a notification explaining the problem.
The firmware must enable RTC wake, possibly called Wake on RTC, RTC Alarm, or Resume by Alarm,
and the machine must remain powered. A correctly written file alone cannot verify firmware
behavior. The first actual scheduled wake is the operational check; `check.sh` only verifies
installed support. The launcher remains loopback-only behind the gateway.

## Networking

Read-only collection uses `ip`, sysfs, and `nmcli` for interface/address/SSID/signal without extra
permissions. Changes require real network mode and a NetworkManager polkit rule authorizing
scan, activation, and profile saving. Without real mode, the backend returns 503 explaining why
it will not alter the development machine. The rule does not grant actions that switch off the
NAS connection entirely.

The launcher invokes `nmcli` with separate arguments rather than composing shell strings. SSIDs
containing spaces, quotes, or semicolons remain plain SSIDs.

## Allowed addresses

Both enforcement layers are required. Direct launcher requests outside the list receive 403.
nftables drops traffic before HTTP processing and covers the rest of the NAS. Real firewall mode,
polkit, and installed `nft` are required. The launcher writes `/var/lib/nas-os/access-rules` and
can only start its dedicated root unit.

Loopback, established connections, DHCP, and ICMP remain allowed, preserving local operation,
address renewal, and IPv6. An empty list adds no restrictions; the first rule activates the
allowlist. The wizard's separate base firewall remains effective throughout.

## Configuration overrides

`/etc/nas-os/nas-os.env` comes from `config/production.env` and is replaced on installation/update.
Place machine-specific overrides in `/etc/nas-os/nas-os.local.env`, which takes precedence and
is preserved.

```bash
echo 'NAS_APT_MODE=simulated' | sudo tee -a /etc/nas-os/nas-os.local.env
sudo systemctl restart nas-os nas-os-gateway
```

Use one unquoted `KEY=value` per line. `check.sh` reads both files. Without the real-power switch,
the backend reports `"simulated": true`, records the notification, and leaves the machine running.

## Troubleshooting

`check.sh` identifies missing components. Common cases are listed below.

- `org.freedesktop.login1.reboot refused` means missing/unreloaded polkit rules. The launcher
  returns 503 rather than reporting a successful shutdown.
- `power mode: simulated` means the real-power setting is missing or overridden locally.
- Listed Wi-Fi networks that cannot connect indicate disabled real network mode or missing
  NetworkManager authorization. The UI displays the actual nmcli error.
- Unresponsive `/health` requires checking `journalctl -u nas-os -n 50`.
- Shutdown without wake requires `journalctl -u nas-os-wake -n 20`. An alarm-set message points
  to firmware settings; absent execution suggests missing polkit authorization and a Wake alarm
  not set notification.
- Unexpected wake may be an obsolete RTC alarm. Clear it with
  `echo 0 | sudo tee /sys/class/rtc/rtc0/wakealarm`.
- An allowed address still blocked should be checked in `nft list table inet nas_launcher`.

## Launcher updates and rollback

Settings > Updates downloads/verifies releases and starts `nas-os-update.service`, which backs up,
installs, verifies readiness, and reboots when the launcher changes. Failure restores the backup
and previous launcher; the next startup reports the outcome.

Root-private backups in `/var/lib/nas-os/backups/` retain the latest three. To restore one manually,
stop the launcher and remove the new WAL/SHM before extracting the backup.

```bash
sudo systemctl stop nas-os
sudo rm -f /var/lib/nas-os/launcher.db-wal /var/lib/nas-os/launcher.db-shm
sudo tar -xpzf /var/lib/nas-os/backups/<date>.tar.gz -C /
sudo systemctl daemon-reload && sudo systemctl start nas-os
```

## Removal

```bash
sudo /usr/local/lib/nas-os/uninstall.sh            # Services, binary, and permissions; preserve data
sudo /usr/local/lib/nas-os/uninstall.sh --purge    # Also remove database, application data, and user after confirmation
```

Full NAS removal, including token, gateway, launcher, base firewall, and NAS-added Debian packages,
uses `sudo ./uninstall.sh`, the single script retained in the wizard package directory.

## Optional antivirus

Installation copies `nas-os-antivirus.service` and `57-nas-os-antivirus.rules` without enabling
antivirus or installing ClamAV. Security Center requests `clamav` and `clamav-freshclam` through
the application package helper only when enabled. The unit runs `--antivirus-job` and revalidates
its request. Scanning/quarantine never grants privileged system access to the external UI.

FreshClam runs for signature updates while enabled and stops/disables when antivirus is disabled.
Because the base firewall does not normally allow user `clamav` outbound web access, the antivirus
unit and installed `clamav-freshclam.service.d/nas-os-firewall.conf` add their cgroups to
`nas_apps_out` using `NFTSet=`, like application units.

`/var/lib/nas-os-antivirus` quarantine remains root-private and survives launcher removal.
Permanent isolated-file deletion requires confirmation through the Antivirus page.

The optional `nas-os-antivirus-lifecycle.service` is installed disabled. The executor enables it
only for requested boot/shutdown scans. Boot ID prevents scans on feature enablement or launcher
restart. Final scans run only when systemd is stopping. Boot/shutdown waits are limited to 24 hours.
Both antivirus units share flock serialization. Results persist for post-reboot notifications;
quarantine and metadata remain root-private. Factory reset and uninstall disable lifecycle
scanning. Relevant unit ordering/options are documented in the official
[systemd manual](https://www.freedesktop.org/software/systemd/man/latest/systemd.service.html)
and [ClamAV manual](https://docs.clamav.net/manual/Usage/Scanning.html).
