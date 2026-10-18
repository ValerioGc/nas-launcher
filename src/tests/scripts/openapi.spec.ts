import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { expect, it } from 'vitest';
import { SCHEMA, TYPES, typesOf } from '@scripts/openapi.mjs';

const ROOT = resolve(__dirname, '../../..');

it('keeps the TypeScript types of the interface in step with the API document', async () => {
  const document = readFileSync(resolve(ROOT, SCHEMA), 'utf8');

  expect(readFileSync(resolve(ROOT, TYPES), 'utf8')).toBe(await typesOf(document));
}, 30_000);
