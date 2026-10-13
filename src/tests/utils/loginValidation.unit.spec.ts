import { beforeEach, describe, expect, it } from 'vitest';
import { setLocale } from '@/i18n';
import { validateLogin } from '@/utils/loginValidation';

beforeEach(() => setLocale('en'));

describe('sign-in validation', () => {
  it('asks the sign-in for a name of 3 to 25 characters and a password of 8 to 128', () => {
    expect(validateLogin('admin', 'long enough')).toEqual({ username: '', password: '' });
    expect(validateLogin('ab', 'short')).toEqual({
      username: 'Between 3 and 25 characters are required.',
      password: 'Between 8 and 128 characters are required.',
    });
  });
});
