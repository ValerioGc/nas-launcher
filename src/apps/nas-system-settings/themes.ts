/**
 * The themes of each type, the first being the default of `src/styles/theme.scss`. Every other
 * one has its file in `src/styles/themes/<type>/_<id>.scss` and its name in the catalogues as
 * `theme_<id>`. A theme sets surfaces and text only: the accent stays the one of the settings.
 */
export const THEMES = {
  light: ['default', 'sand', 'lavender', 'glacier'],
  dark: ['default', 'graphite', 'midnight', 'forest'],
} as const;

export type ThemeKind = keyof typeof THEMES;

export function themeOf(kind: ThemeKind, chosen: string | undefined): string {
  return (THEMES[kind] as readonly string[]).includes(chosen ?? '')
    ? (chosen as string)
    : 'default';
}
