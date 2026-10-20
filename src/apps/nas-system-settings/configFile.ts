import type { ConfigContent } from './types/config';

const FORMAT = 'nas-os-config-encrypted';
const CLEAR_FORMAT = 'nas-os-config';
const VERSION = 1;
const ITERATIONS = 600_000;

interface EncryptedFile {
  format: typeof FORMAT;
  version: number;
  kdf: { name: 'PBKDF2'; hash: 'SHA-256'; iterations: number; salt: string };
  cipher: { name: 'AES-GCM'; iv: string };
  data: string;
}

export class NotAConfigFile extends Error {}

export class WrongFilePassword extends Error {}

function toBase64(bytes: Uint8Array) {
  let text = '';
  for (const byte of bytes) {
    text += String.fromCodePoint(byte);
  }
  return btoa(text);
}

function fromBase64(text: string): Uint8Array<ArrayBuffer> {
  return Uint8Array.from(atob(text), (character) => character.codePointAt(0) ?? 0);
}

function header(version: number) {
  return new TextEncoder().encode(`${FORMAT}:${version}`);
}

async function keyFor(password: string, salt: Uint8Array<ArrayBuffer>, iterations: number) {
  const material = await crypto.subtle.importKey(
    'raw',
    new TextEncoder().encode(password),
    'PBKDF2',
    false,
    ['deriveKey'],
  );
  return crypto.subtle.deriveKey(
    { name: 'PBKDF2', hash: 'SHA-256', salt, iterations },
    material,
    { name: 'AES-GCM', length: 256 },
    false,
    ['encrypt', 'decrypt'],
  );
}

/**
 * The text of the file for `content`, encrypted with `password`.
 */
async function encryptConfig(value: ConfigContent, password: string): Promise<string> {
  const salt = crypto.getRandomValues(new Uint8Array(16));
  const iv = crypto.getRandomValues(new Uint8Array(12));
  const key = await keyFor(password, salt, ITERATIONS);
  const data = await crypto.subtle.encrypt(
    { name: 'AES-GCM', iv, additionalData: header(VERSION) },
    key,
    new TextEncoder().encode(JSON.stringify(value)),
  );
  const file: EncryptedFile = {
    format: FORMAT,
    version: VERSION,
    kdf: { name: 'PBKDF2', hash: 'SHA-256', iterations: ITERATIONS, salt: toBase64(salt) },
    cipher: { name: 'AES-GCM', iv: toBase64(iv) },
    data: toBase64(new Uint8Array(data)),
  };
  return `${JSON.stringify(file, null, 2)}\n`;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function json(text: string): unknown {
  try {
    return JSON.parse(text);
  } catch {
    throw new NotAConfigFile('not JSON');
  }
}

function content(value: unknown): ConfigContent {
  if (!isRecord(value) || !isRecord(value.config) || value.config.format !== CLEAR_FORMAT) {
    throw new NotAConfigFile('no configuration inside');
  }
  return value as unknown as ConfigContent;
}

function encrypted(file: unknown): EncryptedFile {
  if (
    !isRecord(file) ||
    file.format !== FORMAT ||
    typeof file.version !== 'number' ||
    file.version > VERSION ||
    !isRecord(file.kdf) ||
    typeof file.kdf.salt !== 'string' ||
    typeof file.kdf.iterations !== 'number' ||
    file.kdf.iterations < 1 ||
    !isRecord(file.cipher) ||
    typeof file.cipher.iv !== 'string' ||
    typeof file.data !== 'string'
  ) {
    throw new NotAConfigFile('not an exported configuration');
  }
  return file as unknown as EncryptedFile;
}

async function decryptConfig(file: EncryptedFile, password: string): Promise<ConfigContent> {
  let clear: ArrayBuffer;
  try {
    const key = await keyFor(password, fromBase64(file.kdf.salt), file.kdf.iterations);
    clear = await crypto.subtle.decrypt(
      { name: 'AES-GCM', iv: fromBase64(file.cipher.iv), additionalData: header(file.version) },
      key,
      fromBase64(file.data),
    );
  } catch {
    throw new WrongFilePassword('the file does not open with this password');
  }
  return content(json(new TextDecoder().decode(clear)));
}

/**
 * The text of the file for `content` encrypted with `password`, or in clear without one.
 */
export function writeConfig(value: ConfigContent, password = ''): Promise<string> {
  return password
    ? encryptConfig(value, password)
    : Promise.resolve(`${JSON.stringify(value, null, 2)}\n`);
}

/**
 * Whether the text of a file is an encrypted configuration, which needs its password to open; a
 * text that is not a configuration at all is refused here, before any password is asked.
 */
export function isEncrypted(text: string): boolean {
  const file = json(text);
  if (isRecord(file) && file.format === FORMAT) {
    return true;
  }
  content(file);
  return false;
}

/**
 * The configuration in the text of a file, decrypted with `password` when it is encrypted. The
 * shape of the settings inside is the launcher's to check, when they are imported.
 */
export async function readConfig(text: string, password = ''): Promise<ConfigContent> {
  const file = json(text);
  return isRecord(file) && file.format === FORMAT
    ? decryptConfig(encrypted(file), password)
    : content(file);
}

export function configFileName(now = new Date()) {
  return `nas-os-config-${now.toISOString().slice(0, 10)}.json`;
}

export function saveConfigFile(text: string, name = configFileName()) {
  const url = URL.createObjectURL(new Blob([text], { type: 'application/json' }));
  const link = document.createElement('a');
  link.href = url;
  link.download = name;
  link.click();
  URL.revokeObjectURL(url);

  return name;
}
