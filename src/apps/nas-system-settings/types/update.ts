/**
 * A Debian package with a newer version available.
 */
export interface SystemPackage {
  id: string;
  current: string;
  version: string;
  level: 'security' | 'stable' | 'other';
}

/**
 * The upgrades apt offers, the job running on them if any, and whether this installation may run
 * one at all.
 */
export interface SystemUpdates {
  packages: SystemPackage[];
  running: 'refresh' | 'upgrade' | null;
  manageable: boolean;
}

/** A part of the NAS released on its own. */
export type NasComponent = 'launcher' | 'gateway' | 'wizard';

/**
 * One component: the version installed (null when it cannot say it), the newer release the last
 * check found, and the repository it comes from.
 */
export interface ComponentUpdate {
  component: NasComponent;
  currentVersion: string | null;
  availableVersion: string | null;
  repository: string;
}

/** The components installed on the NAS, updated together. */
export interface NasUpdates {
  components: ComponentUpdate[];
  tokenConfigured: boolean;
}
