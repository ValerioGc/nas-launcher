import { flushPromises, mount } from '@vue/test-utils';
import { beforeEach, expect, it, vi } from 'vitest';
import AccountSettingsTab from '@/apps/nas-system-settings/components/tabs/AccountSettingsTab.vue';
import { mailStatus, readAccount } from '@/api/account';
import { setLocale, t } from '@/i18n';

vi.mock('@/api/account', () => ({ readAccount: vi.fn(), mailStatus: vi.fn() }));

const stubs = { AccountUsername: true, AccountPassword: true, AccountEmail: true };

beforeEach(() => {
  setLocale('en');
  vi.clearAllMocks();
});

it('reads the account and whether mail can go, and shows the three parts', async () => {
  vi.mocked(readAccount).mockResolvedValue({ username: 'admin', email: null, recovery: false });
  vi.mocked(mailStatus).mockResolvedValue({ installed: true, sender: true });
  const wrapper = mount(AccountSettingsTab, { global: { stubs } });
  await flushPromises();
  expect(wrapper.get('h1').text()).toBe(t('account'));
  expect(wrapper.findComponent({ name: 'AccountUsername' }).props('account')).toEqual({
    username: 'admin',
    email: null,
    recovery: false,
  });
  expect(wrapper.findComponent({ name: 'AccountEmail' }).props('mail')).toEqual({
    installed: true,
    sender: true,
  });
  expect(wrapper.findComponent({ name: 'AccountPassword' }).exists()).toBe(true);
});

it('keeps going without the status of the mail, and retries a failed account', async () => {
  vi.mocked(readAccount)
    .mockRejectedValueOnce(new Error('offline'))
    .mockResolvedValue({ username: 'admin', email: 'me@example.com', recovery: false });
  vi.mocked(mailStatus).mockRejectedValue(new Error('no launcher'));
  const wrapper = mount(AccountSettingsTab, { global: { stubs } });
  await flushPromises();
  expect(wrapper.text()).toContain('offline');
  await wrapper.get('button').trigger('click');
  await flushPromises();
  expect(wrapper.findComponent({ name: 'AccountEmail' }).props('mail')).toEqual({
    installed: false,
    sender: false,
  });
});
