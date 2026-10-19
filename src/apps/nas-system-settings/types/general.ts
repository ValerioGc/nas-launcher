import type { DesktopBackground } from './appearance';

export type DesktopIconSize = 'small' | 'medium' | 'large' | 'xlarge';

/**
 * Types for general settings.
 */
export interface GeneralSettings {
  nasName: string;
  theme: 'system' | 'light' | 'dark';
  themes: { light: string; dark: string };
  language: 'it' | 'en';
  createShortcut: boolean;
  desktopIconSize: DesktopIconSize;
  accent: string;
  background: DesktopBackground;
}
