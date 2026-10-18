import type { SettingsEntry, SettingsSection } from '@/apps/nas-system-settings/types/search';
import generalIcon from '@/assets/icons/system/sliders.svg';
import accountIcon from '@/assets/icons/system/account.svg';
import networkIcon from '@/assets/icons/network/globe.svg';
import powerIcon from '@/assets/icons/system/power.svg';
import updatesIcon from '@/assets/icons/system/update.svg';
import recoveryIcon from '@/assets/icons/system/recovery.svg';

/**
 * The i18n key of each section title.
 */
export const sectionLabels: Record<SettingsSection, string> = {
  general: 'general',
  account: 'account',
  network: 'networkSettings',
  power: 'power',
  updates: 'updates',
  recovery: 'recovery',
};

/**
 * The icon of each section: beside its name in the settings and on its results in the search.
 */
export const sectionIcons: Record<SettingsSection, string> = {
  general: generalIcon,
  account: accountIcon,
  network: networkIcon,
  power: powerIcon,
  updates: updatesIcon,
  recovery: recoveryIcon,
};

/**
 * The settings entries the global search finds. `label` is an i18n key.
 */
export const settingsEntries: SettingsEntry[] = [
  { section: 'general', label: 'general' },
  { section: 'general', label: 'themeType' },
  { section: 'general', label: 'themeName' },
  { section: 'general', label: 'language' },
  { section: 'general', label: 'accentColor' },
  { section: 'general', label: 'background' },
  { section: 'account', label: 'account' },
  { section: 'account', label: 'accountUsername' },
  { section: 'account', label: 'accountPasswordTitle' },
  { section: 'account', label: 'accountEmail' },
  { section: 'network', label: 'networkSettings' },
  { section: 'network', label: 'availableNetworks' },
  { section: 'power', label: 'power' },
  { section: 'power', label: 'powerRules' },
  { section: 'updates', label: 'updates' },
  { section: 'updates', label: 'systemUpdates' },
  { section: 'updates', label: 'launcherUpdates' },
  { section: 'recovery', label: 'recovery' },
  { section: 'recovery', label: 'configTransfer' },
  { section: 'recovery', label: 'factoryReset' },
];
