import type { DesktopBackground } from './appearance';

/** How big the icons of the desktop are drawn: `large` is the size they always had. */
export type DesktopIconSize = 'small' | 'medium' | 'large' | 'xlarge';

/**
 * Types for general settings.
 */
export interface GeneralSettings {
  nasName: string;
  theme: 'system' | 'light' | 'dark';
  /** The theme of each type, by id (`themes.ts`); the default for an id no longer offered. */
  themes: { light: string; dark: string };
  language: 'it' | 'en';
  createShortcut: boolean;
  desktopIconSize: DesktopIconSize;
  accent: string;
  background: DesktopBackground;
}
