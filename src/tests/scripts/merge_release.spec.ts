import { describe, expect, it } from 'vitest';
import { nextVersion } from '@scripts/release/merge_release.mjs';

describe('merge_release.mjs', () => {
  it('releases the package version as it stands when there is no tag yet', () => {
    expect(nextVersion('0.1.0', null, 'patch')).toBe('0.1.0');
  });

  it('bumps the last tag by the release type', () => {
    expect(nextVersion('0.1.3', 'v0.1.3', 'patch')).toBe('0.1.4');
    expect(nextVersion('0.1.3', 'v0.1.3', 'minor')).toBe('0.2.0');
    expect(nextVersion('0.1.3', 'v0.1.3', 'major')).toBe('1.0.0');
  });

  it('keeps a package version already ahead of the last tag', () => {
    expect(nextVersion('0.3.0', 'v0.2.7', 'patch')).toBe('0.3.0');
  });

  it('refuses a release type it does not know, tag or no tag', () => {
    expect(() => nextVersion('0.1.0', null, 'huge')).toThrow('Unknown release type');
    expect(() => nextVersion('0.1.0', 'v0.1.0', 'huge')).toThrow('Unknown release type');
  });
});
