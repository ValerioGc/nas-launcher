# Launcher configuration

Run after `../ui_installation/install.sh`. It performs root-only configuration through the same
helpers used by launcher services. Installation also copies it to
`/usr/local/lib/nas-os/configure.sh`, where it remains after the package is removed.

The wizard runs it as root at the end of initial setup. It reads the token from stdin, leaving
it unset for empty input, creates the terminal user from the account name, and reconciles that
user's data volumes.

```bash
printf '%s' "$TOKEN" | sudo ./install.sh --user <name>
```

For manual administration, use it to set the token for private releases of
`ValerioGc/nas-launcher`. Normally this is entered during setup or through App Manager settings.

```bash
sudo /usr/local/lib/nas-os/configure.sh            # Prompt privately, encrypt the token, and restart the launcher
sudo /usr/local/lib/nas-os/configure.sh --clear    # Remove the token
```

| File         | Purpose                                                                                |
| ------------ | -------------------------------------------------------------------------------------- |
| `install.sh` | Run `launcher-token.sh` and, for wizard setup, `terminal-user.sh` and `nas-os-storage` |

The actual helpers live in `backend/axum/src/services/packages/launcher-token.sh` and
`backend/axum/src/services/terminal/terminal-user.sh`. `ui_installation` installs them in
`/usr/local/lib/nas-os/`. App Manager saves tokens through `nas-os-token.service`, using the same
storage mechanism.

## Token storage

`systemd-creds` encrypts `/etc/credstore.encrypted/nas-os.launcher-token` with the machine key
and TPM when available. Copying the file to another machine does not allow decryption. The
launcher unit imports it through `ImportCredential=nas-os.*`. At startup, systemd decrypts it
and exposes it only to the service in its private credential directory.

The token is kept out of the database and its backups. The UI only reports whether it is set.

One GitHub token must read `ValerioGc/nas-launcher` and private catalog repositories, such as
`ValerioGc/nas-picture-manager`, with **Contents read-only** permission. The launcher shares it
across its own updates and application releases. The wizard also uses it for downloads, so it
must read `ValerioGc/nas-gateway` and `ValerioGc/nas-wizard` as required by their visibility.

Run `sudo /usr/local/lib/nas-os/check.sh` to verify it. The Release token section checks
decryption, service delivery, and GitHub access to each repository, distinguishing a refused
token (401), missing permissions (403), and an invisible repository or missing release (404).
