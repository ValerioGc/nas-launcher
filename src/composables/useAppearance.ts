import { onBeforeUnmount, ref, watchEffect, type Ref } from 'vue';
import { useSettingsStore } from '@/apps/nas-system-settings/store';
import { desktopInk } from '@/apps/nas-system-settings/background';
import { themeOf } from '@/apps/nas-system-settings/themes';
import type { Theme } from '@/types/types';

/**
 * Applies the appearance the user chose: the type of theme, the theme of that type, the accent
 * colour and the ink the desktop labels are written in.
 */
export function useAppearance(preferred?: Ref<Theme>) {
  const settings = useSettingsStore();
  const media = window.matchMedia('(prefers-color-scheme: dark)');
  const dark = ref(media.matches);

  const update = () => {
    dark.value = media.matches;
  };

  media.addEventListener('change', update);

  watchEffect(() => {
    const general = settings.general;
    const preference = preferred?.value ?? general?.theme ?? 'system';
    const automatic = dark.value ? 'dark' : 'light';
    const theme = preference === 'system' ? automatic : preference;
    const root = document.documentElement;

    root.dataset.theme = theme;
    const palette = themeOf(theme, general?.themes?.[theme]);
    if (palette === 'default') {
      delete root.dataset.palette;
    } else {
      root.dataset.palette = palette;
    }

    if (general?.accent) {
      root.style.setProperty('--accent', general.accent);
    } else {
      root.style.removeProperty('--accent');
    }

    if (general?.background) {
      // The background of the theme drawn now, which the attributes above just chose.
      const surface =
        getComputedStyle(root).getPropertyValue('--theme-background').trim() ||
        (theme === 'dark' ? '#172027' : '#f5f7f6');
      const { ink, shadow } = desktopInk(general.background, surface);
      root.style.setProperty('--desktop-ink', ink);
      root.style.setProperty('--desktop-ink-shadow', shadow);
    }
  });

  onBeforeUnmount(() => media.removeEventListener('change', update));
}
