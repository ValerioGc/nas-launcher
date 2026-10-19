/**
 * Notification tone: determines the icon and banner color.
 */
export type NotificationTone = 'info' | 'success' | 'warning' | 'error';

/**
 * Application notification
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
 * A notification shown on screen for a moment
 */
export interface Toast {
  id: string;
  tone: NotificationTone;
  title: string;
  message?: string;
  action?: { label: string; run: () => unknown };
  duration?: number;
  notification?: AppNotification;
}
