import type { SignInPolicy, WidgetSettings } from '@/types/types';
import type { AccessRuleKind, AccessSettings } from './access';
import type { GeneralSettings } from './general';
import type { PowerSettings } from './power';

/**
 * An allowed address as the configuration carries it.
 */
export interface ConfigRule {
  kind: AccessRuleKind;
  value: string;
  label: string;
}

/**
 * One shortcut of the desktop, in the order of the file.
 */
export interface ConfigShortcut {
  kind: 'app' | 'folder';
  target: string;
  name?: string | null;
}

/**
 * The configuration the launcher exports and imports: never the name of the NAS, the network, the
 * account or the token. `apps` holds the settings of the applications chosen at the export, under
 * their identifiers; the page carries them as they are.
 */
export interface LauncherConfig {
  format: string;
  version: number;
  launcher: string;
  exportedAt: number;
  general: Omit<GeneralSettings, 'nasName'>;
  access: AccessSettings;
  accessRules: ConfigRule[];
  power: PowerSettings;
  widgets: WidgetSettings;
  desktop?: ConfigShortcut[] | null;
  apps?: Record<string, unknown>;
}

/**
 * What the file holds once decrypted: the part of the launcher and the sign-in policy of the
 * gateway.
 */
export interface ConfigContent {
  config: LauncherConfig;
  signIn: SignInPolicy | null;
}
