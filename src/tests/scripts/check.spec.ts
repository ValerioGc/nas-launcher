import { execFileSync } from 'node:child_process';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { describe, expect, it } from 'vitest';
import { SYSTEM } from './helpers';

const CHECK = join(SYSTEM, 'scripts/check.sh');

function repositories(catalogue: string, launcher: string) {
  const script = readFileSync(CHECK, 'utf8');
  const start = script.indexOf('github_repositories() {');
  const body = script.slice(start, script.indexOf('\n}\n', start) + 3);
  return execFileSync('bash', ['-c', `${body}\ngithub_repositories | sort -u`], {
    encoding: 'utf8',
    env: { ...process.env, CATALOGUE: catalogue, launcher_repository: launcher },
  })
    .split('\n')
    .filter(Boolean);
}

describe('check.sh', () => {
  it('tries the token on the launcher and on every GitHub repository of the catalogue', () => {
    expect(repositories(join(SYSTEM, 'files/catalogue.json'), 'ValerioGc/nas-launcher')).toEqual([
      'ValerioGc/nas-backup-manager',
      'ValerioGc/nas-cleanup-manager',
      'ValerioGc/nas-disk-manager',
      'ValerioGc/nas-file-manager',
      'ValerioGc/nas-launcher',
      'ValerioGc/nas-log-manager',
      'ValerioGc/nas-mail-client',
      'ValerioGc/nas-picture-manager',
      'ValerioGc/nas-security-center',
      'ValerioGc/nas-sync-manager',
      'ValerioGc/nas-system-monitor',
      'ValerioGc/nas-system-terminal',
    ]);
  });

  it('hands the token to curl on its input, never on its command line', () => {
    const script = readFileSync(CHECK, 'utf8');

    expect(script).toContain('curl -K -');
    expect(script).not.toMatch(/curl[^\n]*Authorization/);
    expect(script).not.toMatch(/--header[^\n]*\$token/);
  });
});
