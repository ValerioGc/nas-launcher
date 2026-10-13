// One application the register says is installed, or one the launcher is made of.
export interface ManagedApp {
  id: string;
  name: string;
  info: string;
  system: boolean;
  version: string;
  running: boolean;
  icon?: string;
}

// One application the catalogue offers, crossed with what is installed and available updates.
export interface AppOffer {
  id: string;
  name: string;
  system?: boolean;
  info: string;
  repository: string;
  logo: string;
  installedVersion: string | null;
  latestVersion: string | null;
  updateAvailable: boolean;
}

// Whether the launcher may fetch anything, and whether its configured credential is present.
export interface ForgeCredentials {
  configured: boolean;
  packagesEnabled: boolean;
  appsManaged: boolean;
}

// Actions available in the app manager.
export type AppAction = 'install' | 'uninstall' | 'details';

// Only the actions that change the system state, and therefore require confirmation.
export type ManageAction = Exclude<AppAction, 'details'>;

// Detailed information about an app.
export interface AppDetail {
  name: string;
  version: string;
  description: string;
  logo: string;
  url_repo: string;
  website: string;
}

export interface ManagerRow {
  id: string;
  name: string;
  info: string;
  system: boolean;
  version: string;
  icon: string;
  repository: string;
  latestVersion: string | null;
  updateAvailable: boolean;
}

export type ManagerEndpoint = 'installed' | 'available';
export type ManagerView = 'list' | 'preview';

// Where a release is published.
export type Provider = 'github' | 'gitlab';

// The folders of the menu an application can be filed under.
export const APP_CATEGORIES = ['media', 'storage', 'network', 'system', 'development'] as const;
export type AppCategory = (typeof APP_CATEGORIES)[number];

// One entry of the catalogue, as the App Manager edits it.
export interface CatalogueEntry {
  id: string;
  name: string;
  system?: boolean;
  info: string;
  provider: Provider;
  repository: string;
  asset: string;
  preinstall: boolean;
  logo: string;
  category: AppCategory | null;
}

// What a logo may be: the launcher refuses anything else.
export const LOGO_TYPES = ['image/svg+xml', 'image/png', 'image/jpeg', 'image/webp'];
export const LOGO_MAX_BYTES = 256 * 1024;

// How often the NAS looks for newer releases on its own.
export type UpdateFrequency = 'hourly' | 'daily' | 'weekly';

// The settings section of the App Manager.
export interface ManagerSettings {
  autoCheck: boolean;
  frequency: UpdateFrequency;
  checkOnOpen: boolean;
  updateOnOpen: boolean;
}
