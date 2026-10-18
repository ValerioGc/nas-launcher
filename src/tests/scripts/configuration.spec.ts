import { existsSync, readFileSync } from 'node:fs';
import { join } from 'node:path';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { CONFIGURATION, HELPERS, fakeCommand, run, workspace } from './helpers';

let temp: ReturnType<typeof workspace>;

beforeEach(() => {
  temp = workspace();
});

afterEach(() => temp.remove());

function launcherToken(args: string[], input: string) {
  const store = join(temp.path, 'credstore');
  const calls = join(temp.path, 'calls');
  fakeCommand(
    temp.path,
    'systemd-creds',
    `printf 'systemd-creds %s\\n' "$*" >> ${JSON.stringify(calls)}\nprintf 'encrypted:%s' "$(cat)" > "$4"`,
  );
  fakeCommand(temp.path, 'systemctl', `printf 'systemctl %s\\n' "$*" >> ${JSON.stringify(calls)}`);

  const { code, output } = run(join(CONFIGURATION, 'install.sh'), {
    args,
    input,
    env: {
      NAS_CREDSTORE: store,
      NAS_TOKEN_HELPER: HELPERS.token,
      PATH: `${temp.path}:${process.env.PATH}`,
    },
  });
  const credential = join(store, 'nas-os.launcher-token');
  return {
    code,
    output,
    credential: existsSync(credential) ? readFileSync(credential, 'utf8') : null,
    calls: existsSync(calls) ? readFileSync(calls, 'utf8') : '',
  };
}

describe('launcher-token.sh', () => {
  it('encrypts the token under the name the launcher imports, and restarts it', () => {
    const { code, credential, calls } = launcherToken([], 'ghp_secret\n');

    expect(code).toBe(0);
    expect(credential).toBe('encrypted:ghp_secret');
    expect(calls).toContain('systemd-creds encrypt --name=nas-os.launcher-token -');
    expect(calls).toContain('systemctl restart nas-os');
  });

  it('reads the token from its input, never from the command line', () => {
    const { calls } = launcherToken([], 'ghp_secret\n');

    expect(calls).not.toContain('ghp_secret');
  });

  it('refuses what cannot be a token, and writes nothing', () => {
    const { code, output, credential, calls } = launcherToken([], 'two words\n');

    expect(code).not.toBe(0);
    expect(output).toContain('does not look like a token');
    expect(credential).toBeNull();
    expect(calls).toBe('');
  });

  it('removes the token on request', () => {
    launcherToken([], 'ghp_secret\n');
    const { code, credential, output } = launcherToken(['--clear'], '');

    expect(code).toBe(0);
    expect(credential).toBeNull();
    expect(output).toContain('token removed');
  });
});

describe('install.sh', () => {
  it('asks for the launcher to be installed first, when its helper is not there', () => {
    const { code, output } = run(join(CONFIGURATION, 'install.sh'), {
      env: { NAS_TOKEN_HELPER: join(temp.path, 'missing.sh') },
    });

    expect(code).not.toBe(0);
    expect(output).toContain('install the launcher first (ui_installation)');
  });

  it('stores the token, creates the user of the terminal and reconciles the volumes, for the wizard', () => {
    const calls = join(temp.path, 'calls');
    const token = fakeCommand(
      temp.path,
      'token.sh',
      `printf 'token %s\\n' "$(cat)" >> ${JSON.stringify(calls)}`,
    );
    const terminal = fakeCommand(
      temp.path,
      'terminal.sh',
      `printf 'terminal %s\\n' "$(cat)" >> ${JSON.stringify(calls)}`,
    );
    fakeCommand(
      temp.path,
      'systemctl',
      `printf 'systemctl %s\\n' "$*" >> ${JSON.stringify(calls)}`,
    );

    const { code } = run(join(CONFIGURATION, 'install.sh'), {
      args: ['--user', 'owner'],
      input: 'ghp_secret',
      env: {
        NAS_TOKEN_HELPER: token,
        NAS_TERMINAL_HELPER: terminal,
        PATH: `${temp.path}:${process.env.PATH}`,
      },
    });

    expect(code).toBe(0);
    expect(readFileSync(calls, 'utf8')).toBe(
      'token ghp_secret\nterminal owner\nsystemctl start nas-os-storage.service\n',
    );
  });

  it('stores no token when the wizard was given none', () => {
    const calls = join(temp.path, 'calls');
    const token = fakeCommand(temp.path, 'token.sh', `echo token >> ${JSON.stringify(calls)}`);
    const terminal = fakeCommand(temp.path, 'terminal.sh', 'cat > /dev/null');
    fakeCommand(temp.path, 'systemctl', 'exit 0');

    const { code, output } = run(join(CONFIGURATION, 'install.sh'), {
      args: ['--user', 'owner'],
      env: {
        NAS_TOKEN_HELPER: token,
        NAS_TERMINAL_HELPER: terminal,
        PATH: `${temp.path}:${process.env.PATH}`,
      },
    });

    expect(code).toBe(0);
    expect(output).toContain('no token');
    expect(existsSync(calls)).toBe(false);
  });

  it('wants a name for the user of the terminal', () => {
    const token = fakeCommand(temp.path, 'token.sh', 'exit 0');
    const { code, output } = run(join(CONFIGURATION, 'install.sh'), {
      args: ['--user'],
      env: { NAS_TOKEN_HELPER: token },
    });

    expect(code).not.toBe(0);
    expect(output).toContain('usage: install.sh --user <name>');
  });
});
