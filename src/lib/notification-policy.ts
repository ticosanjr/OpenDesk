import {
  isPermissionGranted,
  requestPermission,
  sendNotification,
} from "@tauri-apps/plugin-notification";
export { isTerminalTaskStatus } from "./task-state";

const seen = new Map<string, number>();
const TTL_MS = 15 * 60 * 1000;
const MAX_KEYS = 500;

let permissionPromise: Promise<boolean> | null = null;

export function markNotificationOnce(key: string): boolean {
  const now = Date.now();
  if (seen.size >= MAX_KEYS) {
    for (const [item, at] of seen) {
      if (now - at > TTL_MS) seen.delete(item);
    }
    if (seen.size >= MAX_KEYS) seen.clear();
  }
  const previous = seen.get(key);
  if (previous !== undefined && now - previous < TTL_MS) return false;
  seen.set(key, now);
  return true;
}

export function resetNotificationKeys() {
  seen.clear();
}

export function nativeNotificationsEnabled(): boolean {
  return (
    typeof localStorage === "undefined" ||
    localStorage.getItem("opendesk.native_notifications") !== "false"
  );
}

export type NotificationKind =
  | "run_finished"
  | "run_failed"
  | "interactions"
  | "artifacts"
  | "schedule_skips";

const NOTIFICATION_KEYS: Record<NotificationKind, string> = {
  run_finished: "opendesk.notify_run_finished",
  run_failed: "opendesk.notify_run_failed",
  interactions: "opendesk.notify_interactions",
  artifacts: "opendesk.notify_artifacts",
  schedule_skips: "opendesk.notify_schedule_skips",
};

const NOTIFICATION_DEFAULTS: Record<NotificationKind, boolean> = {
  run_finished: true,
  run_failed: true,
  interactions: true,
  artifacts: false,
  schedule_skips: false,
};

export function notificationEnabled(kind: NotificationKind): boolean {
  if (typeof localStorage === "undefined") return NOTIFICATION_DEFAULTS[kind];
  const stored = localStorage.getItem(NOTIFICATION_KEYS[kind]);
  if (stored !== null) return stored !== "false";
  // Respect the previous artifact opt-in for users who had explicitly set it.
  if (kind === "artifacts") {
    const legacy = localStorage.getItem("opendesk.artifact_notifications");
    if (legacy !== null) return legacy !== "false";
  }
  return NOTIFICATION_DEFAULTS[kind];
}

export function setNotificationEnabled(kind: NotificationKind, value: boolean) {
  if (typeof localStorage === "undefined") return;
  localStorage.setItem(NOTIFICATION_KEYS[kind], String(value));
}

export function resetNotificationPreferences() {
  if (typeof localStorage === "undefined") return;
  for (const key of Object.values(NOTIFICATION_KEYS)) localStorage.removeItem(key);
  localStorage.removeItem("opendesk.artifact_notifications");
}

export function artifactNotificationsEnabled(): boolean {
  return notificationEnabled("artifacts");
}

export function shouldSendNativeNotification(): boolean {
  return (
    nativeNotificationsEnabled() &&
    typeof document !== "undefined" &&
    (document.hidden || !document.hasFocus())
  );
}

export async function ensureNativePermission(): Promise<boolean> {
  if (!permissionPromise) {
    permissionPromise = (async () => {
      try {
        let granted = await isPermissionGranted();
        if (!granted) granted = (await requestPermission()) === "granted";
        return granted;
      } catch {
        return false;
      }
    })();
  }
  return permissionPromise;
}

export async function sendNativeNotification(title: string, body: string) {
  if (!shouldSendNativeNotification()) return false;
  if (!(await ensureNativePermission())) return false;
  try {
    sendNotification({ title, body });
    return true;
  } catch {
    return false;
  }
}
