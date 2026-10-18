import { spawnSync } from 'node:child_process';
import { readdirSync, readFileSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { expect, it } from 'vitest';

const ROOT = resolve(__dirname, '../../..');

it('writes no em dash in any file of the repository', () => {
  const search = spawnSync('git', ['grep', '-nP', String.raw`\x{2014}`], {
    cwd: ROOT,
    encoding: 'utf8',
  });

  expect(search.stdout).toBe('');
  expect(search.status).toBe(1);
});

function templates() {
  const source = join(ROOT, 'src');
  return readdirSync(source, { recursive: true, encoding: 'utf8' })
    .filter((file) => file.endsWith('.vue') && !file.startsWith('tests'))
    .map((file) => {
      const text = readFileSync(join(source, file), 'utf8');
      return { file, template: text.slice(text.indexOf('<template>')) };
    });
}

it('gives every form field an id or a name, which the browser wants for autofill', () => {
  const unnamed = templates().flatMap(({ file, template }) =>
    [...template.matchAll(/<(input|select|textarea)\b([^>]*)>/g)]
      .filter(([, , attributes]) => !/(^|\s)(:?id|:?name|v-bind)=/.test(attributes!))
      .map(([tag]) => `${file}: ${tag.replace(/\s+/g, ' ').slice(0, 80)}`),
  );

  const switches = templates().flatMap(({ file, template }) =>
    [...template.matchAll(/<CommonSwitch\b([^>]*)>/g)]
      .filter(([, attributes]) => !/(^|\s):?(name|testid)=/.test(attributes!))
      .map(([tag]) => `${file}: ${tag.replace(/\s+/g, ' ').slice(0, 80)}`),
  );

  expect(unnamed).toEqual([]);
  expect(switches).toEqual([]);
});

it('writes every pattern so that the browser, which compiles it with the v flag, accepts it', () => {
  const refused = templates().flatMap(({ file, template }) =>
    [...template.matchAll(/\spattern="([^"]+)"/g)]
      .map(([, pattern]) => pattern!)
      .filter((pattern) => {
        try {
          new RegExp(`^(?:${pattern})$`, 'v');
          return false;
        } catch {
          return true;
        }
      })
      .map((pattern) => `${file}: ${pattern}`),
  );

  expect(refused).toEqual([]);
});
