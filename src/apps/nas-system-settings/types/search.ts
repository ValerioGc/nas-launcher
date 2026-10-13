/**
 * The sections of the settings.
 */
export type SettingsSection = 'general' | 'account' | 'network' | 'power' | 'updates' | 'recovery';

/**
 * A settings entry the global search finds, with the section it opens on.
 */
export interface SettingsEntry {
  section: SettingsSection;
  label: string;
}
