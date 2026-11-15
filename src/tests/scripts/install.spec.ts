import { mkdirSync, copyFileSync, readdirSync, readFileSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { fakeCommand, run, SYSTEM, workspace } from './helpers';

let temp: ReturnType<typeof workspace>;
let binary = '';

function dryRun(args: string[] = []) {
  return run(join(SYSTEM, 'install.sh'), { args: ['--dry-run', ...args] });
}

beforeEach(() => {
  temp = workspace();
  binary = join(temp.path, 'nas-os-axum');
  writeFileSync(binary, '');
});

afterEach(() => temp.remove());

describe('install.sh', () => {
  it('changes nothing in a dry run, and says so', () => {
    const result = dryRun([binary]);

    expect(result.code).toBe(0);
    expect(result.output).toContain('Dry run: no change will be applied.');
    for (const line of result.stdout.split('\n').filter((l) => /^\s+\[dry run\]/.test(l))) {
      expect(line).toMatch(
        /\[dry run\] (apt-get|install|useradd|groupadd|systemctl|udevadm|cp|chmod|find|runuser|rm|mv)\b/,
      );
    }
  });

  it('leaves the gateway and the configuration of the system to their own packages', () => {
    const output = dryRun([binary]).stdout;
    const commands = output.split('\n').filter((line) => line.includes('[dry run]'));

    expect(commands.filter((line) => line.includes('gateway'))).toEqual([]);
    expect(output).not.toContain('os_configuration');
    expect(output).toContain('interface:  served by the gateway');
  });

  it('extracts bundled archives only for apps absent from the target directory', () => {
    const installer = join(temp.path, 'install.sh');
    const apps = join(temp.path, 'apps');
    mkdirSync(apps);
    copyFileSync(join(SYSTEM, 'install.sh'), installer);
    writeFileSync(join(apps, 'pictureManager.tar.gz'), 'fixture');
    const result = run(installer, { args: ['--dry-run', binary] });
    expect(result.code).toBe(0);
    expect(result.output).toContain('runuser -u nas-os -- tar -xzf');
    expect(result.output).toContain('/var/lib/nas-os/apps/pictureManager');
  });

  it('installs the user, binaries, units and polkit rules', () => {
    const output = dryRun([binary]).output;

    expect(output).toContain('/usr/local/bin/nas-os-axum');
    expect(output).toContain('/etc/systemd/system/nas-os.service');
    expect(output).toContain('/etc/systemd/system/nas-os-wake.service');
    expect(output).toContain('/etc/systemd/system/nas-os-firewall.service');
    expect(output).toContain('/etc/polkit-1/rules.d/10-nas-os-power.rules');
    expect(output).toContain('/etc/polkit-1/rules.d/20-nas-os-network.rules');
    expect(output).toContain('/etc/polkit-1/rules.d/30-nas-os-firewall.rules');
    expect(output).toContain('/usr/local/lib/nas-os/wake-alarm.sh');
    expect(output).toContain('/usr/local/lib/nas-os/apply-access-rules.sh');
    expect(output).toContain('/etc/systemd/system/nas-os-update.service');
    expect(output).toContain('/etc/polkit-1/rules.d/40-nas-os-update.rules');
    expect(output).toContain('/usr/local/lib/nas-os/install-launcher-update.sh');
    expect(output).toContain('/etc/systemd/system/nas-app@.service');
    expect(output).toContain('/etc/polkit-1/rules.d/50-nas-os-apps.rules');
    expect(output).toContain('/usr/local/lib/nas-os/run-app.sh');
    expect(output).toContain('/etc/systemd/system/nas-os-storage.service');
    expect(output).toContain('/etc/systemd/system/nas-os-storage-initialize.service');
    expect(output).toContain('/etc/systemd/system/nas-os-storage-rename.service');
    expect(output).toContain('/etc/systemd/system/nas-os-storage-eject.service');
    expect(output).toContain('/etc/systemd/system/nas-os-storage-smart.service');
    expect(output).toContain('/etc/systemd/system/nas-os-storage.timer');
    expect(output).toContain('/etc/udev/rules.d/90-nas-os-storage.rules');
    expect(output).toContain('/etc/polkit-1/rules.d/90-nas-os-storage.rules');
    expect(output).toContain('/etc/systemd/system/nas-app@nas-file-manager.service.d/storage.conf');
    expect(output).toContain('install -d -m 0700 -o root -g root /srv/nas-volumes');
    expect(output).toContain('install -d -m 0710 -o root -g nas-storage /srv/storage');
  });

  it('leaves on the machine the check, the removal and the configuration', () => {
    const output = dryRun([binary]).stdout;

    expect(output).toContain('/usr/local/lib/nas-os/check.sh');
    expect(output).toContain('/usr/local/lib/nas-os/uninstall.sh');
    expect(output).toContain('ui_configuration/install.sh /usr/local/lib/nas-os/configure.sh');
  });

  it('removes what the launcher no longer starts to create the user of the terminal', () => {
    const output = dryRun([binary]).stdout;

    expect(output).toContain('rm -f /etc/polkit-1/rules.d/80-nas-os-terminal.rules');
    expect(output).toContain('rm -f /etc/systemd/system/nas-os-terminal-user.service');
  });

  it('writes a starter catalogue and a directory for the packages', () => {
    const output = dryRun([binary]).output;

    expect(output).toContain('/etc/nas-os/catalogue.json');
    expect(output).toContain('/var/lib/nas-os/apps');
    expect(output).toMatch(/catalogue already there|starter catalogue written/);
  });

  it('gives the package directory to the service user', () => {
    const line = dryRun([binary])
      .stdout.split('\n')
      .find((entry) => entry.includes('/var/lib/nas-os/apps'));

    expect(line).toContain('-o nas-os');
    expect(line).toContain('0750');
  });

  it('enables the launcher, the storage refresh and the socket of the encryption', () => {
    const enabled = dryRun([binary])
      .stdout.split('\n')
      .filter((line) => line.includes('systemctl enable'))
      .map((line) => line.trim());

    expect(enabled).toEqual([
      '[dry run] systemctl enable nas-os',
      '[dry run] systemctl enable --now nas-os-storage.timer',
      '[dry run] systemctl enable --now nas-os-storage-crypt.socket',
    ]);
  });

  it('never touches the database', () => {
    const result = dryRun([binary]);

    expect(result.stdout).toContain('(not touched by this script)');
    for (const line of result.stdout.split('\n').filter((l) => l.includes('[dry run]'))) {
      expect(line).not.toContain('launcher.db');
    }
  });

  it('refuses to go on without a binary to install', () => {
    const result = dryRun([join(temp.path, 'does-not-exist')]);

    expect(result.code).not.toBe(0);
    expect(result.output).toContain('binary not found');
    expect(result.output).toContain('cargo build --release');
  });

  it('refuses an option it does not know instead of taking it for a path', () => {
    const result = dryRun(['--wipe-everything']);

    expect(result.code).toBe(2);
    expect(result.output).toContain('unknown option');
  });

  it('says what will not work when a piece of the machine is missing', () => {
    const output = dryRun([binary]).output;
    const missing = /polkit is not here|rules installed/;

    expect(output).toMatch(missing);
    expect(output).toMatch(/wake alarm|scheduled power-on|does not expose/i);
    expect(output).toMatch(/nftables|access list/i);
  });
});

describe('uninstall.sh', () => {
  const source = readFileSync(join(SYSTEM, 'scripts/uninstall.sh'), 'utf8');

  /** The section that stops the launcher, run with a systemctl that knows the unit or not. */
  function service(loadState: string) {
    const section = source.slice(
      source.indexOf('load=$(systemctl show "$SERVICE_NAME.service"'),
      source.indexOf('stop_unit nas-os-hostname.service'),
    );
    const script = join(temp.path, 'service.sh');
    writeFileSync(script, `set -euo pipefail\nSERVICE_NAME=nas-os\n${section}`);
    fakeCommand(
      temp.path,
      'systemctl',
      `if [[ $1 == show ]]; then echo ${loadState}; else printf '%s\\n' "$*" >> "${join(temp.path, 'calls')}"; fi`,
    );
    fakeCommand(temp.path, 'rm', ':');
    return run(script, { env: { PATH: `${temp.path}:${process.env.PATH}` } });
  }

  it('stops, disables and removes the unit of the launcher when systemd knows it', () => {
    const result = service('loaded');

    expect(result.code).toBe(0);
    expect(result.output).toContain('stopped, disabled and unit removed');
    expect(readFileSync(join(temp.path, 'calls'), 'utf8')).toContain(
      'disable --now nas-os.service',
    );
  });

  it('says the launcher was not installed when systemd does not know its unit', () => {
    const result = service('not-found');

    expect(result.code).toBe(0);
    expect(result.output).toContain('it was not installed');
  });

  it('names the step where a removal stopped, since running it again finishes it', () => {
    const say = source.slice(source.indexOf('say() {'), source.indexOf('fail() {'));
    const trap = source.split('\n').find((line) => line.startsWith("trap '"));
    const script = join(temp.path, 'stopped.sh');
    writeFileSync(
      script,
      `set -euo pipefail\n${say}\nstep=''\n${trap}\nsay "Shared folders"\nfalse\n`,
    );

    const result = run(script);

    expect(result.code).not.toBe(0);
    expect(result.stderr).toContain('the removal stopped at "Shared folders", run it again');
  });

  it('refuses an option it does not know before it touches anything', () => {
    const result = run(join(SYSTEM, 'scripts/uninstall.sh'), { args: ['--everything'] });

    expect(result.code).toBe(2);
    expect(result.output).toContain('unknown option --everything');
  });

  it('takes --yes next to --purge in any order, and asks only without it', () => {
    expect(source).toMatch(/--purge\) purge=true ;;\n\s+--yes\) confirmed=true ;;/);
    expect(source).toContain('for argument in "$@"; do');
    const purge = source.slice(source.indexOf('if $purge; then'));
    expect(purge).toMatch(
      /if \$confirmed; then\n\s+answer=delete\n\s+else\n\s+read -r -p " {3}Type 'delete' to confirm: " answer\n\s+fi/,
    );
  });
});

describe('uninstall.sh against install.sh', () => {
  const install = readFileSync(join(SYSTEM, 'install.sh'), 'utf8');
  const uninstall = readFileSync(join(SYSTEM, 'scripts/uninstall.sh'), 'utf8');
  const files = readdirSync(join(SYSTEM, 'files'));

  it('removes every unit the installation writes', () => {
    const units = files.filter((name) => /\.(service|socket|timer)$/.test(name));
    expect(units.length).toBeGreaterThan(20);
    for (const unit of units) {
      expect(uninstall, unit).toContain(`/etc/systemd/system/${unit}`);
    }
  });

  it('removes every polkit and udev rule the installation writes', () => {
    for (const rule of files.filter((name) => name.endsWith('.rules'))) {
      const target =
        rule === '90-nas-os-storage-udev.rules'
          ? '/etc/udev/rules.d/90-nas-os-storage.rules'
          : `/etc/polkit-1/rules.d/${rule}`;
      expect(install, rule).toContain(rule);
      expect(uninstall, rule).toContain(target);
    }
  });

  it('removes the folders of the launcher, the drop-in of freshclam and the update check', () => {
    for (const path of [
      'rm -rf /usr/local/lib/nas-os',
      'rm -rf /usr/local/share/nas-os',
      'rm -f /usr/local/bin/nas-os-axum',
      'rm -f /etc/systemd/system/clamav-freshclam.service.d/nas-os-firewall.conf',
      'stop_unit nas-os-update-verify.service disable --now',
      '/etc/systemd/system/nas-os-update-verify.service',
    ]) {
      expect(uninstall).toContain(path);
    }
  });
});
