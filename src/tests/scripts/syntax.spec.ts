import { execFileSync } from 'node:child_process';
import { readdirSync, readFileSync } from 'node:fs';
import { basename, join } from 'node:path';
import { describe, expect, it } from 'vitest';
import { CONFIG, CONFIGURATION, HELPERS, SYSTEM } from './helpers';

const scripts = [
  ...['install.sh', 'scripts/uninstall.sh', 'scripts/check.sh'].map((name) => join(SYSTEM, name)),
  ...Object.values(HELPERS),
  join(CONFIGURATION, 'install.sh'),
];

describe('installation scripts', () => {
  it.each(scripts)('%s parses', (script) => {
    expect(() => execFileSync('bash', ['-n', script])).not.toThrow();
  });

  it.each(scripts)('%s stops on an unset variable', (script) => {
    expect(readFileSync(script, 'utf8')).toMatch(/^set -[eu]*u[eo]*/m);
  });

  it.each(scripts)('%s starts with a shebang and explains itself', (script) => {
    const lines = readFileSync(script, 'utf8').split('\n');

    expect(lines[0]).toBe('#!/usr/bin/env bash');
    expect(lines.slice(1, 12).filter((line) => line.startsWith('#')).length).toBeGreaterThan(3);
  });

  it('installs only files that are in the repository', () => {
    const install = readFileSync(join(SYSTEM, 'install.sh'), 'utf8');
    const present = new Set([...readdirSync(join(SYSTEM, 'files')), 'nas-os.env']);
    expect(readFileSync(join(CONFIG, 'production.env'), 'utf8')).toContain('NAS_APT_MODE=real');
    const referenced = [...install.matchAll(/\$here\/files\/([\w.@-]+)/g)].map(([, name]) => name);

    expect(referenced.length).toBeGreaterThan(0);
    for (const name of referenced) {
      expect(present, `install.sh copies files/${name}`).toContain(name);
    }
  });

  it('installs only helpers the backend provides', () => {
    const install = readFileSync(join(SYSTEM, 'install.sh'), 'utf8');
    const provided = Object.values(HELPERS).map((path) => basename(path));
    const referenced = [...install.matchAll(/\$here\/helpers\/([\w.-]+)/g)].map(([, name]) => name);

    expect(referenced.sort()).toEqual(provided.sort());
  });

  it('leaves no file behind that nothing installs', () => {
    const install = readFileSync(join(SYSTEM, 'install.sh'), 'utf8');

    for (const name of readdirSync(join(SYSTEM, 'files'))) {
      expect(install, `files/${name} is installed`).toContain(`files/${name}`);
    }
  });

  it('grants a rule for every unit the launcher has to start', () => {
    const rules = readdirSync(join(SYSTEM, 'files'))
      .filter((name) => name.endsWith('.rules'))
      .map((name) => readFileSync(join(SYSTEM, 'files', name), 'utf8'))
      .join('\n');

    const files = readdirSync(join(SYSTEM, 'files'));
    for (const unit of files.filter((name) => name.endsWith('.service'))) {
      const bySocket = files.includes(unit.replace(/@?\.service$/, '.socket'));
      // The lifecycle unit is armed by the root antivirus helper, never by the launcher.
      if (unit === 'nas-os.service' || unit === 'nas-os-antivirus-lifecycle.service' || bySocket) {
        continue;
      }

      const granted = unit.endsWith('@.service') ? unit.slice(0, -'.service'.length) : unit;
      expect(rules, `${unit} can be started`).toContain(granted);
    }
  });
});
