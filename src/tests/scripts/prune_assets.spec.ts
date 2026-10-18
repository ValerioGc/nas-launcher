import { mkdirSync, mkdtempSync, readdirSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { afterEach, describe, expect, it } from 'vitest';
import { compareVersions, pruneAssets } from '@scripts/release/prune_assets.mjs';

const folders: string[] = [];

function folder(names: string[]) {
  const directory = mkdtempSync(join(tmpdir(), 'prune-assets-'));
  for (const name of names) {
    writeFileSync(join(directory, name), '');
  }
  mkdirSync(join(directory, 'package'));
  folders.push(directory);
  return directory;
}

afterEach(() => {
  for (const directory of folders.splice(0)) {
    rmSync(directory, { recursive: true, force: true });
  }
});

describe('prune_assets.mjs', () => {
  it('orders versions as semver does', () => {
    expect(compareVersions('0.1.10', '0.1.9')).toBeGreaterThan(0);
    expect(compareVersions('1.0.0-rc.1', '1.0.0')).toBeLessThan(0);
    expect(compareVersions('0.1.3', '0.1.3')).toBe(0);
  });

  it('keeps the release just packaged and the one before it, and nothing else versioned', () => {
    const directory = folder([
      'nas-os-0.1.8-linux-x64.tar.gz',
      'nas-os-0.1.9-linux-x64.tar.gz',
      'nas-os-0.1.9-linux-x64.tar.gz.sha256',
      'nas-os-0.1.10-linux-x64.tar.gz',
      'nas-os-0.1.10-linux-x64.tar.gz.sha256',
      'nas_os_installation_manual_0.1.9.pdf',
      'notes.txt',
    ]);
    const removed = pruneAssets(directory, '0.1.10', ['nas-os-']);
    expect(readdirSync(directory).sort()).toEqual([
      'nas-os-0.1.10-linux-x64.tar.gz',
      'nas-os-0.1.10-linux-x64.tar.gz.sha256',
      'nas-os-0.1.9-linux-x64.tar.gz',
      'nas-os-0.1.9-linux-x64.tar.gz.sha256',
      'notes.txt',
      'package',
    ]);
    expect(removed.sort()).toEqual(
      ['nas_os_installation_manual_0.1.9.pdf', 'nas-os-0.1.8-linux-x64.tar.gz'].sort(),
    );
  });
});
