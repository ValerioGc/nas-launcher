import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { runInNewContext } from 'node:vm';
import ts from 'typescript';
import { describe, expect, it } from 'vitest';
import { CONFIG, run, SYSTEM } from './helpers';

const read = (name: string) => readFileSync(join(SYSTEM, name), 'utf8');

const code = (name: string) =>
  read(name)
    .replace(/\/\*[\s\S]*?\*\//g, '')
    .split('\n')
    .filter((line) => !line.trimStart().startsWith('#') && !line.trimStart().startsWith('//'))
    .join('\n');

describe('what the installation grants', () => {
  const power = code('files/10-nas-os-power.rules');
  const network = code('files/20-nas-os-network.rules');
  const firewall = code('files/30-nas-os-firewall.rules');
  const update = code('files/40-nas-os-update.rules');
  const apps = code('files/50-nas-os-apps.rules');
  const apt = code('files/60-nas-os-apt.rules');
  const reset = code('files/75-nas-os-reset.rules');
  const grants = code('files/55-nas-os-app-grants.rules');
  const appPackages = code('files/56-nas-os-app-packages.rules');
  const antivirus = code('files/57-nas-os-antivirus.rules');
  const cleanup = code('files/59-nas-os-cleanup.rules');
  const syncPorts = code('files/62-nas-os-sync-ports.rules');
  const discovery = code('files/63-nas-os-discovery.rules');
  const processes = code('files/65-nas-os-processes.rules');
  const rules = [
    power,
    network,
    firewall,
    update,
    apps,
    apt,
    reset,
    grants,
    appPackages,
    antivirus,
    cleanup,
    syncPorts,
    discovery,
    processes,
  ];

  it.each([
    ['power', power],
    ['network', network],
    ['access list', firewall],
    ['update', update],
    ['applications', apps],
    ['system packages', apt],
    ['reset', reset],
    ['application grants', grants],
    ['application packages', appPackages],
    ['antivirus', antivirus],
    ['cleanup', cleanup],
    ['sync ports', syncPorts],
    ['processes', processes],
  ])('the %s rule answers only for the service user', (_name, rule) => {
    expect(rule).toContain("subject.user === 'nas-os'");
    expect(rule.match(/polkit\.Result\.YES/g)).toHaveLength(
      rule.match(/polkit\.addRule/g)?.length ?? 0,
    );
  });

  it('keeps lifecycle scans behind the fixed root helper and before filesystem shutdown', () => {
    const lifecycle = read('files/nas-os-antivirus-lifecycle.service');
    const operation = read('files/nas-os-antivirus.service');
    expect(lifecycle).toContain('Before=nas-os.service');
    expect(lifecycle).toContain('After=local-fs.target network-online.target');
    expect(lifecycle).toContain('RemainAfterExit=yes');
    expect(lifecycle).toContain('ExecStop=/usr/bin/flock /run/nas-os-antivirus.lock');
    expect(operation).toContain('ExecStart=/usr/bin/flock /run/nas-os-antivirus.lock');
    expect(antivirus).not.toContain('nas-os-antivirus-lifecycle.service');
  });

  it('lets the launcher only start the cleanup job, which needs no network', () => {
    expect(cleanup).toContain("action.lookup('unit') === 'nas-os-cleanup.service'");
    expect(cleanup).toContain("action.lookup('verb') === 'start'");
    const job = code('files/nas-os-cleanup.service');
    expect(job).toContain('nas-os-axum --cleanup-job');
    expect(job).toContain('RestrictAddressFamilies=AF_UNIX');
    expect(job).not.toContain('[Install]');
  });

  it('lets the launcher only start the unit that opens the fixed ports of the sync', () => {
    expect(syncPorts).toContain("action.lookup('unit') === 'nas-os-sync-ports.service'");
    const unit = code('files/nas-os-sync-ports.service');
    expect(unit).toContain('ExecStart=/usr/local/lib/nas-os/apply-sync-ports.sh');
    expect(unit).toContain('CapabilityBoundingSet=CAP_NET_ADMIN');
    expect(unit).not.toContain('[Install]');
  });

  it('lets the launcher only start the unit that opens the port of the discovery', () => {
    expect(discovery).toContain("action.lookup('unit') === 'nas-os-discovery.service'");
    expect(discovery).toContain("action.lookup('verb') === 'start'");
    const unit = code('files/nas-os-discovery.service');
    expect(unit).toContain('ExecStart=/usr/local/lib/nas-os/apply-discovery.sh');
    expect(unit).toContain('CapabilityBoundingSet=CAP_NET_ADMIN');
    expect(unit).toContain('ReadOnlyPaths=/var/lib/nas-os');
    expect(unit).not.toContain('[Install]');
  });

  it('never grants anything but a YES', () => {
    for (const rule of rules) {
      expect(rule).not.toContain('AUTH_ADMIN');
      expect(rule).not.toContain('polkit.Result.AUTH');
    }
  });

  it('lets the launcher only start the unit that writes the grants of an application', () => {
    expect(grants).toContain('/^nas-os-app-grants@[A-Za-z0-9_-]{1,64}\\.service$/');
    expect(grants).toContain("action.lookup('verb') === 'start'");
    expect(grants).not.toMatch(/'(stop|enable|restart)'/);
  });

  it('lets the launcher only start the unit that acts on one process', () => {
    expect(processes).toContain(
      '/^nas-os-process@(terminate|restart)-[1-9][0-9]{0,9}-[0-9]{1,20}\\.service$/',
    );
    expect(processes).toContain("action.lookup('verb') === 'start'");
    expect(processes).not.toMatch(/'(stop|enable|restart|kill)'/);
  });

  it('keeps polkit calls compatible with ECMAScript 5', () => {
    for (const rule of rules) {
      const source = ts.createSourceFile(
        'rule.js',
        rule,
        ts.ScriptTarget.ES5,
        true,
        ts.ScriptKind.JS,
      );
      function check(node: ts.Node) {
        if (ts.isCallExpression(node) || ts.isNewExpression(node)) {
          expect(node.arguments?.hasTrailingComma ?? false).toBe(false);
        }
        ts.forEachChild(node, check);
      }
      check(source);
    }
  });

  it('authorizes process requests only for the launcher and the constrained start unit', () => {
    type Rule = (
      action: { id: string; lookup: (key: string) => string | undefined },
      subject: { user: string },
    ) => string | undefined;
    const registered: Rule[] = [];
    runInNewContext(processes, {
      polkit: { addRule: (rule: Rule) => registered.push(rule), Result: { YES: 'yes' } },
    });
    expect(registered).toHaveLength(1);
    const rule = registered[0]!;
    const authorize = (
      unit: string,
      verb = 'start',
      user = 'nas-os',
      id = 'org.freedesktop.systemd1.manage-units',
    ) => rule({ id, lookup: (key) => ({ unit, verb })[key as 'unit' | 'verb'] }, { user });
    expect(authorize('nas-os-process@terminate-4782-655902.service')).toBe('yes');
    expect(authorize('nas-os-process@restart-4782-655902.service')).toBe('yes');
    for (const unit of [
      'nas-os.service',
      'nas-os-process@terminate-0-655902.service',
      'nas-os-process@kill-4782-655902.service',
      'nas-os-process@terminate-4782.service',
    ]) {
      expect(authorize(unit)).toBeUndefined();
    }
    expect(authorize('nas-os-process@terminate-4782-655902.service', 'stop')).toBeUndefined();
    expect(
      authorize('nas-os-process@terminate-4782-655902.service', 'start', 'nas'),
    ).toBeUndefined();
    expect(
      authorize(
        'nas-os-process@terminate-4782-655902.service',
        'start',
        'nas-os',
        'org.freedesktop.systemd1.manage-unit-files',
      ),
    ).toBeUndefined();
  });

  it('does not let the launcher step over an inhibitor', () => {
    expect(power).toContain('org.freedesktop.login1.power-off');
    expect(power).not.toContain('ignore-inhibit');
  });

  it('lets the gateway start only the existing firewall helper for its HTTPS port', () => {
    type Rule = (
      action: { id: string; lookup: (key: string) => string | undefined },
      subject: { user: string },
    ) => string | undefined;
    const registered: Rule[] = [];
    runInNewContext(firewall, {
      polkit: { addRule: (rule: Rule) => registered.push(rule), Result: { YES: 'yes' } },
    });
    const authorize = (unit: string, verb: string, user: string) =>
      registered[0]!(
        {
          id: 'org.freedesktop.systemd1.manage-units',
          lookup: (key) => ({ unit, verb })[key as 'unit' | 'verb'],
        },
        { user },
      );
    expect(authorize('nas-os-firewall.service', 'start', 'nas-os-gateway')).toBe('yes');
    expect(authorize('nas-os-firewall.service', 'stop', 'nas-os-gateway')).toBeUndefined();
    expect(authorize('nas-os.service', 'start', 'nas-os-gateway')).toBeUndefined();
    expect(authorize('nas-os-firewall.service', 'start', 'nas')).toBeUndefined();
  });

  it.each([
    ['nas-os-wake.service', power],
    ['nas-os-firewall.service', firewall],
    ['nas-os-update.service', update],
  ])('lets the launcher start %s and nothing else about it', (unit, rule) => {
    expect(rule).toContain(`action.lookup('unit') === '${unit}'`);
    expect(rule).toContain("action.lookup('verb') === 'start'");
    expect(rule).not.toMatch(/'(stop|enable|restart)'/);
  });

  it('lets the launcher run the application units, and only those', () => {
    expect(apps).toContain('nas-app@');
    expect(apps).toContain('[A-Za-z0-9_-]{1,64}');
    expect(apps).toContain("['start', 'stop', 'restart', 'clean']");
    expect(apps).not.toContain('enable');
    expect(apps).not.toContain('mask');
  });

  it('lets the launcher refresh the index and install upgrades, and nothing else of apt', () => {
    expect(apt).toContain("'nas-os-apt@refresh.service'");
    expect(apt).toContain("'nas-os-apt@upgrade.service'");
    expect(apt).toContain("action.lookup('verb') === 'start'");
    expect(apt).not.toMatch(/'(stop|enable|restart)'/);
    expect(apt).not.toContain('RegExp');
  });

  it('does not let the launcher switch the network off', () => {
    expect(network).toContain('NetworkManager.wifi.scan');
    expect(network).toContain('NetworkManager.network-control');
    expect(network).not.toContain('enable-disable-network');
    expect(network).not.toContain('enable-disable-wifi');
  });
});

describe('the systemd units', () => {
  const launcher = code('files/nas-os.service');
  const wake = code('files/nas-os-wake.service');
  const firewall = code('files/nas-os-firewall.service');
  const update = code('files/nas-os-update.service');
  const template = code('files/nas-app@.service');
  const apt = code('files/nas-os-apt@.service');
  const production = readFileSync(join(CONFIG, 'production.env'), 'utf8');
  const development = readFileSync(join(CONFIG, 'development.env'), 'utf8');

  it('registers application cgroups with the base firewall', () => {
    expect(template).toContain('Requires=nas-os-base-firewall.service');
    expect(template).toContain('NFTSet=cgroup:inet:nas_base:nas_apps_out');
  });

  it('lets the signatures of the antivirus through the base firewall', () => {
    expect(code('files/nas-os-antivirus.service')).toContain(
      'NFTSet=cgroup:inet:nas_base:nas_apps_out',
    );
    expect(code('files/clamav-freshclam-firewall.conf')).toMatch(
      /^NFTSet=cgroup:inet:nas_base:nas_apps_out$/m,
    );
    expect(read('install.sh')).toContain(
      '/etc/systemd/system/clamav-freshclam.service.d/nas-os-firewall.conf',
    );
    expect(read('scripts/uninstall.sh')).toContain(
      'rm -f /etc/systemd/system/clamav-freshclam.service.d/nas-os-firewall.conf',
    );
  });

  it('hides from every application what the launcher does not grant it', () => {
    expect(template).toMatch(
      /^TemporaryFileSystem=\/srv:ro \/mnt:ro \/media:ro \/var\/log:ro \/var\/lib\/nas-os:ro$/m,
    );
    expect(template).toMatch(/^BindReadOnlyPaths=\/var\/lib\/nas-os\/apps\/%i$/m);
    expect(template).toMatch(/^InaccessiblePaths=-\/sys -\/run\/udev$/m);
    expect(template).toMatch(/^ProtectProc=invisible$/m);
    expect(template).toMatch(/^ProcSubset=pid$/m);
    expect(template).not.toContain('/srv/storage');
  });

  it('installs the packages of the applications as root, only through their own helper', () => {
    const unit = code('files/nas-os-app-packages@.service');
    const appPackages = code('files/56-nas-os-app-packages.rules');
    expect(unit).toContain('ExecStart=/usr/local/lib/nas-os/app-packages.sh %i');
    expect(unit).toContain('Type=oneshot');
    expect(unit).toMatch(/^StateDirectory=nas-os-app-packages$/m);
    expect(appPackages).toContain('nas-os-app-packages@[A-Za-z0-9_-]{1,64}');
    expect(appPackages).toContain("action.lookup('verb') === 'start'");
  });

  it('writes the grants as root, only into the drop-ins of the applications', () => {
    const unit = code('files/nas-os-app-grants@.service');
    expect(unit).toContain('ExecStart=/usr/local/lib/nas-os/apply-app-grants.sh %i');
    expect(unit).toContain('Type=oneshot');
    expect(unit).toMatch(/^ReadWritePaths=\/etc\/systemd\/system$/m);
    expect(unit).toContain('ProtectSystem=strict');
    // The requests are the launcher's, 0600 in its 0710 state directory: root needs this to read
    // them, and without it the helper sees no request and grants nothing.
    expect(unit).toMatch(/^CapabilityBoundingSet=CAP_DAC_READ_SEARCH$/m);
  });

  it('acts on a process as root with no capability but signalling', () => {
    const unit = code('files/nas-os-process@.service');
    expect(unit).toContain('ExecStart=/usr/local/lib/nas-os/act-on-process.sh %i');
    expect(unit).toContain('Type=oneshot');
    expect(unit).toMatch(/^CapabilityBoundingSet=CAP_KILL$/m);
    expect(unit).toMatch(/^PrivateNetwork=yes$/m);
    expect(unit).toContain('ProtectSystem=strict');
  });

  it('keeps applications off the loopback, where the launcher trusts every caller', () => {
    expect(template).toContain('IPAddressDeny=localhost');
    expect(template).toMatch(/^IPAddressAllow=127\.0\.0\.53$/m);
  });

  it('gives applications the socket of the launcher instead, in a directory they cannot write', () => {
    const socket = 'NAS_LAUNCHER_SOCKET=/run/nas-os/launcher/apps.sock';
    expect(template).toContain(`Environment=${socket}`);
    expect(launcher).toContain(`Environment=${socket}`);
    expect(launcher).toContain('RuntimeDirectory=nas-os/apps nas-os/launcher');
    expect(template).toMatch(/^ReadWritePaths=\/run\/nas-os\/apps$/m);
  });

  it('runs the launcher as its own user, never as root', () => {
    expect(launcher).toContain('User=nas-os');
    expect(launcher).toContain('NoNewPrivileges=yes');
    expect(launcher).toContain('ProtectSystem=strict');
    expect(launcher).toContain('SystemCallFilter=@system-service');
  });

  it('imports the credentials of the launcher from the systemd store', () => {
    expect(launcher).toContain('ImportCredential=nas-os.*');
    expect(production).toContain('NAS_LAUNCHER_REPOSITORY=ValerioGc/nas-launcher');
  });

  it('reads the settings of the launcher from the release and the machine', () => {
    expect(launcher).toContain('EnvironmentFile=/etc/nas-os/nas-os.env');
    expect(launcher).toContain('EnvironmentFile=-/etc/nas-os/nas-os.local.env');
    expect(launcher).not.toMatch(/^Environment=NAS_[A-Z]+_MODE=/m);
  });

  it('gives the launcher a state directory its group can only cross', () => {
    expect(launcher).toContain('StateDirectory=nas-os');
    expect(launcher).toContain('StateDirectoryMode=0710');
    expect(launcher).toContain('UMask=0077');
  });

  it.each([
    'NAS_POWER_MODE=real',
    'NAS_NETWORK_MODE=real',
    'NAS_FIREWALL_MODE=real',
    'NAS_PACKAGES_MODE=real',
    'NAS_APPS_MODE=real',
    'NAS_APT_MODE=real',
    'NAS_TERMINAL_MODE=real',
    'NAS_TOKEN_MODE=real',
    'NAS_PROCESSES_MODE=real',
  ])('turns on %s on the NAS, and leaves it off in development', (mode) => {
    const [name] = mode.split('=');
    expect(production).toMatch(new RegExp(`^${mode}$`, 'm'));
    expect(development).toContain(`${name}=\${${name}:-simulated}`);
  });

  it.each([
    ['the wake alarm', wake],
    ['the access list', firewall],
    ['the update', update],
    ['the package job', apt],
  ])('%s is a oneshot that nobody enables at boot', (_name, unit) => {
    expect(unit).toContain('Type=oneshot');
    expect(unit).not.toContain('[Install]');
    expect(unit).not.toContain('WantedBy=');
  });

  it('leaves the wake alarm able to write the one file it exists for', () => {
    expect(wake).not.toContain('ProtectKernelTunables');
    expect(wake).toContain('ExecStart=/usr/local/lib/nas-os/wake-alarm.sh');
  });

  it('leaves the access list able to reach netfilter', () => {
    expect(firewall).not.toContain('PrivateNetwork');
    expect(firewall).toContain('ExecStart=/usr/local/lib/nas-os/apply-access-rules.sh');
  });

  it('leaves the update able to write the binary it exists to replace', () => {
    expect(update).not.toContain('ProtectSystem=strict');
    expect(update).toContain('ExecStart=/usr/local/lib/nas-os/install-launcher-update.sh');
    expect(update).not.toContain('PrivateNetwork');
    expect(update).toContain('TimeoutStartSec=');
  });

  it('gives the applications a socket directory that outlives a restart', () => {
    expect(launcher).toContain('RuntimeDirectory=nas-os/apps');
    expect(launcher).toContain('RuntimeDirectoryPreserve=yes');
    expect(launcher).toContain('RuntimeDirectoryMode=3770');
    expect(launcher).toContain('Environment=NAS_APP_SOCKETS=/run/nas-os/apps');
  });

  it('runs an application as a user of its own, able to write almost nothing', () => {
    expect(template).toContain('ExecStart=/usr/local/lib/nas-os/run-app.sh %i');
    expect(template).toContain('DynamicUser=yes');
    expect(template).toContain('SupplementaryGroups=nas-os');
    expect(template).toContain('UMask=0007');
    expect(template).toContain('ProtectSystem=strict');
    expect(template).toContain('ReadWritePaths=/run/nas-os/apps');
    expect(template).toContain('SystemCallFilter=@system-service');
    expect(template).toContain('CapabilityBoundingSet=\n');
    expect(template).toContain('MemoryDenyWriteExecute=yes');
    expect(template).toContain('Environment=TOKIO_WORKER_THREADS=2 MALLOC_ARENA_MAX=2');
    expect(template).not.toContain('[Install]');
    expect(template).not.toContain('WantedBy=');
  });

  it('lets only the launcher reach the encryption of the disks, keys kept off the disk', () => {
    const socket = code('files/nas-os-storage-crypt.socket');
    const helper = code('files/nas-os-storage-crypt@.service');

    expect(socket).toContain('SocketGroup=nas-os-crypt');
    expect(socket).toContain('SocketMode=0660');
    expect(socket).toContain('Accept=yes');
    expect(launcher).toMatch(/^SupplementaryGroups=systemd-journal nas-os-crypt$/m);
    expect(template).not.toContain('nas-os-crypt');
    expect(helper).toContain('StandardInput=socket');
    expect(helper).toContain('RuntimeDirectoryMode=0700');
    expect(code('files/nas-os-storage.service')).toContain('RuntimeDirectoryMode=0700');
    expect(production).toMatch(/^NAS_ENCRYPTION_MODE=off$/m);
    const install = read('install.sh');
    expect(install.indexOf('groupadd --system nas-os-crypt')).toBeLessThan(
      install.indexOf('files/nas-os.service'),
    );
  });

  it('gives the launcher no capability and no device', () => {
    expect(launcher).toContain('Environment=TOKIO_WORKER_THREADS=4 MALLOC_ARENA_MAX=2');
    expect(launcher).toContain('CapabilityBoundingSet=\n');
    expect(launcher).toContain('PrivateDevices=yes');
    expect(launcher).toContain('RestrictAddressFamilies=AF_UNIX AF_INET AF_INET6 AF_NETLINK');
  });

  it('leaves the launcher release address configurable', () => {
    const production = readFileSync(join(CONFIG, 'production.env'), 'utf8');
    expect(production).toContain('NAS_LAUNCHER_REPOSITORY=');
    expect(production).toContain('NAS_LAUNCHER_PROVIDER=');
  });
});

describe('check.sh', () => {
  it('reports every part of the installation, and fails when something is missing', () => {
    const result = run(join(SYSTEM, 'scripts/check.sh'));

    for (const section of [
      'Installation',
      'Service',
      'Reboot and shutdown permissions',
      'Network',
      'Allowed addresses',
      'Application packages',
      'Opening the applications',
      'Scheduled power-on',
      'Answer from the launcher',
      'Interface',
    ]) {
      expect(result.stdout).toContain(section);
    }
    expect(result.stdout).toContain('[KO]');
    expect(result.code).not.toBe(0);
    expect(result.stdout).toMatch(/\d+ things to fix/);
  });

  it('reboots nothing while checking whether it could', () => {
    const source = code('scripts/check.sh');

    expect(source).toContain('pkcheck');
    expect(source).not.toMatch(/systemctl (reboot|poweroff)/);
    expect(source).not.toMatch(/\bnft (add|delete|flush)/);
  });
});

describe('the terminal', () => {
  const socket = code('files/nas-os-terminal.socket');
  const shell = code('files/nas-os-terminal@.service');

  it('lets only the group of the launcher reach the shells', () => {
    expect(socket).toContain('Accept=yes');
    expect(socket).toContain('SocketGroup=nas-os');
    expect(socket).toContain('SocketMode=0660');
  });

  it('starts the app PTY as the configured user and permits authenticated su and sudo', () => {
    expect(shell).toContain('User=nobody');
    expect(shell).toContain('NoNewPrivileges=no');
    expect(shell).toContain(
      'ExecStart=/var/lib/nas-os/apps/nas-system-terminal/bin/nas-system-terminal-axum --shell',
    );
    expect(shell).toContain('KillMode=control-group');
    expect(shell).not.toMatch(/^User=root$/m);
  });
});
