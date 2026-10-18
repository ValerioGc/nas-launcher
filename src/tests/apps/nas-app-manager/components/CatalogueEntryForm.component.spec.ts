import { mount } from '@vue/test-utils';
import { beforeEach, expect, it, vi } from 'vitest';
import CatalogueEntryForm from '@/apps/nas-app-manager/components/CatalogueEntryForm.vue';
import { setLocale } from '@/i18n';
import type { CatalogueEntry } from '@/apps/nas-app-manager/types';

const entry: CatalogueEntry = {
  id: 'photos',
  name: 'Photos',
  info: '',
  provider: 'github',
  repository: 'example/photos',
  asset: 'photos-{version}.tar.gz',
  preinstall: false,
  logo: '',
  category: null,
};

beforeEach(() => setLocale('it'));

function form(existing: boolean) {
  return mount(CatalogueEntryForm, {
    props: { modelValue: { ...entry }, existing },
  });
}

it('names every field and keeps the identifier of an entry already there', () => {
  const wrapper = form(true);

  for (const id of ['id', 'name', 'category', 'provider', 'repository', 'asset', 'logo', 'info']) {
    expect(wrapper.get(`label[for="catalogue_${id}"]`).text()).not.toBe('');
  }
  expect(wrapper.get('[data-testid="catalogue_id"]').attributes('disabled')).toBeDefined();
  expect(form(false).get('[data-testid="catalogue_id"]').attributes('disabled')).toBeUndefined();
});

it('hands every change back and says when it is applied', async () => {
  const wrapper = form(false);

  expect(wrapper.get('[data-testid="catalogue_category"]').text()).toContain('Nessuna');
  await wrapper.get('[data-testid="catalogue_category"]').setValue('media');
  await wrapper.get('[data-testid="catalogue_name"]').setValue('Foto');
  await wrapper.get('[data-testid="catalogue_provider"]').setValue('gitlab');
  await wrapper.get('form').trigger('submit');

  const model = wrapper.props('modelValue');
  expect(model.name).toBe('Foto');
  expect(model.provider).toBe('gitlab');
  expect(model.category).toBe('media');
  expect(wrapper.emitted('apply')).toHaveLength(1);
  expect(wrapper.get('form').attributes('id')).toBe('catalogue_entry');
});

it('comes into view on the first field it lets change', () => {
  const wrapper = mount(CatalogueEntryForm, {
    props: { modelValue: { ...entry }, existing: true },
    attachTo: document.body,
  });

  expect(document.activeElement).toBe(wrapper.get('[data-testid="catalogue_name"]').element);
  wrapper.unmount();
});

function pick(wrapper: ReturnType<typeof form>, file: File) {
  const input = wrapper.get<HTMLInputElement>('[data-testid="catalogue_logo"]');
  Object.defineProperty(input.element, 'files', { value: [file], configurable: true });
  return input.trigger('change');
}

it('takes an image for the logo, refuses what is not one, and goes back to the default', async () => {
  const wrapper = mount(CatalogueEntryForm, {
    props: { modelValue: { ...entry, logo: '/api/apps/photos/logo?v=1' }, existing: true },
  });
  const preview = () => wrapper.get('[data-testid="catalogue_logo_preview"]').attributes('src');
  expect(preview()).toBe('/api/apps/photos/logo?v=1');

  await pick(wrapper, new File(['GIF89a'], 'photos.gif', { type: 'image/gif' }));
  expect(wrapper.text()).toContain('SVG, PNG, JPEG o WebP.');
  expect(wrapper.emitted('update:logo')).toBeUndefined();

  await pick(wrapper, new File([new Uint8Array(300 * 1024)], 'big.png', { type: 'image/png' }));
  expect(wrapper.text()).toContain('supera 256 KB');

  const image = new File(['<svg/>'], 'photos.svg', { type: 'image/svg+xml' });
  await pick(wrapper, image);
  expect(wrapper.emitted('update:logo')).toEqual([[image]]);
  await wrapper.setProps({ logo: image });
  await vi.waitFor(() => expect(preview()).toMatch(/^data:image\/svg\+xml/));

  await wrapper.get('[data-testid="catalogue_logo_remove"]').trigger('click');
  expect(wrapper.emitted('update:logo')?.at(-1)).toEqual([null]);
  await wrapper.setProps({ logo: null });
  expect(preview()).not.toContain('/api/apps/photos/logo');
  expect(wrapper.find('[data-testid="catalogue_logo_remove"]').exists()).toBe(false);
});
