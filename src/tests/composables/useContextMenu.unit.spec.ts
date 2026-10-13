import { describe, expect, it } from 'vitest';
import { useContextMenu } from '@/composables/useContextMenu';

/** Dispatches `event` on an element placed at 10, 20 to 110, 60, opening the menu from there. */
function openFrom(event: MouseEvent | KeyboardEvent) {
  const menu = useContextMenu<string>();
  const element = document.createElement('div');
  element.getBoundingClientRect = () => new DOMRect(10, 20, 100, 40);
  element.addEventListener(event.type, (received) =>
    menu.show('item', received as MouseEvent | KeyboardEvent),
  );
  element.dispatchEvent(event);
  return menu;
}

describe('useContextMenu', () => {
  it('opens where the pointer is', () => {
    const menu = openFrom(new MouseEvent('contextmenu', { clientX: 40, clientY: 50 }));

    expect(menu.open.value).toBe(true);
    expect(menu.target.value).toBe('item');
    expect(menu.point.value).toEqual({ x: 40, y: 50 });
  });

  it('opens under the element from the keyboard, or the menu key with no pointer', () => {
    expect(openFrom(new KeyboardEvent('keydown', { key: 'F10' })).point.value).toEqual({
      x: 10,
      y: 60,
    });
    expect(openFrom(new MouseEvent('contextmenu')).point.value).toEqual({ x: 10, y: 60 });
  });

  it('closes', () => {
    const menu = openFrom(new MouseEvent('contextmenu', { clientX: 1, clientY: 1 }));
    menu.close();
    expect(menu.open.value).toBe(false);
  });
});
