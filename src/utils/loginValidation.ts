import { t } from '@/i18n';

const USERNAME_LENGTH = { min: 3, max: 25 };
const PASSWORD_LENGTH = { min: 8, max: 128 };

function validate(value: string, label: string, length: { min: number; max: number }) {
  if (!value.trim()) {
    return t('fieldRequired', { field: t(label) });
  }

  if (value.length < length.min || value.length > length.max) {
    return t('fieldLength', length);
  }

  return '';
}

export function validateLogin(username: string, password: string) {
  return {
    username: validate(username.trim(), 'fieldUsername', USERNAME_LENGTH),
    password: validate(password, 'fieldPassword', PASSWORD_LENGTH),
  };
}
