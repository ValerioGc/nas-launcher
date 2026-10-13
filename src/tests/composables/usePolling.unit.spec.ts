import { mount } from '@vue/test-utils';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { nextTick } from 'vue';
import PollingProbe from '@tests/fixtures/PollingProbe.vue';

let state: DocumentVisibilityState = 'visible';

function setVisibility(value: DocumentVisibilityState) {
  state = value;
  document.dispatchEvent(new Event('visibilitychange'));
}

beforeEach(() => {
  vi.useFakeTimers();
  state = 'visible';
  vi.spyOn(document, 'visibilityState', 'get').mockImplementation(() => state);
});

afterEach(() => {
  vi.useRealTimers();
  vi.restoreAllMocks();
});

describe('polling', () => {
  it('runs at the interval, and follows a new one', async () => {
    const action = vi.fn();
    const wrapper = mount(PollingProbe, { props: { action } });

    vi.advanceTimersByTime(3000);
    expect(action).toHaveBeenCalledTimes(3);

    wrapper.vm.interval = 5000;
    await nextTick();
    vi.advanceTimersByTime(4000);
    expect(action).toHaveBeenCalledTimes(3);
    vi.advanceTimersByTime(1000);
    expect(action).toHaveBeenCalledTimes(4);
    wrapper.unmount();
  });

  it('pauses while hidden and catches up when the page is visible again', () => {
    const action = vi.fn();
    const wrapper = mount(PollingProbe, { props: { action } });

    setVisibility('hidden');
    vi.advanceTimersByTime(5000);
    expect(action).not.toHaveBeenCalled();

    setVisibility('visible');
    expect(action).toHaveBeenCalledOnce();
    wrapper.unmount();
  });

  it('stops for good once unmounted', () => {
    const action = vi.fn();
    mount(PollingProbe, { props: { action } }).unmount();

    vi.advanceTimersByTime(5000);
    setVisibility('visible');
    expect(action).not.toHaveBeenCalled();
  });
});
