import { afterEach, describe, expect, it, vi } from 'vitest';
import { randomUuid } from '@/utils/randomUuid';

afterEach(() => vi.unstubAllGlobals());

describe('random UUIDs', () => {
  it('generates distinct UUID v4 tokens without crypto.randomUUID', () => {
    vi.stubGlobal('crypto', { getRandomValues: crypto.getRandomValues.bind(crypto) });
    const ids = Array.from({ length: 100 }, () => randomUuid());
    expect(new Set(ids).size).toBe(ids.length);
    for (const id of ids) {
      expect(id).toMatch(/^[\da-f]{8}-[\da-f]{4}-4[\da-f]{3}-[89ab][\da-f]{3}-[\da-f]{12}$/);
    }
  });
});
