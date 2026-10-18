import { beforeEach, expect, it } from 'vitest';
import { setLocale } from '@/i18n';
import { failureMessage } from '@/utils/failureMessage';

beforeEach(() => setLocale('en'));

it('gives the reason the backend gave, or a generic line when there is none', () => {
  expect(failureMessage(new Error('the NAS is already configured'))).toBe(
    'the NAS is already configured',
  );
  expect(failureMessage(new Error(''))).toBe(failureMessage('not an error'));
  expect(failureMessage(undefined)).not.toBe('');
});
