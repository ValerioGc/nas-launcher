import { describe, expect, it } from 'vitest';
import {
  configFileName,
  isEncrypted,
  NotAConfigFile,
  readConfig,
  WrongFilePassword,
  writeConfig,
} from '@/apps/nas-system-settings/configFile';
import type { ConfigContent } from '@/apps/nas-system-settings/types/config';

const content = {
  config: { format: 'nas-os-config', version: 1, accessRules: [{ value: '192.168.1.0/24' }] },
  signIn: { enabled: true, attempts: 5, lockMinutes: 1 },
} as unknown as ConfigContent;

describe('the configuration file', () => {
  it('is in clear without a password, and opens without one', async () => {
    const text = await writeConfig(content);

    expect(JSON.parse(text)).toEqual(content);
    expect(isEncrypted(text)).toBe(false);
    await expect(readConfig(text)).resolves.toEqual(content);
  });

  it('with a password opens only with it, and shows nothing in clear', async () => {
    const text = await writeConfig(content, 'long enough');

    expect(text).not.toContain('192.168.1.0');
    expect(isEncrypted(text)).toBe(true);
    expect(JSON.parse(text)).toMatchObject({ format: 'nas-os-config-encrypted', version: 1 });
    await expect(readConfig(text, 'long enough')).resolves.toEqual(content);
    for (const other of ['another one', '']) {
      await expect(readConfig(text, other)).rejects.toBeInstanceOf(WrongFilePassword);
    }
  });

  it('refuses a file that was changed or is not one', async () => {
    const file = JSON.parse(await writeConfig(content, 'long enough'));
    const changed = file.data.startsWith('A') ? 'B' : 'A';
    const tampered = JSON.stringify({ ...file, data: `${changed}${file.data.slice(1)}` });

    await expect(readConfig(tampered, 'long enough')).rejects.toBeInstanceOf(WrongFilePassword);
    for (const other of [
      'not json',
      '{}',
      JSON.stringify({ config: { format: 'something else' } }),
      JSON.stringify({ ...file, version: 2 }),
    ]) {
      await expect(readConfig(other, 'long enough')).rejects.toBeInstanceOf(NotAConfigFile);
    }
    expect(() => isEncrypted('{"hello":"world"}')).toThrow(NotAConfigFile);
  });

  it('is named after the day, never after the NAS', () => {
    expect(configFileName(new Date('2026-10-02T21:30:00Z'))).toBe('nas-os-config-2026-10-02.json');
  });
});
