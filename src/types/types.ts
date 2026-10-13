/**
 * Launcher types.
 */
import type { Component } from 'vue';

/**
 * Languages supported by the launcher.
 */
export type Locale = 'it' | 'en';

/**
 * An application installed on the system.
 */
export interface InstalledApp {
  id: string;
  name: string;
  info: string;
  system: boolean;
  icon?: string;
  category?: string | null;
}

/**
 * How the shell displays an application.
 * `fullscreen`: occupies the area under the top bar and uses `AppView` for its frame.
 * `modal`: overlays the desktop and extends `CommonModal`.
 */
export type AppPresentation = 'fullscreen' | 'modal';

/**
 * Where the interface comes from. An internal application is compiled into the launcher, while an external one
 * is served by its own process and lives in an iframe. The shell opens them in the same way.
 */
export type AppSource =
  { kind: 'internal'; component: Component } | { kind: 'external'; url: string };

// Definition of an application entry in the registry.
export interface AppEntry {
  label: string;
  presentation: AppPresentation;
  icon: string;
  source: AppSource;
}

/**
 * An application installed along with what the shell needs to open it.
 */
export interface ShellApp extends InstalledApp {
  entry: AppEntry;
}

/**
 * A desktop icon. The backend saves it in the database.
 * `kind` distinguishes applications from folders
 */
export interface DesktopShortcut {
  kind: 'app' | 'folder';
  target: string;
  name: string | null;
  position: number;
}

export interface DesktopItem {
  key: string;
  kind: 'app' | 'folder';
  target: string;
  name: string;
  icon: string;
  available: boolean;
}

// A folder in the application menu.
export interface MenuFolder {
  id: string;
  name: string;
}

// Common settings for the header's popup widgets.
export interface WidgetSettings {
  pollingSeconds: number;
}

// System actions offered by the header.
export type SystemCommand = 'restart' | 'shutdown' | 'logout';

// A system menu item. The identifier is also the key of its label.
/**
 * What a restart or a shutdown would interrupt (the operations of the applications, by name) or has
 * to wait for (the jobs of root: update, apt, initialize, rename, eject, packages).
 */
export interface PowerBusy {
  tasks: string[];
  jobs: string[];
}

export interface SystemAction {
  id: SystemCommand;
  icon: string;
}

// The tone of an alert (determines icon and color).
export type AlertTone = 'info' | 'success' | 'warning' | 'error';

// One of the steps the launcher runs on its own once the wizard has configured the NAS: an
// application the catalogue wants installed from the start.
export interface SetupStep {
  id: string;
  kind: 'app';
  name: string;
  state: 'pending' | 'running' | 'done' | 'failed';
  detail: string;
}

// `completed` once the wizard has configured the NAS, `ready` once the steps that follow are done.
export interface SetupState {
  completed: boolean;
  ready: boolean;
  steps: SetupStep[];
}

// Where the first configuration is: not done by the wizard yet, the steps after it, or nothing left.
export type SetupPhase = 'unconfigured' | 'preparing' | 'ready';

export type Theme = 'system' | 'light' | 'dark';

// How many refused sign-ins lock an address, and for how many minutes. The gateway keeps it.
export interface SignInPolicy {
  enabled: boolean;
  attempts: number;
  lockMinutes: number;
}

// Result of a login attempt: either the username, or the reason for rejection.
export type LoginResult =
  | { success: true; username: string }
  | {
      success: false;
      code: 'invalid_input' | 'refused' | 'throttled' | 'busy' | 'unavailable';
      message: string;
    };

// The protocol between the shell and an external application, carried by `postMessage`.
// Every message declares `nas: 1`: the frame of an application shares its window with whatever
// else posts there, and a message without that mark is not part of this conversation.
export const APP_PROTOCOL = 1;

// What the shell tells an application about the launcher it is running inside.
export interface AppContext {
  app: string;
  nasName: string;
  theme: string;
  accent: string;
  locale: string;
  /** The colours of the theme drawn now, for the application to take the same surfaces. */
  palette: Record<ThemeRole, string>;
}

// The roles of the colours of a theme, as an application's `--nas-<role>` tokens name them.
export type ThemeRole =
  'background' | 'foreground' | 'surface' | 'sunken' | 'hover' | 'border' | 'muted';

// A notification as the shell shows it: its text in the language of the page, its source named.
export interface DescribedNotification {
  id: string;
  title: string;
  message: string;
  source: string;
}

// A file or folder selected in a provider. `url` is the provider's authenticated download URL,
// absent for a folder.
export interface PickedFile {
  name: string;
  path: string;
  url?: string;
}

// A network share the network-disks provider reports as mounted.
export interface NetworkDisk {
  name: string;
  mount: string;
  source: string | null;
  readOnly: boolean;
}

// Shell to application.
export type ShellMessage =
  | { nas: typeof APP_PROTOCOL; type: 'context'; context: AppContext }
  | { nas: typeof APP_PROTOCOL; type: 'described'; notifications: DescribedNotification[] }
  | {
      nas: typeof APP_PROTOCOL;
      type: 'shortcut-result';
      requestId: string;
      success: boolean;
      error?: string;
    }
  | {
      nas: typeof APP_PROTOCOL;
      type: 'pick-files-result';
      requestId: string;
      files: PickedFile[];
      cancelled: boolean;
      error?: string;
    }
  | {
      nas: typeof APP_PROTOCOL;
      type: 'initialize-disk-result';
      requestId: string;
      success: boolean;
      taskId?: string;
      error?: string;
    }
  | {
      nas: typeof APP_PROTOCOL;
      type: 'rename-disk-result';
      requestId: string;
      success: boolean;
      error?: string;
    }
  | {
      nas: typeof APP_PROTOCOL;
      type:
        | 'smart-disk-result'
        | 'network-disks-result'
        | 'power-schedule-result'
        | DiagnosticReply
        | AccessReply
        | DiskReply
        | SystemMonitorReply;
      requestId: string;
      success: boolean;
      data?: unknown;
      error?: string;
    };

// What the log manager may ask about the notifications, and the answers it receives.
export type DiagnosticRequest = 'diagnostics' | 'diagnostic-log' | 'remove-diagnostic';
export type DiagnosticReply = `${DiagnosticRequest}-result`;

// What the disk manager may ask about the external disks, and the answers it receives.
export type DiskRequest = 'eject-disk' | 'disk-locks' | 'lock-disk';
export type DiskReply = `${DiskRequest}-result`;

// What the security center may ask about the devices allowed in with mTLS.
export type SecurityDeviceRequest =
  | 'mtls-state'
  | 'save-mtls'
  | 'devices'
  | 'add-device'
  | 'update-device'
  | 'renew-device'
  | 'revoke-device'
  | 'remove-device';

// What the security center may ask about the certificate HTTPS answers with.
export type SecurityCertificateRequest =
  | 'certificate'
  | 'save-certificate-names'
  | 'renew-certificate'
  | 'upload-certificate'
  | 'remove-custom-certificate';

export type AntivirusRequest =
  | 'antivirus-status'
  | 'antivirus-enable'
  | 'antivirus-settings'
  | 'antivirus-scan'
  | 'antivirus-quarantine'
  | 'antivirus-restore'
  | 'antivirus-delete'
  | 'antivirus-forget';

// What the security center may ask about who reaches the NAS, and the answers it receives.
export type AccessRequest =
  | SecurityDeviceRequest
  | SecurityCertificateRequest
  | AntivirusRequest
  | 'access-settings'
  | 'save-access-settings'
  | 'discovery-settings'
  | 'save-discovery-settings'
  | 'access-rules'
  | 'add-access-rule'
  | 'remove-access-rule'
  | 'sign-in-policy'
  | 'save-sign-in-policy'
  | 'refused-sign-ins';
export type AccessReply = `${AccessRequest}-result`;

// Privileged process actions, and telemetry forwarding for older System Monitor clients.
export type SystemMonitorRequest = 'system-telemetry' | 'process-action';
export type SystemMonitorReply = `${SystemMonitorRequest}-result`;

// A refused sign-in, as the launcher noted it while the notifications for them are on.
export interface RefusedSignIn {
  id: string;
  at: number;
  from: string;
  field: string;
}

// Application to shell: it asks for the context, for a notification, or for a window.
export type AppMessage =
  | { nas: typeof APP_PROTOCOL; type: 'ready' }
  | {
      nas: typeof APP_PROTOCOL;
      type: 'notify';
      title: string;
      message?: string;
      tone?: AlertTone;
      log?: string;
    }
  | { nas: typeof APP_PROTOCOL; type: 'open'; app: string; location?: string }
  | { nas: typeof APP_PROTOCOL; type: 'open-https' }
  | { nas: typeof APP_PROTOCOL; type: 'describe'; notifications: unknown }
  | { nas: typeof APP_PROTOCOL; type: 'shortcut'; requestId: string; path: string; name: string }
  | {
      nas: typeof APP_PROTOCOL;
      type: 'pick-files';
      requestId: string;
      multiple?: boolean;
      extensions?: string[];
      directory?: string;
      folder?: boolean;
    }
  | { nas: typeof APP_PROTOCOL; type: 'network-disks'; requestId: string }
  | { nas: typeof APP_PROTOCOL; type: 'power-schedule'; requestId: string }
  | {
      nas: typeof APP_PROTOCOL;
      type: 'initialize-disk';
      requestId: string;
      device: string;
      label: string;
    }
  | {
      nas: typeof APP_PROTOCOL;
      type: 'rename-disk';
      requestId: string;
      device: string;
      label: string;
    }
  | {
      nas: typeof APP_PROTOCOL;
      type: 'smart-disk';
      requestId: string;
      device: string;
    }
  | {
      nas: typeof APP_PROTOCOL;
      type: DiagnosticRequest;
      requestId: string;
      notification?: string;
    }
  | {
      nas: typeof APP_PROTOCOL;
      type: DiskRequest;
      requestId: string;
      device?: unknown;
      uuid?: unknown;
      locked?: unknown;
    }
  | { nas: typeof APP_PROTOCOL; type: AccessRequest; requestId: string; payload?: unknown }
  | {
      nas: typeof APP_PROTOCOL;
      type: SystemMonitorRequest;
      requestId: string;
      payload?: unknown;
    };

// A column of `CommonTable`: the field it shows and the catalogue key of its heading.
export interface TableColumn<T> {
  key: keyof T & string;
  label: string;
  sortable?: boolean;
  class?: string;
}

/** Connected external disks, as listed by the launcher. */
export type ExternalDisk = import('@/types/api').components['schemas']['ExternalDisk'];

// The account of the NAS as the gateway reports it: `email` is the recovery address, once verified,
// and `recovery` whether the password may be reset with a code mailed to it.
export interface AccountInfo {
  username: string;
  email: string | null;
  recovery: boolean;
}

// Whether the Email app is installed, and whether it can send a code to an address.
export interface MailStatus {
  installed: boolean;
  sender: boolean;
}

/**
 * Who signed in, and the device the browser signed in from when it showed a certificate of one
 * (mTLS): the gateway reads it from the TLS handshake, never from the page.
 */
export interface GatewayLogin {
  username: string;
  device: string | null;
}

// What the disk manager sends with a request about an external disk, checked before any use.
export interface DiskPayload {
  device?: unknown;
  uuid?: unknown;
  locked?: unknown;
}
