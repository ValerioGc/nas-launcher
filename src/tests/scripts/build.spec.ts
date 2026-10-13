import { readdirSync, readFileSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { describe, expect, it } from 'vitest';
import { RUNTIME_CLASS_PREFIXES } from '../../../vite.config.common';

const SOURCES = resolve(__dirname, '../..');

function components(directory: string): string[] {
  return readdirSync(directory, { withFileTypes: true }).flatMap((entry) => {
    const path = join(directory, entry.name);
    if (entry.isDirectory()) {
      return entry.name === 'tests' ? [] : components(path);
    }
    return entry.name.endsWith('.vue') ? [path] : [];
  });
}

describe('the CSS of the build', () => {
  it('keeps every class a component puts together at runtime', () => {
    const built = new Set<string>();
    for (const file of components(SOURCES)) {
      const source = readFileSync(file, 'utf8');
      for (const [, prefix] of source.matchAll(/:class="[^"]*`([a-z_]+_)\$\{/g)) {
        built.add(prefix!);
      }
    }

    expect(built.size).toBeGreaterThan(0);
    for (const prefix of built) {
      expect(RUNTIME_CLASS_PREFIXES, `${prefix} is kept by PurgeCSS`).toContain(prefix);
    }
  });
});
