import { onBeforeUnmount, onMounted, watch, type Ref } from 'vue';

/**
 * Composable for polling an action at a reactive interval, only while the page is visible. A page
 * that comes back into view runs the action at once instead of showing what it read before.
 */
export function usePolling(action: () => void, intervalMs: Ref<number>) {
  let timer: ReturnType<typeof setInterval> | undefined;

  const visible = () => document.visibilityState === 'visible';

  function stop() {
    if (timer !== undefined) {
      clearInterval(timer);
      timer = undefined;
    }
  }

  function start() {
    stop();

    timer = setInterval(() => {
      if (visible()) {
        action();
      }
    }, intervalMs.value);
  }

  function resume() {
    if (visible()) {
      action();
      start();
    }
  }

  onMounted(() => {
    start();
    document.addEventListener('visibilitychange', resume);
  });

  watch(intervalMs, start);

  onBeforeUnmount(() => {
    stop();
    document.removeEventListener('visibilitychange', resume);
  });

  return { start, stop };
}
