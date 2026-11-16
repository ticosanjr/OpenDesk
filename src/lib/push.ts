import { apiGet, apiPost } from "@/lib/api";

export interface PushSubscriptionPayload {
  endpoint: string;
  keys: {
    p256dh: string;
    auth: string;
  };
}

export function urlBase64ToUint8Array(value: string): Uint8Array<ArrayBuffer> {
  const padding = "=".repeat((4 - (value.length % 4)) % 4);
  const base64 = (value + padding).replace(/-/g, "+").replace(/_/g, "/");
  const raw = window.atob(base64);
  const bytes = new Uint8Array(new ArrayBuffer(raw.length));
  for (let index = 0; index < raw.length; index += 1) {
    bytes[index] = raw.charCodeAt(index);
  }
  return bytes;
}

export function pushSupported(): boolean {
  return (
    typeof window !== "undefined" &&
    !("__TAURI_INTERNALS__" in window) &&
    "serviceWorker" in navigator &&
    "PushManager" in window &&
    "Notification" in window
  );
}

async function registration(): Promise<ServiceWorkerRegistration> {
  const existing = await navigator.serviceWorker.getRegistration();
  if (existing) return existing;
  return navigator.serviceWorker.register("/sw.js");
}

function toPayload(subscription: PushSubscription): PushSubscriptionPayload {
  const json = subscription.toJSON();
  const p256dh = json.keys?.p256dh;
  const auth = json.keys?.auth;
  if (!json.endpoint || !p256dh || !auth) {
    throw new Error("O navegador devolveu uma assinatura push incompleta.");
  }
  return { endpoint: json.endpoint, keys: { p256dh, auth } };
}

export async function currentPushSubscription(): Promise<PushSubscription | null> {
  if (!pushSupported()) return null;
  const worker = await registration();
  return worker.pushManager.getSubscription();
}

/** Subscribes this browser and registers the endpoint with the daemon. */
export async function subscribeToPush(): Promise<PushSubscriptionPayload> {
  if (!pushSupported()) {
    throw new Error("Este navegador não suporta Web Push.");
  }
  const permission = await Notification.requestPermission();
  if (permission !== "granted") {
    throw new Error("Permissão de notificação negada pelo navegador.");
  }
  const { public_key: publicKey } = await apiGet<{ public_key: string }>(
    "/api/push/vapid",
  );
  const worker = await registration();
  const existing = await worker.pushManager.getSubscription();
  const subscription =
    existing ??
    (await worker.pushManager.subscribe({
      userVisibleOnly: true,
      applicationServerKey: urlBase64ToUint8Array(publicKey),
    }));
  const payload = toPayload(subscription);
  await apiPost("/api/push/subscribe", {
    endpoint: payload.endpoint,
    keys: payload.keys,
    label: navigator.userAgent.slice(0, 120),
  });
  return payload;
}

export async function unsubscribeFromPush(): Promise<void> {
  const subscription = await currentPushSubscription();
  if (!subscription) return;
  await apiPost("/api/push/unsubscribe", { endpoint: subscription.endpoint }).catch(
    () => undefined,
  );
  await subscription.unsubscribe();
}

export async function sendTestPush(): Promise<number> {
  const result = await apiPost<{ delivered: number }>("/api/push/test");
  return result.delivered;
}
