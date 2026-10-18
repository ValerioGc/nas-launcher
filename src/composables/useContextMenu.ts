import { ref } from 'vue';

/**
 * Composable for managing the state of a context menu, including the selected item and its position.
 * @template T - The type of the item that can be selected in the context menu.
 */
export function useContextMenu<T>() {
  const open = ref(false);
  const target = ref<T | null>(null) as import('vue').Ref<T | null>;
  const point = ref({ x: 0, y: 0 });

  function show(item: T, event: MouseEvent | KeyboardEvent) {
    target.value = item;
    const bounds = (event.currentTarget as HTMLElement).getBoundingClientRect();
    const pointed = event instanceof MouseEvent && (event.clientX !== 0 || event.clientY !== 0);
    point.value = pointed
      ? { x: event.clientX, y: event.clientY }
      : { x: bounds.left, y: bounds.bottom };

    open.value = true;
  }

  function close() {
    open.value = false;
  }

  return { open, target, point, show, close };
}
