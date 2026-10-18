/**
 * Notification tone: determines the icon and banner color.
 */
export type NotificationTone = 'info' | 'success' | 'warning' | 'error';

/**
 * Application notification: produced by the system or an installed application.
 */
export interface AppNotification {
  id: string;
  title: string;
  message: string;
  tone: NotificationTone;
  at: number;
  read: boolean;
  source: string;
  key?: string | null;
  params?: Record<string, string>;
  log?: string | null;
}

/**
 * A notification shown on screen for a moment: a new entry of the log, or a problem of the page
 * itself that the log cannot hold, such as a backend that does not answer.
 */
export interface Toast {
  id: string;
  tone: NotificationTone;
  title: string;
  message?: string;
  action?: { label: string; run: () => unknown };
  duration?: number;
  /** The notification of the log it stands for, whose message may hold a link. */
  notification?: AppNotification;
}
