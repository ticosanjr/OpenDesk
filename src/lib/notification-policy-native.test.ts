import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("@tauri-apps/plugin-notification", () => ({
  isPermissionGranted: vi.fn(),
  requestPermission: vi.fn(),
  sendNotification: vi.fn(),
}));

import {
  isPermissionGranted,
  requestPermission,
  sendNotification,
} from "@tauri-apps/plugin-notification";

beforeEach(() => {
  vi.resetModules();
  vi.clearAllMocks();
  localStorage.clear();
});

afterEach(() => {
  vi.restoreAllMocks();
});

async function load() {
  return import("./notification-policy");
}

describe("native notification settings", () => {
  it("uses minimal defaults and honors explicit opt-ins", async () => {
    const mod = await load();
    expect(mod.nativeNotificationsEnabled()).toBe(true);
    expect(mod.artifactNotificationsEnabled()).toBe(false);
    expect(mod.notificationEnabled("run_finished")).toBe(true);
    expect(mod.notificationEnabled("run_failed")).toBe(true);
    expect(mod.notificationEnabled("interactions")).toBe(true);
    expect(mod.notificationEnabled("schedule_skips")).toBe(false);
    localStorage.setItem("opendesk.native_notifications", "false");
    localStorage.setItem("opendesk.artifact_notifications", "true");
    expect(mod.nativeNotificationsEnabled()).toBe(false);
    expect(mod.artifactNotificationsEnabled()).toBe(true);
    mod.setNotificationEnabled("artifacts", false);
    expect(mod.artifactNotificationsEnabled()).toBe(false);
    mod.resetNotificationPreferences();
    expect(mod.notificationEnabled("run_finished")).toBe(true);
    expect(mod.notificationEnabled("artifacts")).toBe(false);
  });

  it("defaults to minimal preferences when storage is unavailable", async () => {
    vi.stubGlobal("localStorage", undefined);
    const mod = await load();
    expect(mod.nativeNotificationsEnabled()).toBe(true);
    expect(mod.artifactNotificationsEnabled()).toBe(false);
    expect(mod.notificationEnabled("run_finished")).toBe(true);
    expect(mod.notificationEnabled("schedule_skips")).toBe(false);
    vi.unstubAllGlobals();
  });

  it("only sends when the document is hidden or unfocused", async () => {
    vi.spyOn(document, "hasFocus").mockReturnValue(true);
    Object.defineProperty(document, "hidden", {
      configurable: true,
      value: false,
    });
    const mod = await load();
    expect(mod.shouldSendNativeNotification()).toBe(false);

    vi.spyOn(document, "hasFocus").mockReturnValue(false);
    expect(mod.shouldSendNativeNotification()).toBe(true);
  });

  it("handles permission granted, denied and plugin failure", async () => {
    vi.mocked(isPermissionGranted).mockResolvedValueOnce(false);
    vi.mocked(requestPermission).mockResolvedValueOnce("granted");
    expect(await (await load()).ensureNativePermission()).toBe(true);

    vi.resetModules();
    vi.mocked(isPermissionGranted).mockResolvedValueOnce(true);
    expect(await (await load()).ensureNativePermission()).toBe(true);

    vi.resetModules();
    vi.mocked(isPermissionGranted).mockResolvedValueOnce(false);
    vi.mocked(requestPermission).mockResolvedValueOnce("denied");
    expect(await (await load()).ensureNativePermission()).toBe(false);

    vi.resetModules();
    vi.mocked(isPermissionGranted).mockRejectedValueOnce(new Error("nope"));
    expect(await (await load()).ensureNativePermission()).toBe(false);
  });

  it("sends, skips and swallows notification failures", async () => {
    const hidden = vi
      .spyOn(document, "hasFocus")
      .mockReturnValue(true);
    Object.defineProperty(document, "hidden", {
      configurable: true,
      value: false,
    });
    vi.mocked(isPermissionGranted).mockResolvedValue(true);
    vi.mocked(sendNotification).mockImplementation(() => undefined);

    let mod = await load();
    expect(await mod.sendNativeNotification("t", "b")).toBe(false);

    hidden.mockReturnValue(false);
    mod = await load();
    expect(await mod.sendNativeNotification("t", "b")).toBe(true);
    expect(sendNotification).toHaveBeenCalledWith({ title: "t", body: "b" });

    vi.resetModules();
    vi.mocked(sendNotification).mockImplementationOnce(() => {
      throw new Error("send failed");
    });
    mod = await load();
    expect(await mod.sendNativeNotification("t", "b")).toBe(false);
  });
});

describe("notification dedupe pressure", () => {
  it("prunes expired keys when the cache is full", async () => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2026-11-08T10:00:00Z"));
    const mod = await load();
    for (let index = 0; index < 500; index += 1) {
      expect(mod.markNotificationOnce(`key-${index}`)).toBe(true);
    }
    vi.setSystemTime(new Date("2026-11-08T10:20:00Z"));
    expect(mod.markNotificationOnce("fresh")).toBe(true);
    vi.useRealTimers();
  });
});

describe("notification preference coverage", () => {
  beforeEach(() => {
    vi.resetModules();
    localStorage.clear();
  });

  it("toggles every notification kind", async () => {
    const mod = await load();
    const kinds = [
      "run_finished",
      "run_failed",
      "interactions",
      "artifacts",
      "schedule_skips",
    ] as const;
    for (const kind of kinds) {
      mod.setNotificationEnabled(kind, false);
      expect(mod.notificationEnabled(kind)).toBe(false);
      mod.setNotificationEnabled(kind, true);
      expect(mod.notificationEnabled(kind)).toBe(true);
    }
    mod.resetNotificationPreferences();
    expect(mod.notificationEnabled("artifacts")).toBe(false);
    expect(mod.notificationEnabled("run_finished")).toBe(true);
  });

  it("honors the legacy artifact opt-out", async () => {
    localStorage.setItem("opendesk.artifact_notifications", "false");
    const mod = await load();
    expect(mod.artifactNotificationsEnabled()).toBe(false);
  });

  it("does not throw when storage is unavailable", async () => {
    vi.stubGlobal("localStorage", undefined);
    const mod = await load();
    expect(() => mod.setNotificationEnabled("run_finished", false)).not.toThrow();
    expect(() => mod.resetNotificationPreferences()).not.toThrow();
    expect(mod.notificationEnabled("run_finished")).toBe(true);
    vi.unstubAllGlobals();
  });
});
