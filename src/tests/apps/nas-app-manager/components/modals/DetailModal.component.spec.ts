import { mount, flushPromises } from '@vue/test-utils';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import DetailModal from '@/apps/nas-app-manager/components/modals/DetailModal.vue';
import { request } from '@/api/client';
import { setLocale } from '@/i18n';

vi.mock('@/api/client', () => ({ request: vi.fn() }));

const detail = {
  name: 'Server multimediale',
  version: '1.2.3',
  description: 'Riproduce musica e video.',
  logo: '/api/apps/mediaServer/logo?v=1',
  url_repo: 'https://example.test/repo',
  website: 'https://example.test',
};

async function open(props = {}) {
  const wrapper = mount(DetailModal, {
    props: { modelValue: false, appId: 'mediaServer', ...props },
    global: { stubs: { teleport: true } },
    attachTo: document.body,
  });
  await wrapper.setProps({ modelValue: true });
  await flushPromises();
  return wrapper;
}

beforeEach(() => {
  setLocale('it');
  vi.mocked(request).mockReset().mockResolvedValue(detail);
  document.body.innerHTML = '';
});

describe('application details', () => {
  it('names the repository and says when the application is not installed', async () => {
    vi.mocked(request).mockResolvedValue({
      ...detail,
      version: '',
      url_repo: 'https://github.com/ValerioGc/nas-picture-manager',
      website: '',
    });
    const wrapper = await open();

    const body = wrapper.get('[data-testid="detail_modal_body"]');
    expect(body.text()).toContain('Non installata');
    expect(body.get('a').text()).toBe('ValerioGc/nas-picture-manager');
    expect(body.findAll('a')).toHaveLength(1);
    wrapper.unmount();
  });

  it('asks for the details only when it opens', async () => {
    const wrapper = mount(DetailModal, {
      props: { modelValue: false, appId: 'mediaServer' },
      global: { stubs: { teleport: true } },
    });
    await flushPromises();
    expect(request).not.toHaveBeenCalled();

    await wrapper.setProps({ modelValue: true });
    await flushPromises();
    expect(request).toHaveBeenCalledWith('/apps/mediaServer/info');
    expect(wrapper.get('[data-testid="detail_modal_body"]').text()).toContain('1.2.3');
    expect(wrapper.get('.common_modal').classes()).toContain('common_modal_wide');
    expect(wrapper.get('.detail_information').text()).toContain('Riproduce musica e video.');
    wrapper.unmount();
  });

  it('says so when the details cannot be read', async () => {
    vi.mocked(request).mockRejectedValue(new Error('down'));
    const wrapper = await open();

    expect(wrapper.get('[data-testid="detail_modal_error"]').text()).not.toBe('');
    expect(wrapper.find('[data-testid="detail_modal_body"]').exists()).toBe(false);
    wrapper.unmount();
  });

  it.each([
    ['javascript:alert(1)', 0],
    ['not a url', 0],
    ['https://example.test', 1],
  ])('turns %s into %i links', async (website, links) => {
    vi.mocked(request).mockResolvedValue({ ...detail, website, url_repo: '', logo: '' });
    const wrapper = await open();

    expect(wrapper.findAll('[data-testid="detail_modal_body"] a')).toHaveLength(links);
    expect(wrapper.get('img').attributes('src')).not.toBe('');
    wrapper.unmount();
  });

  it('names and describes an application of the launcher in the language of the page', async () => {
    vi.mocked(request).mockResolvedValue({
      ...detail,
      name: 'Terminal',
      description: 'Command console of the NAS and system log.',
    });
    const wrapper = await open({ appId: 'nas-system-terminal' });

    expect(wrapper.get('h3').text()).toBe('Terminale');
    expect(wrapper.text()).toContain('Console dei comandi del NAS e log di sistema.');
    wrapper.unmount();
  });

  it('says nothing is selected when no application was given', async () => {
    const wrapper = await open({ appId: null });

    expect(request).not.toHaveBeenCalled();
    expect(wrapper.text()).toContain('Nessuna applicazione');
    wrapper.unmount();
  });
});
