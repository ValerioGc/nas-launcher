import { mount } from '@vue/test-utils';
import { beforeEach, expect, it } from 'vitest';
import CatalogueEditor from '@/apps/nas-app-manager/components/CatalogueEditor.vue';
import { setLocale } from '@/i18n';
import type { CatalogueEntry } from '@/apps/nas-app-manager/types';

const photos: CatalogueEntry = {
  id: 'photos',
  name: 'Photos',
  info: '',
  provider: 'github',
  repository: 'example/photos',
  asset: 'photos-{version}.tar.gz',
  preinstall: false,
  logo: '/api/apps/photos/logo?v=1',
  category: 'media',
};

beforeEach(() => setLocale('it'));

it('lists the catalogue with the logo of each entry', () => {
  const wrapper = mount(CatalogueEditor, { props: { apps: [photos] } });

  const row = wrapper.get('[data-testid="catalogue_row_photos"]');
  expect(row.text()).toContain('Photos');
  expect(row.text()).toContain('example/photos');
  expect(row.get('img').attributes('src')).toBe('/api/apps/photos/logo?v=1');
});

it('says when the catalogue is empty', () => {
  const wrapper = mount(CatalogueEditor, { props: { apps: [] } });

  expect(wrapper.text()).toContain('Il catalogo è vuoto.');
});

it('asks to add, edit or remove, naming the entry by its position', async () => {
  const wrapper = mount(CatalogueEditor, {
    props: { apps: [photos, { ...photos, id: 'disks', name: 'Disks' }] },
  });

  await wrapper.get('[data-testid="catalogue_add"]').trigger('click');
  await wrapper.get('[data-testid="catalogue_edit_disks"]').trigger('click');
  await wrapper.get('[data-testid="catalogue_remove_photos"]').trigger('click');

  expect(wrapper.emitted('add')).toHaveLength(1);
  expect(wrapper.emitted('edit')).toEqual([[1]]);
  expect(wrapper.emitted('remove')).toEqual([[0]]);
});
