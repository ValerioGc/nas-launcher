import { onBeforeUnmount } from 'vue';

/**
 * Runs `action` once the caller has stopped asking for `delayMs`.
 *
 * A filter that applies while you type must not become one request per keystroke. `schedule`
 * pushes the call forward every time it is called, and only the last one survives. The timer is
 * cleared when the component goes away, so a pending call cannot land on an unmounted view.
 */
export function useDebounced(action: () => void, delayMs: number) {
  let timer: ReturnType<typeof setTimeout> | undefined;

  function cancel() {
    window.clearTimeout(timer);
    timer = undefined;
  }

  function schedule() {
    cancel();
    timer = setTimeout(action, delayMs);
  }

  onBeforeUnmount(cancel);

  return { schedule, cancel };
}
