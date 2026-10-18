import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';
import { THEMES, themeOf } from '@/apps/nas-system-settings/themes';
import { t, setLocale } from '@/i18n';

const STYLES = 'src/styles/themes';

describe('the themes', () => {
  it('have a file, a place in the index and a name for every one offered', () => {
    const index = readFileSync(`${STYLES}/_index.scss`, 'utf8');
    for (const locale of ['it', 'en'] as const) {
      setLocale(locale);
      for (const [kind, ids] of Object.entries(THEMES)) {
        for (const id of ids) {
          expect(t(`theme_${id}`)).not.toBe(`theme_${id}`);
          if (id === 'default') {
            continue;
          }
          const file = readFileSync(`${STYLES}/${kind}/_${id}.scss`, 'utf8');
          expect(file).toContain(`:root[data-theme='${kind}'][data-palette='${id}']`);
          expect(file).not.toContain('--accent');
          expect(index).toContain(`@use '${kind}/${id}';`);
        }
      }
    }
    setLocale('it');
  });

  it('fall back to the default for an id of the other type or one no longer offered', () => {
    expect(themeOf('light', 'sand')).toBe('sand');
    expect(themeOf('light', 'forest')).toBe('default');
    expect(themeOf('dark', undefined)).toBe('default');
  });
});
