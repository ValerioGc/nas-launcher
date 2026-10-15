import { execFileSync } from 'node:child_process';
import { mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';

/** Where the installation package lives, from the root of the repository. */
export const SYSTEM = resolve(__dirname, '../../../system/packages/ui_installation');
export const CONFIG = resolve(__dirname, '../../../config');

/** Where the configuration package lives. */
export const CONFIGURATION = resolve(__dirname, '../../../system/packages/ui_configuration');

/** Where the backend keeps the helpers that the units run as root. */
const BACKEND = resolve(__dirname, '../../../backend/axum/src');

export const HELPERS = {
  wake: join(BACKEND, 'apps/services/nas_system_settings/power/wake-alarm.sh'),
  access: join(BACKEND, 'apps/services/nas_system_settings/access/apply-access-rules.sh'),
  update: join(BACKEND, 'apps/services/nas_system_settings/updates/install-launcher-update.sh'),
  app: join(BACKEND, 'services/lifecycle/run-app.sh'),
  apt: join(BACKEND, 'apps/services/nas_system_settings/system_updates/apt-updates.sh'),
  token: join(BACKEND, 'services/packages/launcher-token.sh'),
  terminal: join(BACKEND, 'services/terminal/terminal-user.sh'),
  hostname: join(BACKEND, 'apps/services/nas_system_settings/apply-hostname.sh'),
  reset: join(BACKEND, 'apps/services/nas_system_settings/reset/factory-reset.sh'),
  grants: join(BACKEND, 'services/grants/apply-app-grants.sh'),
  packages: join(BACKEND, 'services/system_packages/app-packages.sh'),
  process: join(BACKEND, 'services/process_control/act-on-process.sh'),
  shares: join(BACKEND, 'services/shares/apply-smb-shares.sh'),
  syncPorts: join(BACKEND, 'services/sync_ports/apply-sync-ports.sh'),
  discovery: join(BACKEND, 'apps/services/nas_system_settings/discovery/apply-discovery.sh'),
};

export interface Run {
  code: number;
  stdout: string;
  stderr: string;
  output: string;
}

export function run(
  script: string,
  options: { args?: string[]; env?: NodeJS.ProcessEnv; input?: string } = {},
): Run {
  const spawn = {
    encoding: 'utf8' as const,
    env: { ...process.env, ...options.env },
    input: options.input ?? '',
  };
  try {
    const stdout = execFileSync('bash', [script, ...(options.args ?? [])], spawn);
    return { code: 0, stdout, stderr: '', output: stdout };
  } catch (failure) {
    const error = failure as { status?: number; stdout?: string; stderr?: string };
    const stdout = error.stdout ?? '';
    const stderr = error.stderr ?? '';
    return { code: error.status ?? 1, stdout, stderr, output: stdout + stderr };
  }
}

export function workspace() {
  const path = mkdtempSync(join(tmpdir(), 'nas-os-scripts-'));
  return { path, remove: () => rmSync(path, { recursive: true, force: true }) };
}

export function fakeCommand(directory: string, name: string, body: string) {
  const path = join(directory, name);
  execFileSync(
    'bash',
    ['-c', `cat > ${JSON.stringify(path)} && chmod +x ${JSON.stringify(path)}`],
    {
      input: `#!/usr/bin/env bash\n${body}\n`,
      encoding: 'utf8',
    },
  );
  return path;
}
