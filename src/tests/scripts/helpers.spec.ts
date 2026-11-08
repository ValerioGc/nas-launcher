import { execFileSync, spawn, type ChildProcess } from 'node:child_process';
import {
  chmodSync,
  existsSync,
  lstatSync,
  mkdirSync,
  readdirSync,
  readFileSync,
  rmSync,
  statSync,
  symlinkSync,
  writeFileSync,
} from 'node:fs';
import { dirname, join } from 'node:path';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { fakeCommand, HELPERS, run, workspace } from './helpers';

let temp: ReturnType<typeof workspace>;

beforeEach(() => {
  temp = workspace();
});

afterEach(() => temp.remove());

describe('apply-access-rules.sh', () => {
  const script = HELPERS.access;

  function withFakeNft(list: string, https?: string, refuse = false) {
    const rules = join(temp.path, 'access-rules');
    const loaded = join(temp.path, 'loaded');
    writeFileSync(rules, list);
    const connection = join(temp.path, 'gateway.connection');
    if (https !== undefined) {
      writeFileSync(connection, `${https}\n1\n`);
    }
    fakeCommand(
      temp.path,
      'nft',
      `if [[ "\${1:-}" == "-f" ]]; then cat >> ${JSON.stringify(loaded)}; exit ${refuse ? 1 : 0}; fi
       printf 'nft %s\\n' "$*" >> ${JSON.stringify(join(temp.path, 'calls'))}
       [[ "\${1:-}" == "list" ]] && exit 1
       exit 0`,
    );

    const result = run(script, {
      env: {
        NAS_ACCESS_RULES: rules,
        NAS_GATEWAY_CONNECTION: connection,
        PATH: `${temp.path}:${process.env.PATH}`,
      },
    });
    const read = (path: string) => {
      try {
        return readFileSync(path, 'utf8');
      } catch {
        return '';
      }
    };
    return { result, ruleset: read(loaded), calls: read(join(temp.path, 'calls')) };
  }

  it('replaces the HTTPS port in one nft transaction and preserves the access list', () => {
    const { result, ruleset } = withFakeNft('192.168.1.5\n', '8443');
    expect(result.code).toBe(0);
    expect(ruleset).toContain(
      'flush set inet nas_base nas_https\nadd element inet nas_base nas_https { 8443 }\n',
    );
    expect(ruleset).toContain('elements = { 192.168.1.5 }');
    expect(ruleset).not.toContain('{ 443 }');
  });

  it.each(['0', '65536', '443; flush ruleset'])('rejects an invalid HTTPS port: %s', (port) => {
    const { result, ruleset } = withFakeNft('', port);
    expect(result.code).not.toBe(0);
    expect(ruleset).toBe('');
  });

  it('reports a refused HTTPS firewall transaction instead of claiming success', () => {
    const { result } = withFakeNft('', '8443', true);
    expect(result.code).not.toBe(0);
    expect(result.output).toContain('cannot update the HTTPS port');
  });

  it('turns the saved list into one set per family', () => {
    const { result, ruleset } = withFakeNft('192.168.1.5\n192.168.1.0/24\n2001:db8::/32\n');

    expect(result.code).toBe(0);
    expect(result.stdout).toContain('2 IPv4, 1 IPv6');
    expect(ruleset).toContain('elements = { 192.168.1.5,192.168.1.0/24 }');
    expect(ruleset).toContain('elements = { 2001:db8::/32 }');
  });

  it('keeps anything that is not an address out of the ruleset', () => {
    const { result, ruleset } = withFakeNft(
      '192.168.1.5\n1.2.3.4 } ; flush ruleset ; table ip x {\n10.0.0.0/8\n',
    );

    expect(result.code).toBe(0);
    expect(ruleset).toContain('elements = { 192.168.1.5,10.0.0.0/8 }');
    expect(ruleset).not.toContain('flush');
  });

  it('replaces the previous table in the same transaction', () => {
    const { calls, ruleset } = withFakeNft('192.168.1.5\n');

    expect(ruleset.split('\n').slice(0, 2)).toEqual([
      'table inet nas_launcher {}',
      'delete table inet nas_launcher',
    ]);
    expect(calls).not.toContain('delete');
  });

  it('lets overlapping rules live in the same set', () => {
    const { ruleset } = withFakeNft('192.168.1.5\n192.168.1.0/24\n');

    expect(ruleset).toContain('auto-merge');
  });

  it.each([
    ['loopback', 'iif lo accept'],
    ['what is already connected', 'ct state established,related accept'],
    ['the DHCP lease', 'udp sport 67 udp dport 68 accept'],
    ['IPv6 neighbours', 'nd-neighbor-solicit'],
  ])('never drops %s', (_name, rule) => {
    const { ruleset } = withFakeNft('192.168.1.5\n');

    expect(ruleset).toContain(rule);
  });

  it('drops both families once something is allowed', () => {
    const { ruleset } = withFakeNft('192.168.1.5\n');

    expect(ruleset).toContain('meta nfproto ipv4 drop');
    expect(ruleset).toContain('meta nfproto ipv6 drop');
    expect(ruleset.indexOf('accept')).toBeLessThan(ruleset.indexOf('drop'));
  });

  it('removes the whole table when the list is empty', () => {
    const { result, ruleset, calls } = withFakeNft('\n\n');

    expect(result.code).toBe(0);
    expect(result.stdout).toContain('no restriction in place');
    expect(ruleset).toBe('');
    expect(calls).toContain('nft list table inet nas_launcher');
  });

  it('says so instead of guessing when there is no list at all', () => {
    fakeCommand(temp.path, 'nft', 'exit 0');
    const result = run(script, {
      env: {
        NAS_ACCESS_RULES: join(temp.path, 'missing'),
        PATH: `${temp.path}:${process.env.PATH}`,
      },
    });

    expect(result.code).not.toBe(0);
    expect(result.output).toContain('no list in');
  });

  it('says so instead of failing obscurely when nft is not installed', () => {
    writeFileSync(join(temp.path, 'access-rules'), '192.168.1.5\n');
    const result = run(script, {
      env: {
        NAS_ACCESS_RULES: join(temp.path, 'access-rules'),
        PATH: `${temp.path}:/usr/bin:/bin`,
      },
    });

    expect(result.code).not.toBe(0);
    expect(result.output).toContain('nft is not installed');
  });
});

describe('wake-alarm.sh', () => {
  const script = HELPERS.wake;

  function withClock(request: string, clock: string | null) {
    const requestPath = join(temp.path, 'wakealarm');
    const rtcPath = join(temp.path, 'rtc');
    writeFileSync(requestPath, request);
    if (clock !== null) {
      writeFileSync(rtcPath, clock);
    }

    const result = run(script, {
      env: { NAS_WAKE_REQUEST: requestPath, NAS_WAKE_RTC: rtcPath },
    });
    return { result, clock: clock === null ? '' : readFileSync(rtcPath, 'utf8') };
  }

  it('writes the time the launcher asked for', () => {
    const at = Math.floor(Date.now() / 1000) + 3600;
    const { result, clock } = withClock(`${at}`, '0');

    expect(result.code).toBe(0);
    expect(result.stdout).toContain('alarm set for');
    expect(clock.trim()).toBe(String(at));
  });

  it('clears the previous alarm before writing the new one', () => {
    const at = Math.floor(Date.now() / 1000) + 3600;
    const { result } = withClock(`${at}`, '1700000000');

    expect(result.code).toBe(0);
  });

  it.each([
    ['an hour ago', `${Math.floor(Date.now() / 1000) - 3600}`, 'already passed'],
    ['nothing readable', 'domani mattina', 'unreadable request'],
  ])('refuses %s', (_name, request, reason) => {
    const { result } = withClock(request, '0');

    expect(result.code).not.toBe(0);
    expect(result.output).toContain(reason);
  });

  it('says so when the machine has no clock alarm to write', () => {
    const { result } = withClock(`${Math.floor(Date.now() / 1000) + 3600}`, null);

    expect(result.code).not.toBe(0);
    expect(result.output).toContain('does not expose the clock alarm');
  });
});

describe('install-launcher-update.sh', () => {
  const script = HELPERS.update;
  const healthServers: ChildProcess[] = [];

  interface UpdateOptions {
    launcher?: string | null | false;
    gateway?: string;
    wizard?: string;
    reboot?: boolean;
    firewall?: boolean;
  }

  function nas() {
    const root = join(temp.path, 'root');
    const state = join(root, 'var/lib/nas-os');
    const calls = join(temp.path, 'calls');
    const at = (path: string) => join(root, path);
    const put = (path: string, body: string, mode = 0o644) => {
      mkdirSync(dirname(at(path)), { recursive: true });
      writeFileSync(at(path), body, { mode });
    };
    const flag = (name: string) => join(temp.path, name);
    const log = (name: string) => `printf '${name} %s\\n' "$*" >> ${JSON.stringify(calls)}\n`;

    put('usr/local/bin/nas-os-axum', 'old launcher\n');
    put('usr/local/bin/nas-os-gateway', 'old gateway\n');
    put('usr/local/lib/nas-wizard/wizard', 'old wizard\n');
    put(
      'usr/local/lib/nas-os-configuration/base-firewall.sh',
      `#!/usr/bin/env bash\n${log('old-firewall')}`,
      0o755,
    );
    put('etc/systemd/system/nas-os.service', 'old unit\n');
    mkdirSync(at('etc/systemd/system/multi-user.target.wants'), { recursive: true });
    symlinkSync(
      '../nas-os.service',
      at('etc/systemd/system/multi-user.target.wants/nas-os.service'),
    );
    put('etc/systemd/system/nas-os-kept.service', 'kept unit\n');
    symlinkSync(
      '../nas-os-kept.service',
      at('etc/systemd/system/multi-user.target.wants/nas-os-kept.service'),
    );
    put('etc/systemd/system/ssh.service', 'debian\n');
    put('etc/polkit-1/rules.d/10-nas-os-power.rules', 'old rule\n');
    put('etc/nas-os/network', 'eth0\n');
    put('var/lib/nas-os/launcher.db', 'old data\n');
    put('var/lib/private/nas-os-gateway/gateway.db', 'old sessions\n');
    mkdirSync(join(temp.path, 'net/eth0/device'), { recursive: true });
    writeFileSync(flag('launcher-up'), '');
    writeFileSync(flag('gateway-up'), '');

    const ports = [healthServer(flag('launcher-up')), healthServer(flag('gateway-up'))];

    fakeCommand(
      temp.path,
      'systemctl',
      `${log('systemctl')}case "$*" in
         'start nas-os') [[ -e ${JSON.stringify(flag('old-broken'))} ]] || touch ${JSON.stringify(flag('launcher-up'))} ;;
         'stop nas-os') rm -f ${JSON.stringify(flag('launcher-up'))} ;;
         'start nas-os-gateway' | 'try-restart nas-os-gateway') touch ${JSON.stringify(flag('gateway-up'))} ;;
         'stop nas-os-gateway') rm -f ${JSON.stringify(flag('gateway-up'))} ;;
         'is-active --quiet nas-os-kept.service') [[ -e ${JSON.stringify(at('etc/systemd/system/nas-os-kept.service'))} && -e ${JSON.stringify(flag('kept-up'))} ]] || exit 1 ;;
         'start nas-os nas-os-gateway') touch ${JSON.stringify(flag('launcher-up'))} ${JSON.stringify(flag('gateway-up'))} ;;
         'is-system-running') if [[ -e ${JSON.stringify(flag('stopping'))} ]]; then echo stopping; else echo running; fi ;;
       esac
       exit 0`,
    );
    fakeCommand(
      temp.path,
      'ip',
      `network=${JSON.stringify(at('etc/nas-os/network'))}
       [[ -f $network ]] || exit 0
       case "$*" in
         *address*) while read -r card; do
             [[ -n $card ]] && echo "2: $card    inet 192.168.1.14/24 scope global $card"
           done <"$network" ;;
         *route*) [[ -s $network ]] && echo 'default via 192.168.1.1 dev eth0' ;;
       esac
       exit 0`,
    );
    fakeCommand(
      temp.path,
      'nft',
      `[[ -f ${JSON.stringify(at('etc/nas-os/firewall'))} ]] || exit 1
       echo 'elements = { ${ports[1]} }'`,
    );
    fakeCommand(temp.path, 'udevadm', log('udevadm'));

    const env = {
      NAS_UPDATE_ROOT: root,
      NAS_HEALTH_PORT: String(ports[0]),
      NAS_GATEWAY_HEALTH_PORT: String(ports[1]),
      NAS_GATEWAY_TLS_PORT: String(ports[1]),
      NAS_HEALTH_TRIES: '2',
      NAS_BOOT_TRIES: '2',
      NAS_HEALTH_PAUSE: '0',
      NAS_UPDATE_PROBE: '',
      NAS_NET_DIR: join(temp.path, 'net'),
      PATH: `${temp.path}:${process.env.PATH}`,
    };
    const read = (path: string) => (existsSync(at(path)) ? readFileSync(at(path), 'utf8') : '');
    const there = (path: string) => existsSync(at(path)) || isLink(at(path));

    function release(component: string, installerPath: string, body: string) {
      const folder = join(temp.path, `release-${component}`);
      rmSync(folder, { recursive: true, force: true });
      mkdirSync(dirname(join(folder, installerPath)), { recursive: true });
      writeFileSync(join(folder, installerPath), body);
      execFileSync('tar', [
        '--create',
        '--file',
        join(state, `${component}-update.tar`),
        '--directory',
        folder,
        '.',
      ]);
    }

    const newLauncher = [
      `printf 'new launcher\\n' > ${JSON.stringify(at('usr/local/bin/nas-os-axum'))}`,
      `printf 'new unit\\n' > ${JSON.stringify(at('etc/systemd/system/nas-os-added.service'))}`,
      `ln -sf ../nas-os-added.service ${JSON.stringify(at('etc/systemd/system/multi-user.target.wants/nas-os-added.service'))}`,
      `printf 'new rule\\n' > ${JSON.stringify(at('etc/polkit-1/rules.d/99-nas-os-added.rules'))}`,
      `printf 'new journal\\n' > ${JSON.stringify(at('var/lib/nas-os/launcher.db-wal'))}`,
      `touch ${JSON.stringify(flag('launcher-up'))}`,
    ].join('\n');

    function update(options: UpdateOptions = {}) {
      if (options.firewall) {
        put('etc/nas-os/firewall', 'loaded\n');
      }
      if (options.launcher !== false) {
        if (options.launcher === null) {
          release('launcher', 'README', '');
        } else {
          release(
            'launcher',
            'packages/ui_installation/install.sh',
            `${newLauncher}\n${options.launcher ?? ''}`,
          );
        }
      }
      if (options.gateway !== undefined) {
        release(
          'gateway',
          'packages/gateway_installation/install.sh',
          `${log('gateway-installer')}touch ${JSON.stringify(flag('gateway-up'))}\n${options.gateway}`,
        );
      }
      if (options.wizard !== undefined) {
        release('wizard', 'install.sh', log('wizard-installer') + options.wizard);
      }
      return run(script, {
        env: { ...env, NAS_UPDATE_REBOOT: options.reboot === false ? 'no' : 'yes' },
      });
    }

    function verify() {
      return run(join(state, 'update-verify/verify.sh'), { args: ['--verify'], env });
    }

    const backups = () =>
      existsSync(join(state, 'backups')) ? readdirSync(join(state, 'backups')) : [];

    return {
      root,
      state,
      at,
      put,
      flag,
      read,
      there,
      update,
      verify,
      backups,
      status: () => read('var/lib/nas-os/launcher-update.status').trim(),
      calls: () => (existsSync(calls) ? readFileSync(calls, 'utf8') : ''),
      listing: () =>
        execFileSync('tar', ['--list', '--gzip', '--file', join(state, 'backups', backups()[0]!)], {
          encoding: 'utf8',
        }),
    };
  }

  function isLink(path: string) {
    try {
      return lstatSync(path).isSymbolicLink();
    } catch {
      return false;
    }
  }

  function healthServer(up: string) {
    const portFile = `${up}.port`;
    const server = spawn(process.execPath, [
      '-e',
      `const { existsSync, writeFileSync } = require('node:fs');
       const server = require('node:http').createServer((_, answer) => {
         answer.statusCode = existsSync(${JSON.stringify(up)}) ? 200 : 503;
         answer.end('ok');
       }).listen(0, () => writeFileSync(${JSON.stringify(portFile)}, String(server.address().port)));`,
    ]);
    healthServers.push(server);
    execFileSync('bash', [
      '-c',
      'for i in {1..150}; do [[ -s "$1" ]] && exit 0; sleep 0.1; done; exit 1',
      'wait-for-port',
      portFile,
    ]);
    return Number(readFileSync(portFile, 'utf8'));
  }

  let machine: ReturnType<typeof nas>;
  beforeEach(() => {
    machine = nas();
  });
  afterEach(() => {
    healthServers.splice(0).forEach((server) => server.kill());
  });

  function expectPrevious() {
    expect(machine.read('usr/local/bin/nas-os-axum')).toBe('old launcher\n');
    expect(machine.read('var/lib/nas-os/launcher.db')).toBe('old data\n');
    expect(machine.there('var/lib/nas-os/launcher.db-wal')).toBe(false);
    expect(machine.there('etc/systemd/system/nas-os-added.service')).toBe(false);
    expect(machine.there('etc/systemd/system/multi-user.target.wants/nas-os-added.service')).toBe(
      false,
    );
    expect(machine.there('etc/polkit-1/rules.d/99-nas-os-added.rules')).toBe(false);
    expect(machine.read('etc/polkit-1/rules.d/10-nas-os-power.rules')).toBe('old rule\n');
    expect(isLink(machine.at('etc/systemd/system/multi-user.target.wants/nas-os.service'))).toBe(
      true,
    );
    expect(machine.read('etc/systemd/system/ssh.service')).toBe('debian\n');
    expect(machine.there('var/lib/nas-os/update-verify')).toBe(false);
    expect(machine.there('etc/systemd/system/nas-os-update-verify.service')).toBe(false);
  }

  it('backs up, installs, checks the NAS and leaves a check for after the restart', () => {
    const result = machine.update();

    expect(result.code).toBe(0);
    expect(machine.status()).toBe('restarting launcher');
    expect(machine.read('usr/local/bin/nas-os-axum')).toBe('new launcher\n');
    const calls = machine.calls();
    expect(calls).toContain('systemctl stop nas-os\n');
    expect(calls).toContain('systemctl enable nas-os-update-verify.service\n');
    expect(calls.trim().split('\n').at(-1)).toBe('systemctl reboot');
    const unit = machine.read('etc/systemd/system/nas-os-update-verify.service');
    expect(unit).toContain(`ExecStart=/bin/bash ${machine.state}/update-verify/verify.sh --verify`);
    expect(unit).toContain('After=network-online.target nas-os.service nas-os-gateway.service');
    expect(unit).toContain('Type=exec');
    expect(unit).not.toContain('Type=oneshot');
    expect(machine.read('var/lib/nas-os/update-verify/pending')).toContain('phase=installed');
    expect(readdirSync(machine.state).some((name) => name.endsWith('-update.tar'))).toBe(false);
    expect(machine.backups()).toHaveLength(1);
  });

  it('backs up every file of the NAS it may write, and no file of Debian', () => {
    machine.update();
    const listing = machine.listing();

    expect(listing).toContain('usr/local/bin/nas-os-axum');
    expect(listing).toContain('etc/polkit-1/rules.d/10-nas-os-power.rules');
    expect(listing).toContain('etc/systemd/system/multi-user.target.wants/nas-os.service');
    expect(listing).toContain('usr/local/lib/nas-os-configuration/base-firewall.sh');
    expect(listing).toContain('var/lib/nas-os/launcher.db');
    expect(listing).not.toContain('ssh.service');
    expect(listing).not.toContain('nas-os-gateway/gateway.db');
    const backup = join(machine.state, 'backups', machine.backups()[0]!);
    expect(statSync(backup).mode & 0o777).toBe(0o600);
  });

  it('ends the update after the restart once the NAS works, and removes its check', () => {
    machine.update();
    const result = machine.verify();

    expect(result.code).toBe(0);
    expect(machine.status()).toBe('done launcher');
    expect(machine.read('usr/local/bin/nas-os-axum')).toBe('new launcher\n');
    expect(machine.calls()).toContain('systemctl disable nas-os-update-verify.service');
    expect(machine.there('var/lib/nas-os/update-verify')).toBe(false);
    expect(machine.there('etc/systemd/system/nas-os-update-verify.service')).toBe(false);
  });

  it('puts the previous version back when the NAS breaks only after the restart', () => {
    machine.update({ firewall: true });
    rmSync(machine.flag('launcher-up'));
    const result = machine.verify();

    expect(result.code).not.toBe(0);
    expect(machine.status()).toBe('rolled-back after the restart the launcher does not answer');
    expectPrevious();
    const calls = machine.calls();
    expect(calls).toContain('systemctl stop nas-os-added.service');
    expect(calls).toContain('old-firewall');
    expect(calls).toContain('udevadm control --reload');
    expect(calls).not.toContain('systemctl stop nas-os.service');
  });

  it('puts the previous version back at boot when the update was cut short', () => {
    const cut = machine.update({ launcher: 'kill -KILL $PPID' });
    expect(cut.code).not.toBe(0);
    expect(machine.read('var/lib/nas-os/update-verify/pending')).toContain('phase=installing');

    const result = machine.verify();

    expect(result.code).not.toBe(0);
    expect(machine.status()).toBe('rolled-back the update was cut short before it ended');
    expectPrevious();
  });

  it('puts the previous version back when the update is stopped while it installs', () => {
    const result = machine.update({ launcher: 'kill -TERM $PPID; sleep 0.3' });

    expect(result.code).not.toBe(0);
    expect(machine.status()).toBe('rolled-back the update was stopped before it ended');
    expectPrevious();
    expect(machine.calls()).not.toContain('reboot');
  });

  it('installs the wizard, the gateway and the launcher in this order, from one backup', () => {
    const result = machine.update({ gateway: '', wizard: '' });

    expect(result.code).toBe(0);
    expect(machine.status()).toBe('restarting wizard gateway launcher');
    const calls = machine
      .calls()
      .split('\n')
      .filter((line) => /^(systemctl (stop|reboot)|wizard-installer|gateway-installer)/.test(line));
    expect(calls).toEqual([
      'systemctl stop nas-os',
      'systemctl stop nas-os-gateway',
      'wizard-installer --update',
      'gateway-installer ',
      'systemctl reboot',
    ]);
    expect(machine.listing()).toContain('var/lib/private/nas-os-gateway/gateway.db');
    expect(machine.listing()).toContain('usr/local/lib/nas-wizard/wizard');
  });

  it('without a launcher release, starts the launcher again and ends at once', () => {
    const result = machine.update({ launcher: false, gateway: '' });

    expect(result.code).toBe(0);
    expect(machine.status()).toBe('done gateway');
    expect(machine.calls()).toContain('systemctl start nas-os\n');
    expect(machine.calls()).not.toContain('reboot');
    expect(machine.there('var/lib/nas-os/update-verify')).toBe(false);
  });

  it('puts everything back when one component fails, the wizard included', () => {
    const result = machine.update({
      wizard: `printf 'new wizard\\n' > ${JSON.stringify(machine.at('usr/local/lib/nas-wizard/wizard'))}`,
      gateway: `printf 'half gateway\\n' > ${JSON.stringify(machine.at('usr/local/bin/nas-os-gateway'))}; exit 1`,
    });

    expect(result.code).not.toBe(0);
    expect(machine.status()).toBe('rolled-back the installer of the gateway failed');
    expect(machine.read('usr/local/lib/nas-wizard/wizard')).toBe('old wizard\n');
    expect(machine.read('usr/local/bin/nas-os-gateway')).toBe('old gateway\n');
    expect(machine.read('var/lib/private/nas-os-gateway/gateway.db')).toBe('old sessions\n');
    expect(machine.calls()).toContain('systemctl start nas-os-gateway');
    expect(machine.calls()).not.toContain('reboot');
  });

  it('puts the previous version back when the installer of the launcher fails', () => {
    const result = machine.update({ launcher: 'exit 1' });

    expect(result.code).not.toBe(0);
    expect(machine.status()).toBe('rolled-back the installer of the launcher failed');
    expectPrevious();
    expect(machine.calls()).toContain('systemctl start nas-os\n');
  });

  it('starts again a unit of the NAS that the new version had removed', () => {
    const kept = machine.at('etc/systemd/system/nas-os-kept.service');
    const link = machine.at('etc/systemd/system/multi-user.target.wants/nas-os-kept.service');
    const result = machine.update({
      launcher: `rm -f ${JSON.stringify(kept)} ${JSON.stringify(link)}; exit 1`,
    });

    expect(result.code).not.toBe(0);
    expect(machine.read('etc/systemd/system/nas-os-kept.service')).toBe('kept unit\n');
    expect(isLink(link)).toBe(true);
    expect(machine.calls()).toContain('systemctl start --no-block nas-os-kept.service');
    expectPrevious();
  });

  it('rolls back when the new launcher does not answer', () => {
    const result = machine.update({
      launcher: `rm -f ${JSON.stringify(machine.flag('launcher-up'))}`,
    });

    expect(result.code).not.toBe(0);
    expect(machine.status()).toBe('rolled-back the launcher does not answer');
    expectPrevious();
  });

  it('rolls back when the gateway stops answering', () => {
    const result = machine.update({
      launcher: `rm -f ${JSON.stringify(machine.flag('gateway-up'))}`,
    });

    expect(result.code).not.toBe(0);
    expect(machine.status()).toMatch(/^rolled-back the gateway does not answer on port \d+$/);
    expectPrevious();
    expect(machine.calls()).toContain('systemctl try-restart nas-os-gateway');
  });

  it('rolls back when a network card loses its address', () => {
    const result = machine.update({
      launcher: `: > ${JSON.stringify(machine.at('etc/nas-os/network'))}`,
    });

    expect(result.code).not.toBe(0);
    expect(machine.status()).toBe(
      'rolled-back the interface eth0 lost its address, the default route is gone',
    );
    expect(machine.read('etc/nas-os/network')).toBe('eth0\n');
    expectPrevious();
  });

  it('rolls back when the base firewall loses the HTTPS port, and loads the previous one', () => {
    const result = machine.update({
      firewall: true,
      launcher: `rm -f ${JSON.stringify(machine.at('etc/nas-os/firewall'))}`,
    });

    expect(result.code).not.toBe(0);
    expect(machine.status()).toBe('rolled-back the base firewall is not loaded');
    expect(machine.calls()).toContain('old-firewall');
    expectPrevious();
  });

  it('does not ask for what did not work before the update', () => {
    rmSync(machine.at('etc/nas-os/network'));
    const result = machine.update({ reboot: false });

    expect(result.code).toBe(0);
    expect(machine.status()).toBe('done launcher');
  });

  it('says so when the previous version does not work either', () => {
    writeFileSync(machine.flag('old-broken'), '');
    const result = machine.update({
      launcher: `rm -f ${JSON.stringify(machine.flag('launcher-up'))}`,
    });

    expect(result.code).not.toBe(0);
    expect(machine.status()).toBe(
      'rollback-failed the launcher does not answer, and the previous version does not work either (the launcher does not answer)',
    );
    expect(machine.read('usr/local/bin/nas-os-axum')).toBe('old launcher\n');
    expect(machine.there('var/lib/nas-os/update-verify')).toBe(false);
  });

  it('removes nothing when the backup cannot be read', () => {
    const result = machine.update({
      launcher: `for f in ${JSON.stringify(join(machine.state, 'backups'))}/*.tar.gz; do : > "$f"; done; exit 1`,
    });

    expect(result.code).not.toBe(0);
    expect(machine.status()).toMatch(
      /^rollback-failed the installer of the launcher failed, and the backup .+ could not be restored$/,
    );
    expect(machine.read('usr/local/bin/nas-os-axum')).toBe('new launcher\n');
    expect(machine.read('etc/systemd/system/nas-os.service')).toBe('old unit\n');
  });

  it('gives up without touching the NAS after three rollbacks', () => {
    machine.update();
    const pending = machine.at('var/lib/nas-os/update-verify/pending');
    writeFileSync(
      pending,
      readFileSync(pending, 'utf8')
        .replace(/^attempts=\d+$/m, 'attempts=3')
        .replace(/^reason=.*$/m, 'reason=the launcher does not answer'),
    );
    const result = machine.verify();

    expect(result.code).not.toBe(0);
    expect(machine.status()).toBe('rollback-failed the launcher does not answer');
    expect(machine.read('usr/local/bin/nas-os-axum')).toBe('new launcher\n');
    expect(machine.there('var/lib/nas-os/update-verify')).toBe(false);
    expect(machine.calls()).toContain('systemctl start nas-os nas-os-gateway\n');
  });

  it('while the NAS shuts down, puts the files back and ends the rollback at the next start', () => {
    const result = machine.update({
      launcher: `touch ${JSON.stringify(machine.flag('stopping'))}; exit 1`,
    });

    expect(result.code).not.toBe(0);
    expect(machine.status()).toBe('rolling-back the installer of the launcher failed');
    expect(machine.read('usr/local/bin/nas-os-axum')).toBe('old launcher\n');
    const pending = machine.read('var/lib/nas-os/update-verify/pending');
    expect(pending).toContain('phase=rolling-back');
    expect(pending).toContain('attempts=0');
    expect(machine.there('etc/systemd/system/nas-os-update-verify.service')).toBe(true);
    expect(machine.calls()).not.toContain('systemctl start nas-os\n');

    rmSync(machine.flag('stopping'));
    const boot = machine.verify();

    expect(boot.code).not.toBe(0);
    expect(machine.status()).toBe('rolled-back the installer of the launcher failed');
    expectPrevious();
    expect(machine.calls()).toContain('systemctl start nas-os\n');
  });

  it('disables the check and starts the launcher again when the check cannot be prepared', () => {
    const units = machine.at('etc/systemd/system');
    chmodSync(units, 0o555);
    try {
      const result = machine.update();

      expect(result.code).not.toBe(0);
      expect(machine.status()).toBe('failed the check after the update could not be prepared');
    } finally {
      chmodSync(units, 0o755);
    }
    expect(machine.read('usr/local/bin/nas-os-axum')).toBe('old launcher\n');
    expect(machine.there('var/lib/nas-os/update-verify')).toBe(false);
    expect(machine.backups()).toHaveLength(0);
    const calls = machine.calls();
    expect(calls).toContain('systemctl disable nas-os-update-verify.service');
    expect(calls).toContain('systemctl start nas-os\n');
    expect(calls).not.toContain('reboot');
  });

  it('refuses a new update while the previous one waits for its check', () => {
    machine.update();
    const before = machine.calls();
    const result = machine.update();

    expect(result.code).not.toBe(0);
    expect(machine.status()).toBe('failed the previous update has not been checked yet');
    expect(machine.calls()).toBe(before);
  });

  it('refuses a release without an installer, and leaves the launcher running', () => {
    const result = machine.update({ launcher: null });

    expect(result.code).not.toBe(0);
    expect(machine.status()).toBe('failed the release of the launcher has no installer');
    expect(machine.calls()).toBe('');
    expect(machine.read('usr/local/bin/nas-os-axum')).toBe('old launcher\n');
  });

  it('keeps only the three latest backups once the update is checked', () => {
    const backups = join(machine.state, 'backups');
    mkdirSync(backups, { recursive: true });
    for (const name of ['20260101-000000', '20260102-000000', '20260103-000000']) {
      writeFileSync(join(backups, `${name}.tar.gz`), '');
    }

    machine.update();
    expect(machine.backups()).toHaveLength(4);
    machine.verify();

    expect(machine.backups()).toHaveLength(3);
    expect(machine.backups()).not.toContain('20260101-000000.tar.gz');
  });

  it('has nothing to do after a restart without an update', () => {
    const result = run(script, {
      args: ['--verify'],
      env: { NAS_UPDATE_ROOT: machine.root, PATH: `${temp.path}:${process.env.PATH}` },
    });

    expect(result.code).toBe(0);
    expect(machine.status()).toBe('');
  });

  it('says so when there is no release to install', () => {
    const result = run(script, {
      env: { NAS_UPDATE_ROOT: machine.root, PATH: `${temp.path}:${process.env.PATH}` },
    });

    expect(result.code).not.toBe(0);
    expect(result.output).toContain('no release in');
  });
});

describe('apt-updates.sh', () => {
  const script = HELPERS.apt;

  function job(name: string, selection?: string, aptExit = 0) {
    const state = join(temp.path, 'state');
    execFileSync('mkdir', ['-p', state]);
    if (selection !== undefined) {
      writeFileSync(join(state, 'apt-upgrade'), selection);
    }
    fakeCommand(
      temp.path,
      'apt-get',
      `printf '%s\\n' "$*" >> ${JSON.stringify(join(temp.path, 'calls'))}\nexit ${aptExit}`,
    );
    const result = run(script, {
      args: [name],
      env: { NAS_STATE_DIR: state, PATH: `${temp.path}:${process.env.PATH}` },
    });
    const calls = existsSync(join(temp.path, 'calls'))
      ? readFileSync(join(temp.path, 'calls'), 'utf8')
      : '';
    return { result, calls, state };
  }

  it('refreshes the index', () => {
    const { result, calls } = job('refresh');

    expect(result.code).toBe(0);
    expect(calls.trim()).toBe(
      'update -q -o APT::Update::Error-Mode=any -o DPkg::Lock::Timeout=300',
    );
  });

  it('upgrades the packages that were chosen, and only upgrades them', () => {
    const { result, calls, state } = job('upgrade', 'bash\nlibc6\n');

    expect(result.code).toBe(0);
    expect(calls).toContain('install -q -y --only-upgrade');
    expect(calls).toContain('--force-confold');
    expect(calls).toContain('-o DPkg::Lock::Timeout=300');
    expect(calls).toContain('-- bash libc6\n');
    expect(calls).not.toContain('autoremove');
    expect(calls).not.toContain('clean');
    expect(() => readFileSync(join(state, 'apt-upgrade'))).toThrow();
  });

  it('drops from the selection whatever is not a package name', () => {
    const { calls } = job('upgrade', 'bash\n-o APT::Get::Assume-Yes\n$(reboot)\n');

    expect(calls).toContain('-- bash\n');
    expect(calls).not.toContain('reboot');
  });

  it('refuses a selection with nothing to install', () => {
    const { result, calls } = job('upgrade', '--purge\n');

    expect(result.code).not.toBe(0);
    expect(result.output).toContain('no package name');
    expect(calls).toBe('');
  });

  it('says so when apt fails, and when the job is unknown', () => {
    expect(job('refresh', undefined, 100).result.output).toContain('could not be refreshed');
    const unknown = job('dist-upgrade');
    expect(unknown.result.code).not.toBe(0);
    expect(unknown.result.output).toContain('unknown job');
  });
});

describe('run-app.sh', () => {
  const script = HELPERS.app;

  function withPackage(id: string, exec: string, program = '#!/usr/bin/env bash\ntrue') {
    const apps = join(temp.path, 'apps');
    const sockets = join(temp.path, 'sockets');
    execFileSync('mkdir', ['-p', join(apps, id, 'bin'), sockets]);
    writeFileSync(
      join(apps, id, 'nas-app.json'),
      JSON.stringify({ manifest_version: 1, app: { id }, run: { exec } }),
    );
    if (!exec.startsWith('/') && !exec.includes('..')) {
      const file = join(apps, id, exec);
      writeFileSync(
        file,
        `#!/usr/bin/env bash\nprintf '%s\\n' "$*" > ${JSON.stringify(join(temp.path, 'called'))}\n${program}`,
      );
      execFileSync('chmod', ['+x', file]);
    }
    return { apps, sockets };
  }

  it('runs what the manifest declares, on the socket named after the application', () => {
    const { apps, sockets } = withPackage('fileExplorer', 'bin/file-explorer');

    const result = run(script, {
      args: ['fileExplorer'],
      env: { NAS_APPS_DIR: apps, NAS_APP_SOCKETS: sockets },
    });

    expect(result.code).toBe(0);
    expect(readFileSync(join(temp.path, 'called'), 'utf8').trim()).toBe(
      `--listen ${join(sockets, 'fileExplorer.sock')}`,
    );
  });

  it.each(['/usr/bin/id', '../../../usr/bin/id'])('refuses to execute %s', (exec) => {
    const { apps, sockets } = withPackage('fileExplorer', exec);

    const result = run(script, {
      args: ['fileExplorer'],
      env: { NAS_APPS_DIR: apps, NAS_APP_SOCKETS: sockets },
    });

    expect(result.code).toBe(4);
    expect(result.output).toContain('inside the package');
  });

  it.each(['', '../etc', 'file explorer', 'a;systemctl'])(
    'refuses %s as an application identifier',
    (id) => {
      const result = run(script, { args: [id], env: { NAS_APPS_DIR: temp.path } });

      expect(result.code).toBe(2);
      expect(result.output).toContain('not an application identifier');
    },
  );

  it('says so when the package has no manifest', () => {
    const result = run(script, { args: ['mediaServer'], env: { NAS_APPS_DIR: temp.path } });

    expect(result.code).toBe(3);
    expect(result.output).toContain('no manifest');
  });
});

describe('terminal-user.sh', () => {
  function withFakeSystem(passwd: string) {
    const calls = join(temp.path, 'calls');
    const log = `printf '%s %s\\n' "$(basename "$0")" "$*" >> ${JSON.stringify(calls)}`;
    fakeCommand(
      temp.path,
      'getent',
      `${log}\n[[ -n ${JSON.stringify(passwd)} ]] && echo ${JSON.stringify(passwd)} || exit 2`,
    );
    for (const name of ['useradd', 'passwd', 'gpasswd', 'systemctl']) {
      fakeCommand(temp.path, name, log);
    }
    fakeCommand(temp.path, 'id', 'echo "valerio sudo users"');
    return (input: string) => {
      const result = run(HELPERS.terminal, {
        input,
        env: {
          NAS_TERMINAL_DROPIN: join(temp.path, 'dropin'),
          PATH: `${temp.path}:${process.env.PATH}`,
        },
      });
      const read = (path: string) => (existsSync(path) ? readFileSync(path, 'utf8') : '');
      return { result, calls: read(calls), dropin: read(join(temp.path, 'dropin/user.conf')) };
    };
  }

  it('creates the user without sudo and enables the shells as that user', () => {
    const { result, calls, dropin } = withFakeSystem('')('valerio\n');

    expect(result.code).toBe(0);
    expect(calls).toContain('useradd --create-home --user-group --shell /bin/bash');
    expect(calls).toContain('passwd --lock valerio');
    expect(calls).toContain('gpasswd --delete valerio sudo');
    expect(calls).toContain('systemctl enable --now nas-os-terminal.socket');
    expect(dropin).toBe('[Service]\nUser=valerio\nGroup=valerio\n');
  });

  it('refuses a user that exists and is not the terminal one', () => {
    const { result, calls, dropin } = withFakeSystem('nas:x:1000:1000:Admin:/home/nas:/bin/bash')(
      'nas\n',
    );

    expect(result.code).not.toBe(0);
    expect(result.stderr).toContain('not the user of the terminal');
    expect(calls).not.toContain('useradd');
    expect(dropin).toBe('');
  });

  it('keeps the user it created before', () => {
    const { result, calls } = withFakeSystem(
      'valerio:x:1001:1001:nas-os terminal:/home/valerio:/bin/bash',
    )('valerio\n');

    expect(result.code).toBe(0);
    expect(calls).not.toContain('useradd');
  });

  it.each(['root; rm -rf /', '9lives', ''])('refuses %j as a name', (name) => {
    const { result, calls } = withFakeSystem('')(`${name}\n`);

    expect(result.code).not.toBe(0);
    expect(calls).toBe('');
  });
});

describe('apply-hostname.sh', () => {
  function rename(name: string, refuse = false) {
    const state = join(temp.path, 'state');
    mkdirSync(state, { recursive: true });
    const hosts = join(temp.path, 'hosts');
    const initial = '127.0.0.1 localhost\n127.0.1.1 debian\n192.168.1.10 printer\n';
    writeFileSync(hosts, initial);
    writeFileSync(join(state, 'hostname'), name);
    const calls = join(temp.path, 'hostname-calls');
    fakeCommand(temp.path, 'hostname', 'echo debian');
    fakeCommand(temp.path, 'systemctl', 'exit 0');
    fakeCommand(
      temp.path,
      'hostnamectl',
      `printf '%s\\n' "$*" >> ${JSON.stringify(calls)}\n${refuse ? 'echo refused >&2; exit 1' : 'exit 0'}`,
    );
    const restore = fakeCommand(temp.path, 'restore-system', 'exit 0');
    const result = run(HELPERS.hostname, {
      env: {
        NAS_STATE_DIR: state,
        NAS_HOSTS_FILE: hosts,
        NAS_CONFIGURATION_HELPER: restore,
        PATH: `${temp.path}:${process.env.PATH}`,
      },
    });
    return {
      result,
      hosts: readFileSync(hosts, 'utf8'),
      initial,
      calls: existsSync(calls) ? readFileSync(calls, 'utf8') : '',
      error: existsSync(join(state, 'hostname.error'))
        ? readFileSync(join(state, 'hostname.error'), 'utf8')
        : '',
    };
  }

  it('keeps the current hostname locally resolvable and preserves other hosts', () => {
    const changed = rename('nas-valerio');
    expect(changed.result.code).toBe(0);
    expect(changed.hosts).toContain('127.0.1.1\tnas-valerio\n');
    expect(changed.hosts).toContain('127.0.0.1 localhost\n');
    expect(changed.hosts).toContain('192.168.1.10 printer\n');
    expect(changed.calls).toContain('--static --transient hostname nas-valerio');
    expect(changed.error).toBe('');
  });

  it('keeps hosts unchanged when hostnamectl refuses the name', () => {
    const changed = rename('nas-valerio', true);
    expect(changed.result.code).not.toBe(0);
    expect(changed.hosts).toBe(changed.initial);
    expect(changed.error).toContain('refused');
  });

  it('rejects an invalid name before changing the system', () => {
    const changed = rename('bad/name');
    expect(changed.result.code).not.toBe(0);
    expect(changed.hosts).toBe(changed.initial);
    expect(changed.calls).toBe('');
  });
});

describe('factory-reset.sh', () => {
  function machine(
    options: {
      wizard?: boolean;
      terminal?: string;
      update?: 'working' | 'broken';
      original?: boolean;
      nftFailure?: boolean;
      pending?: boolean;
      configureFailure?: boolean;
    } = {},
  ) {
    const root = join(temp.path, 'machine');
    const path = (...parts: string[]) => join(root, ...parts);
    for (const directory of [
      'var/lib/nas-os/apps/nas-file-manager',
      'var/lib/private/nas-os-gateway',
      'var/lib/private/nas-app/nas-file-manager',
      'var/lib/nas-wizard/ui',
      'var/lib/nas-wizard-updates',
      'run',
      'etc/credstore.encrypted',
      'etc/systemd/system/nas-os-terminal@.service.d',
      'etc/systemd/system/nas-app@nas-file-manager.service.d',
      'srv/storage/v-data/.nas-users/valerio',
    ]) {
      mkdirSync(path(directory), { recursive: true });
    }
    writeFileSync(path('var/lib/nas-os/launcher.db'), 'db');
    writeFileSync(path('var/lib/private/nas-os-gateway/gateway.db'), 'accounts');
    writeFileSync(path('etc/credstore.encrypted/nas-os.launcher-token'), 'secret');
    writeFileSync(path('etc/credstore.encrypted/nas-os.smb-n-0123abcd'), 'sealed');
    writeFileSync(path('etc/credstore.encrypted/nas-os.luks-0f3a'), 'sealed');
    for (const file of [
      'nas-os-terminal.sock',
      'nas-os-share-password.sock',
      'nas-os-antivirus.lock',
      'nas-os-cleanup.lock',
    ]) {
      writeFileSync(path('run', file), 'stale runtime file');
    }
    writeFileSync(path('srv/storage/v-data/.nas-users/valerio/photo.jpg'), 'memories');
    writeFileSync(
      path('etc/systemd/system/nas-os-terminal@.service.d/user.conf'),
      '[Service]\nUser=valerio\nGroup=valerio\n',
    );
    if (options.wizard ?? true) {
      writeFileSync(path('etc/systemd/system/nas-wizard.service'), '[Service]\n');
    }

    const calls = join(temp.path, 'calls');
    const log = `printf '%s %s\\n' "$(basename "$0")" "$*" >> ${JSON.stringify(calls)}`;
    const home = path('usr/local/lib/nas-wizard');
    const launcherTools = path('usr/local/lib/nas-os');
    const gatewayTools = path('usr/local/lib/nas-os-gateway');
    mkdirSync(launcherTools, { recursive: true });
    mkdirSync(gatewayTools, { recursive: true });
    fakeCommand(launcherTools, 'uninstall.sh', log);
    fakeCommand(gatewayTools, 'uninstall.sh', log);
    mkdirSync(join(home, 'wizard/bin'), { recursive: true });
    mkdirSync(join(home, 'packages/os_configuration/scripts'), { recursive: true });
    fakeCommand(join(home, 'wizard/bin'), 'nas-wizard', 'exit 0');
    fakeCommand(
      join(home, 'packages/os_configuration/scripts'),
      'uninstall.sh',
      `${log}\nif [[ $1 == --check ]]; then exit ${options.original === false ? 1 : 0}; fi`,
    );
    fakeCommand(
      join(home, 'packages/os_configuration/scripts'),
      'firewall.sh',
      `${log}\n${options.nftFailure ? 'exit 1' : 'nft delete table inet nas_launcher\nnft delete table inet nas_base'}`,
    );
    fakeCommand(join(home, 'packages/os_configuration/scripts'), 'wifi.sh', log);
    fakeCommand(
      join(home, 'packages/os_configuration'),
      'install.sh',
      `${log}\n${options.configureFailure ? 'exit 1' : 'exit 0'}`,
    );
    if (options.pending) {
      mkdirSync(path('var/lib/nas-os/update-verify'), { recursive: true });
      writeFileSync(path('var/lib/nas-os/update-verify/pending'), 'phase=installed\n');
    }
    fakeCommand(
      temp.path,
      'nft',
      `${log}\n${options.nftFailure ? 'exit 1' : 'if [[ $* == "list tables" ]]; then printf "table inet nas_launcher\\ntable inet nas_base\\n"; elif [[ $* == "list table inet nas_base" ]]; then printf "set nas_shares {\\nset nas_sync {\\nset nas_discovery {\\n"; fi'}`,
    );
    if (options.update) {
      const release = join(temp.path, 'wizard-release');
      mkdirSync(release, { recursive: true });
      writeFileSync(
        join(release, 'install.sh'),
        `printf 'install.sh %s\\n' "$*" >> ${JSON.stringify(calls)}\n` +
          (options.update === 'broken' ? 'exit 1\n' : ''),
      );
      execFileSync('tar', ['-cf', path('var/lib/nas-os/wizard-update.tar'), '-C', release, '.']);
    }
    for (const name of ['systemctl', 'userdel', 'pkill', 'udevadm']) {
      fakeCommand(temp.path, name, log);
    }
    const passwd =
      options.terminal ?? 'valerio:x:1001:1001:nas-os terminal:/home/valerio:/bin/bash';
    fakeCommand(temp.path, 'getent', `${log}\necho ${JSON.stringify(passwd)}`);

    const result = run(HELPERS.reset, {
      env: {
        NAS_STATE_DIR: path('var/lib/nas-os'),
        NAS_GATEWAY_STATES: path('var/lib/private/nas-os-gateway'),
        NAS_APP_STATES: path('var/lib/private/nas-app'),
        NAS_CREDENTIAL: path('etc/credstore.encrypted/nas-os.launcher-token'),
        NAS_TERMINAL_DROPIN: path('etc/systemd/system/nas-os-terminal@.service.d'),
        NAS_WIZARD_UNIT: path('etc/systemd/system/nas-wizard.service'),
        NAS_WIZARD_STATE: path('var/lib/nas-wizard'),
        NAS_WIZARD_UPDATES: path('var/lib/nas-wizard-updates'),
        NAS_LAUNCHER_TOOLS: launcherTools,
        NAS_GATEWAY_TOOLS: gatewayTools,
        NAS_APP_DROPINS: path('etc/systemd/system'),
        NAS_STORAGE_RULE: path('etc/udev/rules.d/90-nas-os-storage.rules'),
        NAS_WIZARD_HOME: home,
        NAS_LOG_DIR: path('var/log/nas-os'),
        NAS_RUNTIME_DIR: path('run/nas-os'),
        PATH: `${temp.path}:${process.env.PATH}`,
      },
    });
    return {
      result,
      calls: existsSync(calls) ? readFileSync(calls, 'utf8') : '',
      exists: (part: string) => existsSync(path(part)),
      read: (part: string) => readFileSync(path(part), 'utf8'),
    };
  }

  it('removes the data of launcher, gateway and applications, and starts the wizard', () => {
    const { result, calls, exists } = machine();

    expect(result.code).toBe(0);
    for (const removed of [
      'var/lib/nas-os',
      'var/lib/private/nas-os-gateway',
      'var/lib/private/nas-app',
      'var/lib/nas-wizard',
      'var/lib/nas-wizard-updates',
      'run/nas-os-terminal.sock',
      'run/nas-os-share-password.sock',
      'run/nas-os-antivirus.lock',
      'run/nas-os-cleanup.lock',
      'etc/credstore.encrypted/nas-os.launcher-token',
      'etc/credstore.encrypted/nas-os.smb-n-0123abcd',
      'etc/systemd/system/nas-os-terminal@.service.d',
      'etc/systemd/system/nas-app@nas-file-manager.service.d',
    ]) {
      expect(exists(removed), removed).toBe(false);
    }
    expect(exists('etc/credstore.encrypted/nas-os.luks-0f3a')).toBe(true);
    expect(calls).toContain('systemctl disable --now nas-os-gateway.service');
    expect(calls).toContain('systemctl disable --now nas-os.service');
    expect(calls).toContain('systemctl disable --now nas-os-storage.timer');
    expect(calls).toContain('uninstall.sh --remove-terminal-user valerio');
    expect(calls).toContain('nft delete table inet nas_launcher');
    expect(calls).toContain('uninstall.sh --restore');
    expect(calls.trim().split('\n').at(-1)).toBe('systemctl is-active --quiet nas-wizard.service');
  });

  it('ends as the entry script left the machine, firewall, wireless check and screen as installed', () => {
    const { result, calls } = machine();

    expect(result.code).toBe(0);
    expect(calls).not.toContain('nft delete table inet nas_base');
    expect(calls).not.toContain('firewall.sh --remove');
    expect(calls).not.toContain('wifi.sh --remove');
    for (const set of [
      'nas_shares',
      'nas_sync',
      'nas_discovery',
      'nas_apps_out',
      'nas_smb_servers',
    ]) {
      expect(calls).toContain(`nft flush set inet nas_base ${set}\n`);
    }
    expect(calls).toContain('systemctl disable --now nas-os-screen.service');
    const lines = calls.trim().split('\n');
    const configured = lines.indexOf('install.sh ');
    expect(configured).toBeGreaterThan(lines.indexOf('uninstall.sh --restore'));
    expect(configured).toBeLessThan(lines.indexOf('systemctl enable --now nas-wizard.service'));
  });

  it('starts the wizard even when the configuration of the system cannot be applied again', () => {
    const { result, calls } = machine({ configureFailure: true });

    expect(result.code).toBe(0);
    expect(calls).toContain('install.sh \n');
    expect(calls.trim().split('\n').at(-1)).toBe('systemctl is-active --quiet nas-wizard.service');
  });

  it('removes nothing while an update waits for its check after the restart', () => {
    const { result, exists, calls } = machine({ pending: true });

    expect(result.code).not.toBe(0);
    expect(result.stderr).toContain('an update waits for its check');
    expect(exists('var/lib/nas-os/launcher.db')).toBe(true);
    expect(calls).not.toContain('disable --now');
  });

  it('installs a newer wizard the launcher downloaded before anything is removed', () => {
    const { result, calls } = machine({ update: 'working' });

    expect(result.code).toBe(0);
    const lines = calls.trim().split('\n');
    expect(lines.indexOf('install.sh --update')).toBeLessThan(
      lines.indexOf('systemctl disable --now nas-os-gateway.service'),
    );
    expect(result.stdout).toContain('the wizard was updated');
    expect(lines.at(-1)).toBe('systemctl is-active --quiet nas-wizard.service');
  });

  it('goes on with the installed wizard when its update fails', () => {
    const { result, calls, exists } = machine({ update: 'broken' });

    expect(result.code).toBe(0);
    expect(calls).toContain('install.sh --update');
    expect(exists('var/lib/nas-os')).toBe(false);
    expect(calls.trim().split('\n').at(-1)).toBe('systemctl is-active --quiet nas-wizard.service');
  });

  it('never touches the data disks', () => {
    const { result, read } = machine();

    expect(result.code).toBe(0);
    expect(read('srv/storage/v-data/.nas-users/valerio/photo.jpg')).toBe('memories');
  });

  it('removes only the user it created for the terminal', () => {
    const { result, calls } = machine({
      terminal: 'valerio:x:1000:1000:Admin:/home/valerio:/bin/bash',
    });

    expect(result.code).toBe(0);
    expect(calls).not.toContain('userdel');
    expect(calls).not.toContain('--remove-terminal-user');
  });

  it('removes nothing when the wizard is not there to configure the NAS again', () => {
    const { result, calls, exists } = machine({ wizard: false });

    expect(result.code).not.toBe(0);
    expect(result.stderr).toContain('the wizard is not installed');
    expect(exists('var/lib/nas-os/launcher.db')).toBe(true);
    expect(calls).toBe('');
  });
  it('removes no data without the original configuration or when the firewall cannot be inspected', () => {
    for (const options of [{ original: false }, { nftFailure: true }]) {
      const { result, exists } = machine(options);
      expect(result.code).not.toBe(0);
      expect(exists('var/lib/nas-os/launcher.db')).toBe(true);
      expect(result.stdout).not.toContain('the original system configuration is restored');
    }
  });
});

describe('apply-app-grants.sh', () => {
  function grant(request: string | null, id = 'nas-file-manager') {
    const requests = join(temp.path, 'app-grants');
    const units = join(temp.path, 'system');
    mkdirSync(requests, { recursive: true });
    mkdirSync(units, { recursive: true });

    if (request === null) {
      rmSync(join(requests, id), { force: true });
    } else if (id) {
      writeFileSync(join(requests, id), request);
    }
    const calls = join(temp.path, 'calls');
    fakeCommand(temp.path, 'systemctl', `printf '%s\\n' "$*" >> ${JSON.stringify(calls)}`);
    const result = run(HELPERS.grants, {
      args: [id],
      env: {
        NAS_GRANTS_DIR: requests,
        NAS_APP_DROPINS: units,
        PATH: `${temp.path}:${process.env.PATH}`,
      },
    });
    const conf = join(units, `nas-app@${id}.service.d`, 'grants.conf');
    return {
      result,
      conf: existsSync(conf) ? readFileSync(conf, 'utf8') : null,
      calls: existsSync(calls) ? readFileSync(calls, 'utf8') : '',
    };
  }

  it('gives the data disks to who asked for them, in their group', () => {
    const { result, conf, calls } = grant('/srv/storage\n');

    expect(result.code).toBe(0);
    expect(conf).toContain('BindPaths=/srv/storage');
    expect(conf).toContain('SupplementaryGroups=nas-storage');
    expect(conf).toContain('Environment=NAS_STORAGE_ROOT=/srv/storage');
    expect(conf).not.toContain('InaccessiblePaths');
    expect(conf).not.toContain('ProcSubset');
    expect(calls.trim()).toBe('daemon-reload');
  });

  it('shows the kernel views granted, and keeps hiding the rest', () => {
    const disks = grant('/proc\n/sys\n/run/udev\n', 'nas-disk-manager').conf;
    expect(disks).toContain('ProcSubset=all');
    expect(disks).toContain('ProtectProc=default');
    expect(disks).toMatch(/^InaccessiblePaths=$/m);
    expect(disks).not.toMatch(/^InaccessiblePaths=-/m);
    expect(disks).not.toContain('/srv/storage');

    const onlySys = grant('/sys', 'probe').conf;
    expect(onlySys).toMatch(/^InaccessiblePaths=$/m);
    expect(onlySys).toMatch(/^InaccessiblePaths=-\/run\/udev$/m);
    expect(onlySys).not.toContain('ProtectProc');
  });

  it('grants System Monitor read access to process metadata and sensors without storage or capabilities', () => {
    const { result, conf } = grant('/proc\n/sys\n', 'nas-system-monitor');
    expect(result.code).toBe(0);
    expect(conf).toContain('ProcSubset=all');
    expect(conf).toContain('ProtectProc=default');
    expect(conf).toMatch(/^InaccessiblePaths=-\/run\/udev$/m);
    expect(conf).not.toContain('BindPaths');
    expect(conf).not.toContain('CapabilityBoundingSet');
  });

  it('lets an application granted SMTP into the firewall set that reaches the mail ports', () => {
    const { result, conf } = grant('network:smtp\n', 'nas-mail-client');
    expect(result.code).toBe(0);
    expect(conf).toContain('NFTSet=cgroup:inet:nas_base:nas_apps_smtp');
    expect(conf).not.toContain('BindPaths');

    expect(grant('network:ftp\n', 'nas-mail-client').result.code).not.toBe(0);
  });

  it('lets an application granted the sync into the firewall set that reaches the local network', () => {
    const { result, conf } = grant('/srv/storage\nnetwork:sync\n', 'nas-sync-manager');
    expect(result.code).toBe(0);
    expect(conf).toContain('NFTSet=cgroup:inet:nas_base:nas_apps_sync');
    expect(conf).not.toContain('nas_apps_smtp');
  });

  it('lowers the priority of an application that works without its window', () => {
    const { result, conf } = grant('/srv/storage\nrun:background\n', 'nas-backup-manager');
    expect(result.code).toBe(0);
    for (const line of [
      'Nice=10',
      'CPUWeight=50',
      'IOSchedulingClass=best-effort',
      'IOSchedulingPriority=7',
    ]) {
      expect(conf).toMatch(new RegExp(`^${line}$`, 'm'));
    }
    expect(conf).toContain('BindPaths=/srv/storage');
    expect(grant('/srv/storage\n', 'nas-backup-manager').conf).not.toContain('Nice=');
    expect(grant('run:root\n', 'nas-backup-manager').result.code).not.toBe(0);
  });

  it('takes the drop-in away when nothing is granted any more', () => {
    grant('/srv/storage\n');
    expect(grant('').conf).toBeNull();

    grant('/srv/storage\n');
    const removed = grant(null);
    expect(removed.result.code).toBe(0);
    expect(removed.conf).toBeNull();
    expect(existsSync(join(temp.path, 'system', 'nas-app@nas-file-manager.service.d'))).toBe(false);
  });

  it('refuses a path it does not know and changes nothing', () => {
    const before = grant('/srv/storage\n').conf;
    const refused = grant('/srv/storage\n/etc\n');

    expect(refused.result.code).not.toBe(0);
    expect(refused.result.stderr).toContain('asks for /etc');
    expect(refused.conf).toBe(before);
  });

  it('refuses what is not an application identifier', () => {
    for (const id of ['../etc', 'a b', '']) {
      expect(grant('/srv/storage\n', id).result.code, id).not.toBe(0);
    }
  });
});

describe('act-on-process.sh', () => {
  function machine(processes: { pid: number; parent?: number; unit?: string }[]) {
    const proc = join(temp.path, 'proc');
    for (const { pid, parent = 1, unit } of processes) {
      mkdirSync(join(proc, String(pid)), { recursive: true });
      writeFileSync(
        join(proc, String(pid), 'stat'),
        `${pid} (odd (name)) S ${parent} ${pid} ${pid} 0 -1 4194304 1 0 0 0 11 7 0 0 20 0 1 0 900 0 2048\n`,
      );
      const path = unit ? `/system.slice/${unit}/worker` : '/';
      writeFileSync(join(proc, String(pid), 'cgroup'), `0::${path}\n`);
    }
    const calls = join(temp.path, 'calls');
    fakeCommand(
      temp.path,
      'systemctl',
      `printf 'systemctl %s\\n' "$*" >> ${JSON.stringify(calls)}`,
    );
    return (instance: string) => {
      const result = run(HELPERS.process, {
        args: [instance],
        env: {
          NAS_PROC: proc,
          PATH: `${temp.path}:${process.env.PATH}`,

          'BASH_FUNC_kill%%': `() { printf 'kill %s\\n' "$*" >> ${JSON.stringify(calls)}; }`,
        },
      });
      return { result, calls: existsSync(calls) ? readFileSync(calls, 'utf8') : '' };
    };
  }

  it('terminates a process the NAS does not depend on', () => {
    const act = machine([{ pid: 400, unit: 'cron.service' }]);
    const { result, calls } = act('terminate-400-900');

    expect(result.code).toBe(0);
    expect(calls.trim()).toBe('kill -TERM -- 400');
  });

  it('stops the whole NAS application service instead of only signalling one process', () => {
    const act = machine([{ pid: 400, unit: 'nas-app@nas-picture-manager.service' }]);
    const { result, calls } = act('terminate-400-900');

    expect(result.code).toBe(0);
    expect(calls.trim()).toBe('systemctl stop -- nas-app@nas-picture-manager.service');
    expect(calls).not.toContain('kill');
    expect(act('terminate-400-899').result.stderr).toContain('no longer the one shown');
  });

  it('reports a failure when the NAS application service cannot be stopped', () => {
    const act = machine([{ pid: 400, unit: 'nas-app@nas-picture-manager.service' }]);
    fakeCommand(temp.path, 'systemctl', 'echo "service stop failed" >&2; exit 1');
    const { result, calls } = act('terminate-400-900');
    expect(result.code).not.toBe(0);
    expect(result.stderr).toContain('service stop failed');
    expect(calls).toBe('');
  });

  it('restarts the service of a process, without waiting when it is the launcher', () => {
    const act = machine([
      { pid: 400, unit: 'cron.service' },
      { pid: 401, unit: 'nas-os.service' },
    ]);

    expect(act('restart-400-900').calls).toContain('systemctl restart -- cron.service');
    expect(act('restart-401-900').calls).toContain(
      'systemctl restart --no-block -- nas-os.service',
    );
  });

  it('only restarts what keeps the NAS working', () => {
    const act = machine([
      { pid: 401, unit: 'nas-os.service' },
      { pid: 402, unit: 'systemd-journald.service' },
    ]);

    for (const pid of [401, 402]) {
      const { result, calls } = act(`terminate-${pid}-900`);
      expect(result.code, String(pid)).not.toBe(0);
      expect(result.stderr).toContain('it can only be restarted');
      expect(calls).toBe('');
    }
  });

  it('leaves the kernel, the first process and a process that changed alone', () => {
    const act = machine([
      { pid: 1, parent: 0, unit: 'init.scope' },
      { pid: 50, parent: 2 },
      { pid: 400, unit: 'cron.service' },
    ]);

    expect(act('terminate-50-900').result.stderr).toContain('belongs to the kernel');
    expect(act('terminate-1-900').result.code).not.toBe(0);
    expect(act('terminate-400-899').result.stderr).toContain('no longer the one shown');
    expect(act('terminate-999-900').result.stderr).toContain('no longer running');
    expect(act('restart-400-899').calls).toBe('');
  });

  it('restarts only a service, never a scope or its own unit', () => {
    const act = machine([
      { pid: 400, unit: 'session-3.scope' },
      { pid: 401, unit: 'nas-os-process@restart-401-900.service' },
    ]);

    for (const instance of ['restart-400-900', 'restart-401-900']) {
      const { result, calls } = act(instance);
      expect(result.stderr, instance).toContain('not in a service that can be restarted');
      expect(calls).toBe('');
    }
  });

  it('refuses what is not a request for a process', () => {
    const act = machine([{ pid: 400, unit: 'cron.service' }]);
    for (const instance of ['', 'stop-400-900', 'terminate-0400-900', 'terminate-400', 'kill']) {
      expect(act(instance).result.code, instance).not.toBe(0);
    }
    expect(act('terminate-400-900; reboot').result.stderr).toContain('not a request');
  });
});

describe('app-packages.sh', () => {
  function packages(id: string, requests: Record<string, string>, present: string[] = []) {
    const directory = join(temp.path, 'app-packages');
    const record = join(temp.path, 'record', 'installed');
    const dpkg = join(temp.path, 'dpkg');
    const calls = join(temp.path, 'calls');
    rmSync(directory, { recursive: true, force: true });
    mkdirSync(directory, { recursive: true });
    for (const [app, text] of Object.entries(requests)) {
      writeFileSync(join(directory, app), text);
    }
    if (!existsSync(dpkg)) {
      writeFileSync(dpkg, present.map((name) => `${name}\n`).join(''));
    }
    fakeCommand(
      temp.path,
      'dpkg-query',
      `if (($# == 2)); then awk '{ print $0 "\\tinstalled" }' ${JSON.stringify(dpkg)}; else grep -qxF -- "\${@: -1}" ${JSON.stringify(dpkg)} && printf 'install ok installed'; fi`,
    );
    fakeCommand(
      temp.path,
      'apt-get',
      `printf '%s\\n' "$*" >> ${JSON.stringify(calls)}
name=\${@: -1}
case $1 in
  install) printf '%s\\n' "$name" >> ${JSON.stringify(dpkg)} ;;
  purge) grep -vxF -- "$name" ${JSON.stringify(dpkg)} > ${JSON.stringify(`${dpkg}.new`)}; mv ${JSON.stringify(`${dpkg}.new`)} ${JSON.stringify(dpkg)} ;;
esac`,
    );
    const restore = fakeCommand(temp.path, 'restore-system', 'exit 0');
    const result = run(HELPERS.packages, {
      args: [id],
      env: {
        NAS_APP_PACKAGES_DIR: directory,
        NAS_APP_PACKAGES_RECORD: record,
        NAS_CONFIGURATION_HELPER: restore,
        PATH: `${temp.path}:${process.env.PATH}`,
      },
    });
    return {
      result,
      calls: existsSync(calls) ? readFileSync(calls, 'utf8') : '',
      record: existsSync(record) ? readFileSync(record, 'utf8') : '',
    };
  }

  it('installs what an application asks for and purges it when no application asks any more', () => {
    const installed = packages('nas-mail-client', { 'nas-mail-client': 'msmtp\n' });
    expect(installed.result.code).toBe(0);
    expect(installed.calls).toContain(
      'install -q -y --no-install-recommends -o DPkg::Lock::Timeout=300 -- msmtp',
    );
    expect(installed.record).toBe('msmtp\n');
    expect(installed.calls).not.toContain('autoremove');
    expect(installed.calls).not.toContain('clean');

    const removed = packages('nas-mail-client', {});
    expect(removed.calls).toContain('purge -q -y -o DPkg::Lock::Timeout=300 -- msmtp');
    expect(removed.record).toBe('');
  });

  it('keeps a package another application still asks for, and one it did not install', () => {
    packages('nas-mail-client', { 'nas-mail-client': 'msmtp\n', other: 'msmtp\n' });
    const shared = packages('nas-mail-client', { other: 'msmtp\n' });
    expect(shared.calls).not.toMatch(/^purge /m);
    expect(shared.record).toBe('msmtp\n');
  });

  it('never removes a package that was on the machine before', () => {
    const before = packages('nas-mail-client', { 'nas-mail-client': 'msmtp\n' }, ['msmtp']);
    expect(before.calls).toBe('');
    expect(before.record).toBe('');
    expect(packages('nas-mail-client', {}).calls).toBe('');
  });

  it('refuses a package outside its list and an identifier that is not one', () => {
    const refused = packages('nas-mail-client', { 'nas-mail-client': 'postfix\n' });
    expect(refused.result.code).not.toBe(0);
    expect(refused.result.output).toContain('postfix');
    expect(refused.calls).toBe('');
    expect(packages('../etc', {}).result.code).not.toBe(0);
  });
  it('installs ClamAV through the same helper and exposes apt progress only on explicit request', () => {
    fakeCommand(
      temp.path,
      'install',
      `args=()
while (($#)); do
  case $1 in
    -o|-g) shift 2 ;;
    *) args+=("$1"); shift ;;
  esac
done
/usr/bin/install "\${args[@]}"`,
    );
    const installed = packages('nas-security-center', {
      'nas-security-center': 'clamav\nclamav-freshclam\n',
    });
    expect(installed.result.code).toBe(0);
    expect(installed.record).toBe('clamav\nclamav-freshclam\n');
    expect(installed.calls).toContain('-o APT::Status-Fd=3 -- clamav');
    expect(installed.calls).toContain('-o APT::Status-Fd=3 -- clamav-freshclam');
  });
});

describe('apply-sync-ports.sh', () => {
  function ports(files: Record<string, string>, nft = '') {
    const requests = join(temp.path, 'sync-ports');
    const calls = join(temp.path, 'calls');
    rmSync(requests, { recursive: true, force: true });
    rmSync(calls, { force: true });
    mkdirSync(requests, { recursive: true });
    for (const [id, text] of Object.entries(files)) {
      writeFileSync(join(requests, id), text);
    }
    fakeCommand(temp.path, 'nft', `printf 'nft %s\\n' "$*" >> ${JSON.stringify(calls)}\n${nft}`);
    const result = run(HELPERS.syncPorts, {
      env: { PATH: `${temp.path}:${process.env.PATH}`, NAS_SYNC_PORTS_DIR: requests },
    });
    return { ...result, calls: existsSync(calls) ? readFileSync(calls, 'utf8') : '' };
  }

  it('opens the fixed ports while a sync is on, and only then', () => {
    const open = ports({ 'nas-sync-manager': 'on' });
    expect(open.code).toBe(0);
    expect(open.calls).toBe(
      'nft flush set inet nas_base nas_sync\nnft add element inet nas_base nas_sync { 22000, 21027 }\n',
    );
    const closed = ports({});
    expect(closed.code).toBe(0);
    expect(closed.stdout).toContain('ports closed');
    expect(closed.calls).toBe('nft flush set inet nas_base nas_sync\n');
  });

  it('refuses a request it does not know, and says when the firewall has no set', () => {
    expect(ports({ 'nas-sync-manager': '22, 80' }).code).not.toBe(0);
    expect(ports({ 'bad name': 'on' }).code).not.toBe(0);
    const missing = ports({ 'nas-sync-manager': 'on' }, 'exit 1');
    expect(missing.code).not.toBe(0);
    expect(missing.output).toContain('nas_sync is missing');
  });
});

describe('apply-discovery.sh', () => {
  function discovery(request: string | null, nft = '') {
    const file = join(temp.path, 'discovery');
    const calls = join(temp.path, 'calls');
    rmSync(file, { force: true });
    rmSync(calls, { force: true });
    if (request !== null) {
      writeFileSync(file, request);
    }
    fakeCommand(temp.path, 'nft', `printf 'nft %s\\n' "$*" >> ${JSON.stringify(calls)}\n${nft}`);
    const result = run(HELPERS.discovery, {
      env: { PATH: `${temp.path}:${process.env.PATH}`, NAS_DISCOVERY_REQUEST: file },
    });
    return { ...result, calls: existsSync(calls) ? readFileSync(calls, 'utf8') : '' };
  }

  it('opens only the fixed port while the option is on, and closes it otherwise', () => {
    const open = discovery('on');
    expect(open.code).toBe(0);
    expect(open.calls).toBe(
      'nft flush set inet nas_base nas_discovery\nnft add element inet nas_base nas_discovery { 39877 }\n',
    );
    const closed = discovery(null);
    expect(closed.code).toBe(0);
    expect(closed.stdout).toContain('port closed');
    expect(closed.calls).toBe('nft flush set inet nas_base nas_discovery\n');
  });

  it('refuses a request it does not know, and says when the firewall has no set', () => {
    const odd = discovery('22, 80');
    expect(odd.code).not.toBe(0);
    expect(odd.calls).toBe('');
    const missing = discovery('on', 'exit 1');
    expect(missing.code).not.toBe(0);
    expect(missing.output).toContain('nas_discovery is missing');
  });
});

describe('apply-smb-shares.sh', () => {
  function nas({
    samba = true,
    testparm = 'exit 0',
    conf = '',
    mounted = 'exit 0',
    fstype = 'echo ext4',
  } = {}) {
    const storage = join(temp.path, 'storage');
    const requests = join(temp.path, 'requests');
    const samba_dir = join(temp.path, 'samba');
    const dropin = join(temp.path, 'dropin');
    const calls = join(temp.path, 'calls');
    for (const directory of [join(storage, 'disk', 'Foto'), join(storage, 'disk', 'Musica')]) {
      mkdirSync(directory, { recursive: true });
    }
    mkdirSync(requests, { recursive: true });
    mkdirSync(samba_dir, { recursive: true });
    mkdirSync(dropin, { recursive: true });
    writeFileSync(join(dropin, 'user.conf'), '[Service]\nUser=valerio\nGroup=valerio\n');
    if (conf) {
      writeFileSync(join(samba_dir, 'smb.conf'), conf);
    }
    const record = (name: string) =>
      fakeCommand(temp.path, name, `printf '${name} %s\\n' "$*" >> ${JSON.stringify(calls)}`);
    record('systemctl');
    record('nft');
    fakeCommand(temp.path, 'getent', 'exit 0');
    fakeCommand(temp.path, 'mountpoint', mounted);
    fakeCommand(temp.path, 'findmnt', fstype);
    if (samba) {
      fakeCommand(temp.path, 'smbd', 'exit 0');
      fakeCommand(temp.path, 'testparm', testparm);
    }
    const apply = (files: Record<string, string> = {}) => {
      for (const [id, text] of Object.entries(files)) {
        writeFileSync(join(requests, id), text);
      }
      const result = run(HELPERS.shares, {
        env: {
          PATH: samba ? `${temp.path}:${process.env.PATH}` : `${temp.path}:/usr/bin:/bin`,
          NAS_SHARES_DIR: requests,
          NAS_SAMBA_DIR: samba_dir,
          NAS_STORAGE_ROOT: storage,
          NAS_TERMINAL_DROPIN: dropin,
          NAS_APP_PACKAGES_RECORD: join(temp.path, 'installed'),
        },
      });
      const read = (path: string) => (existsSync(path) ? readFileSync(path, 'utf8') : '');
      return {
        ...result,
        conf: read(join(samba_dir, 'smb.conf')),
        original: read(join(samba_dir, 'smb.conf.nas-os-original')),
        calls: read(calls),
      };
    };
    return { storage, apply };
  }

  it('shares each folder as the account user, protected or open, and opens 445 meanwhile', () => {
    const { storage, apply } = nas({ conf: '[global]\n   workgroup = DEBIAN\n' });
    const result = apply({
      'nas-file-manager': `Foto\t${storage}/disk/Foto\tro\tprotected\nMusica di casa\t${storage}/disk/Musica\trw\topen\n`,
    });

    expect(result.code).toBe(0);
    expect(result.stdout).toContain('2 folders shared');
    expect(result.conf.split('\n')[0]).toContain('# Written by apply-smb-shares.sh');
    expect(result.conf).toContain(`[Foto]\n   path = ${storage}/disk/Foto\n`);
    expect(result.conf).toMatch(
      /\[Foto\][^[]*read only = yes[^[]*guest ok = no\n {3}valid users = valerio/,
    );
    expect(result.conf).toMatch(/\[Musica di casa\][^[]*read only = no[^[]*guest ok = yes/);
    expect(result.conf).toContain('force user = valerio\n   force group = nas-storage');
    expect(result.conf).toContain('disable netbios = yes');
    expect(result.conf).toContain('follow symlinks = no');

    expect(result.original).toContain('workgroup = DEBIAN');
    expect(result.calls).toContain('systemctl disable --now nmbd.service');
    expect(result.calls).toContain('systemctl enable smbd.service');
    expect(result.calls).toContain('systemctl reload-or-restart smbd.service');
    expect(result.calls).toContain('nft add element inet nas_base nas_shares { 445 }');
  });

  it('sends the files deleted from a share to the trash of its disk', () => {
    const { storage, apply } = nas();
    const result = apply({
      'nas-file-manager': `Foto\t${storage}/disk/Foto\tro\topen\t${storage}/disk/.nas-trash/Foto\nMusica\t${storage}/disk/Musica\trw\topen\t-\n`,
    });
    expect(result.code).toBe(0);
    expect(result.conf).toMatch(
      new RegExp(
        `\\[Foto\\][^[]*vfs objects = recycle\\n {3}recycle:repository = ${storage}/disk/\\.nas-trash/Foto\\n`,
      ),
    );
    expect(result.conf).toContain('recycle:keeptree = yes\n   recycle:versions = no');
    expect(result.conf).toMatch(/\[Musica\][^[]*directory mask = 2770\n?$/);
  });

  it('leaves out the shares of a disk that is not mounted', () => {
    const { storage, apply } = nas({
      mounted: `[ "$3" = ${JSON.stringify(join(temp.path, 'storage', 'disk'))} ] && exit 1; exit 0`,
    });
    mkdirSync(join(storage, 'other'), { recursive: true });
    const result = apply({
      'nas-file-manager': `Disco\t${storage}/disk\trw\topen\nAltro\t${storage}/other\trw\topen\n`,
    });

    expect(result.code).toBe(0);
    expect(result.stdout).toContain(`the disk of ${storage}/disk is not mounted`);
    expect(result.stdout).toContain('1 folders shared');
    expect(result.conf).not.toContain('[Disco]');
    expect(result.conf).toContain(`[Altro]\n   path = ${storage}/other\n`);
  });

  it('never shares again a network disk', () => {
    const { storage, apply } = nas({
      fstype: `[ "$*" = ${JSON.stringify(`--noheadings --output FSTYPE --mountpoint ${join(temp.path, 'storage', 'disk')}`)} ] && echo cifs || echo ext4`,
    });
    mkdirSync(join(storage, 'other'), { recursive: true });
    const result = apply({
      'nas-file-manager': `Rete\t${storage}/disk/Foto\tro\topen\nAltro\t${storage}/other\trw\topen\n`,
    });

    expect(result.code).toBe(0);
    expect(result.stdout).toContain('is on a network disk: Rete is not shared');
    expect(result.conf).not.toContain('[Rete]');
    expect(result.conf).toContain('[Altro]');
  });

  it('stops Samba and closes 445 when nothing is shared any more', () => {
    const { storage, apply } = nas();
    expect(apply({ 'nas-file-manager': `Foto\t${storage}/disk/Foto\tro\topen\n` }).code).toBe(0);
    const result = apply({ 'nas-file-manager': '' });

    expect(result.code).toBe(0);
    expect(result.stdout).toContain('Samba stopped');
    expect(result.calls).toContain('nft flush set inet nas_base nas_shares');
    expect(result.calls).toContain('systemctl disable --now smbd.service nmbd.service');
  });

  it('leaves alone a Samba that was there before while nothing is shared', () => {
    const result = nas({ conf: '[global]\n' }).apply();

    expect(result.code).toBe(0);
    expect(result.calls).not.toContain('systemctl');
    expect(result.calls).toContain('nft flush set inet nas_base nas_shares');
  });

  it('refuses a line it does not trust, and keeps the configuration', () => {
    const { storage, apply } = nas({ conf: '[global]\n' });
    for (const line of [
      `global\t${storage}/disk/Foto\tro\topen`,
      `Fo]to\t${storage}/disk/Foto\tro\topen`,
      `Foto\t/etc\tro\topen`,
      `Foto\t${storage}/disk/../disk/Foto\tro\topen`,
      `Foto\t${storage}/disk/Foto%u\tro\topen`,
      `Foto\t${storage}/disk/Foto\tall\topen`,
      `Foto\t${storage}/disk/Foto\tro\tmaybe`,
      `Foto\t${storage}/disk/Foto\tro\topen\t-\textra`,
      `Foto\t${storage}/disk/Foto\tro\topen\t${storage}/other/.nas-trash/Foto`,
      `Foto\t${storage}/disk/Foto\tro\topen\t/tmp/.nas-trash/Foto`,
    ]) {
      const result = apply({ 'nas-file-manager': `${line}\n` });
      expect(result.code, line).not.toBe(0);
      expect(result.conf, line).toBe('[global]\n');
    }
    const twice = apply({
      'nas-file-manager': `Foto\t${storage}/disk/Foto\tro\topen\nfoto\t${storage}/disk/Musica\tro\topen\n`,
    });
    expect(twice.output).toContain('shared twice');
  });

  it('leaves out a folder that is gone, and stops when Samba is missing or refuses', () => {
    const { storage, apply } = nas();
    const gone = apply({
      'nas-file-manager': `Foto\t${storage}/disk/Foto\tro\topen\nVia\t${storage}/disk/Via\tro\topen\n`,
    });
    expect(gone.code).toBe(0);
    expect(gone.stdout).toContain('Via is not shared for now');
    expect(gone.conf).not.toContain('[Via]');

    rmSync(join(temp.path, 'calls'));
    const refused = nas({ testparm: 'exit 1' });
    const result = refused.apply({
      'nas-file-manager': `Foto\t${refused.storage}/disk/Foto\tro\topen\n`,
    });
    expect(result.code).not.toBe(0);
    expect(result.output).toContain('samba refused the configuration');
    expect(result.calls).not.toContain('nas_shares { 445 }');
  });
});
