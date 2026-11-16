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

function installPushEnvironment() {
  Object.defineProperty(window, "PushManager", {
    value: function PushManager() {},
    configurable: true,
  });
  Object.defineProperty(window, "Notification", {
    value: { requestPermission: vi.fn().mockResolvedValue("granted") },
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
      getSubscription: vi.fn().mockResolvedValue(null),
      subscribe: vi.fn().mockResolvedValue(subscription),
    },
  };
  Object.defineProperty(navigator, "serviceWorker", {
    value: {
      getRegistration: vi.fn().mockResolvedValue(registration),
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

  it("is unavailable inside Tauri", async () => {
    installPushEnvironment();
    Object.defineProperty(window, "__TAURI_INTERNALS__", {
      value: {},
      configurable: true,
    });
    const { pushSupported } = await import("./push");
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
});
