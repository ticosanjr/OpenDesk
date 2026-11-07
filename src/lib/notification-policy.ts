import {
  isPermissionGranted,
  requestPermission,
  sendNotification,
} from "@tauri-apps/plugin-notification";

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

export function shouldSendNativeNotification(): boolean {
  return typeof document !== "undefined" && (document.hidden || !document.hasFocus());
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

export function isTerminalTaskStatus(status: string): boolean {
  return status === "DONE" || status === "BLOCKED" || status === "CANCELLED";
}
