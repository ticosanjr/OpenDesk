import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("@/lib/api", () => ({
  apiGet: vi.fn(),
  apiPost: vi.fn(),
}));

import { apiGet, apiPost } from "@/lib/api";

const originalServiceWorker = Object.getOwnPropertyDescriptor(
  navigator,
  "serviceWorker",
);

function installPushEnvironment(options: {
  permission?: string;
  existing?: unknown;
  registration?: unknown;
  subscribe?: unknown;
  getRegistration?: unknown;
} = {}) {
  Object.defineProperty(window, "PushManager", {
    value: function PushManager() {},
    configurable: true,
  });
  Object.defineProperty(window, "Notification", {
    value: {
      requestPermission: vi
        .fn()
        .mockResolvedValue(options.permission ?? "granted"),
    },
    configurable: true,
  });
  const subscription = {
    endpoint: "https://push.example/abc",
    toJSON: () => ({
      endpoint: "https://push.example/abc",
      keys: { p256dh: "p256dh-key", auth: "auth-key" },
    }),
    unsubscribe: vi.fn().mockResolvedValue(true),
  };
  const registration = {
    pushManager: {
      getSubscription: vi.fn().mockResolvedValue(options.existing ?? null),
      subscribe: vi
        .fn()
        .mockResolvedValue(options.subscribe ?? subscription),
    },
  };
  Object.defineProperty(navigator, "serviceWorker", {
    value: {
      getRegistration: vi
        .fn()
        .mockResolvedValue(
          options.getRegistration === undefined
            ? registration
            : options.getRegistration,
        ),
      register: vi.fn().mockResolvedValue(registration),
    },
    configurable: true,
  });
  return { registration, subscription };
}

afterEach(() => {
  vi.clearAllMocks();
  Reflect.deleteProperty(window, "__TAURI_INTERNALS__");
  Reflect.deleteProperty(window, "PushManager");
  Reflect.deleteProperty(window, "Notification");
  if (originalServiceWorker) {
    Object.defineProperty(navigator, "serviceWorker", originalServiceWorker);
  } else {
    Reflect.deleteProperty(navigator, "serviceWorker");
  }
});

beforeEach(() => {
  vi.resetModules();
});

describe("web push client", () => {
  it("decodes the VAPID application server key", async () => {
    const { urlBase64ToUint8Array } = await import("./push");
    const bytes = urlBase64ToUint8Array("AQID");
    expect(Array.from(bytes)).toEqual([1, 2, 3]);
  });

  it("is unavailable inside Tauri or without PushManager", async () => {
    installPushEnvironment();
    Object.defineProperty(window, "__TAURI_INTERNALS__", {
      value: {},
      configurable: true,
    });
    const { pushSupported } = await import("./push");
    expect(pushSupported()).toBe(false);

    Reflect.deleteProperty(window, "__TAURI_INTERNALS__");
    Reflect.deleteProperty(window, "PushManager");
    expect(pushSupported()).toBe(false);
  });

  it("subscribes the browser and registers the endpoint", async () => {
    const { registration } = installPushEnvironment();
    vi.mocked(apiGet).mockResolvedValue({ public_key: "AQID" });
    vi.mocked(apiPost).mockResolvedValue({});

    const { subscribeToPush } = await import("./push");
    const payload = await subscribeToPush();

    expect(registration.pushManager.subscribe).toHaveBeenCalledWith({
      userVisibleOnly: true,
      applicationServerKey: expect.any(Uint8Array),
    });
    expect(apiPost).toHaveBeenCalledWith(
      "/api/push/subscribe",
      expect.objectContaining({
        endpoint: "https://push.example/abc",
        keys: { p256dh: "p256dh-key", auth: "auth-key" },
      }),
    );
    expect(payload.endpoint).toBe("https://push.example/abc");
  });

  it("rejects denied permission and unsupported browsers", async () => {
    const { subscribeToPush } = await import("./push");
    await expect(subscribeToPush()).rejects.toThrow(/não suporta/);

    installPushEnvironment({ permission: "denied" });
    await expect(subscribeToPush()).rejects.toThrow(/negada/);
  });

  it("reuses an existing subscription when keys are complete", async () => {
    const existing = {
      endpoint: "https://push.example/old",
      toJSON: () => ({
        endpoint: "https://push.example/old",
        keys: { p256dh: "k", auth: "a" },
      }),
      unsubscribe: vi.fn(),
    };
    const { registration } = installPushEnvironment({ existing });
    vi.mocked(apiGet).mockResolvedValue({ public_key: "AQID" });
    vi.mocked(apiPost).mockResolvedValue({});
    const { subscribeToPush } = await import("./push");
    const payload = await subscribeToPush();
    expect(registration.pushManager.subscribe).not.toHaveBeenCalled();
    expect(payload.endpoint).toBe("https://push.example/old");
  });

  it("rejects an incomplete browser subscription", async () => {
    installPushEnvironment({
      existing: {
        endpoint: "https://push.example/broken",
        toJSON: () => ({ endpoint: "https://push.example/broken" }),
        unsubscribe: vi.fn(),
      },
    });
    vi.mocked(apiGet).mockResolvedValue({ public_key: "AQID" });
    const { subscribeToPush } = await import("./push");
    await expect(subscribeToPush()).rejects.toThrow(/incompleta/);
  });

  it("registers the worker when no registration exists", async () => {
    installPushEnvironment({ getRegistration: null });
    vi.mocked(apiGet).mockResolvedValue({ public_key: "AQID" });
    vi.mocked(apiPost).mockResolvedValue({});
    const { subscribeToPush } = await import("./push");
    await subscribeToPush();
    expect(navigator.serviceWorker.register).toHaveBeenCalledWith("/sw.js");
  });

  it("unsubscribes locally even when the daemon call fails", async () => {
    const existing = {
      endpoint: "https://push.example/abc",
      toJSON: () => ({
        endpoint: "https://push.example/abc",
        keys: { p256dh: "k", auth: "a" },
      }),
      unsubscribe: vi.fn().mockResolvedValue(true),
    };
    installPushEnvironment({ existing });
    vi.mocked(apiPost).mockRejectedValueOnce(new Error("offline"));
    const { unsubscribeFromPush } = await import("./push");
    await unsubscribeFromPush();
    expect(existing.unsubscribe).toHaveBeenCalled();
    expect(apiPost).toHaveBeenCalledWith("/api/push/unsubscribe", {
      endpoint: "https://push.example/abc",
    });
  });

  it("does nothing when there is no current subscription", async () => {
    installPushEnvironment({ existing: null });
    const { unsubscribeFromPush, currentPushSubscription } = await import(
      "./push"
    );
    expect(await currentPushSubscription()).toBeNull();
    await unsubscribeFromPush();
    expect(apiPost).not.toHaveBeenCalled();
  });

  it("sends a test notification and returns the delivered count", async () => {
    vi.mocked(apiPost).mockResolvedValue({ delivered: 2 });
    const { sendTestPush } = await import("./push");
    expect(await sendTestPush()).toBe(2);
  });
});
