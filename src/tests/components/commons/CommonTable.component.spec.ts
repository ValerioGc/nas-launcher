import { mount } from '@vue/test-utils';
import { beforeEach, describe, expect, it } from 'vitest';
import CommonTable from '@/components/commons/CommonTable.vue';
import { setLocale } from '@/i18n';

interface Row {
  id: number;
  name: string;
  size: number;
}

const rows: Row[] = [
  { id: 1, name: 'beta', size: 30 },
  { id: 2, name: 'Alpha', size: 10 },
  { id: 3, name: 'gamma', size: 20 },
];

const columns = [
  { key: 'name' as const, label: 'name', sortable: true },
  { key: 'size' as const, label: 'size', class: 'size_cell' },
];

const names = (wrapper: ReturnType<typeof mount>) =>
  wrapper.findAll('tbody tr').map((row) => row.find('td').text());

beforeEach(() => setLocale('en'));

describe('CommonTable', () => {
  it('shows one row per item and one translated heading per column', () => {
    const wrapper = mount(CommonTable<Row>, {
      props: { rows, columns, rowKey: 'id', testid: 'list' },
    });

    expect(wrapper.findAll('th').map((th) => th.text())).toEqual(['Name', 'Size']);
    expect(names(wrapper)).toEqual(['beta', 'Alpha', 'gamma']);
    expect(wrapper.find('[data-testid="list_row_2"]').exists()).toBe(true);
    expect(wrapper.findAll('td.size_cell')).toHaveLength(3);
  });

  it('sorts by a sortable column, and reverses on a second click', async () => {
    const wrapper = mount(CommonTable<Row>, {
      props: { rows, columns, rowKey: 'id', testid: 'list' },
    });
    const heading = wrapper.get('th');
    expect(heading.attributes('aria-sort')).toBe('none');

    await wrapper.get('[data-testid="list_sort_name"]').trigger('click');
    expect(names(wrapper)).toEqual(['Alpha', 'beta', 'gamma']);
    expect(heading.attributes('aria-sort')).toBe('ascending');
    expect(wrapper.emitted('update:sortBy')?.[0]).toEqual(['name']);

    await wrapper.get('[data-testid="list_sort_name"]').trigger('click');
    expect(names(wrapper)).toEqual(['gamma', 'beta', 'Alpha']);
    expect(heading.attributes('aria-sort')).toBe('descending');
  });

  it('offers no sorting on a column that does not ask for it', () => {
    const wrapper = mount(CommonTable<Row>, { props: { rows, columns, rowKey: 'id' } });

    expect(wrapper.findAll('th')[1]!.find('button').exists()).toBe(false);
    expect(wrapper.findAll('th')[1]!.attributes('aria-sort')).toBeUndefined();
  });

  it('starts from the order it is given', () => {
    const wrapper = mount(CommonTable<Row>, {
      props: { rows, columns, rowKey: 'id', sortBy: 'size', ascending: false },
    });

    expect(names(wrapper)).toEqual(['beta', 'gamma', 'Alpha']);
  });

  it('renders a cell slot, the actions slot and a class per row', () => {
    const wrapper = mount(CommonTable<Row>, {
      props: { rows, columns, rowKey: 'id', rowClass: (row: Row) => ({ big: row.size > 25 }) },
      slots: {
        cell_size: '<template #cell_size="{ row }">{{ row.size }} kB</template>',
        actions: '<template #actions="{ row }"><button>Remove {{ row.name }}</button></template>',
      },
    });

    expect(wrapper.findAll('th')).toHaveLength(3);
    expect(wrapper.findAll('th')[2]!.text()).toBe('Actions');
    expect(wrapper.findAll('tbody tr')[0]!.text()).toContain('30 kB');
    expect(wrapper.findAll('tbody button').map((button) => button.text())).toContain('Remove beta');
    expect(wrapper.findAll('tr.big')).toHaveLength(1);
  });

  it('opens a menu for a row only when one is asked for', async () => {
    const plain = mount(CommonTable<Row>, { props: { rows, columns, rowKey: 'id' } });
    const untouched = new MouseEvent('contextmenu', { cancelable: true });
    plain.get('tbody tr').element.dispatchEvent(untouched);
    expect(untouched.defaultPrevented).toBe(false);
    expect(plain.get('tbody tr').attributes('tabindex')).toBeUndefined();

    const opened: string[] = [];
    const withMenu = mount(CommonTable<Row>, {
      props: { rows, columns, rowKey: 'id', onRowContext: (row: Row) => opened.push(row.name) },
    });
    const asked = new MouseEvent('contextmenu', { cancelable: true });
    withMenu.get('tbody tr').element.dispatchEvent(asked);
    expect(asked.defaultPrevented).toBe(true);
    expect(opened).toHaveLength(1);
    expect(withMenu.get('tbody tr').attributes('tabindex')).toBe('0');
  });
});
