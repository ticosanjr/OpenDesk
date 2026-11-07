import { beforeEach, describe, expect, it, vi } from "vitest";
import {
  isTerminalTaskStatus,
  markNotificationOnce,
  resetNotificationKeys,
} from "./notification-policy";

vi.mock("@tauri-apps/plugin-notification", () => ({
  isPermissionGranted: vi.fn().mockResolvedValue(true),
  requestPermission: vi.fn().mockResolvedValue("granted"),
  sendNotification: vi.fn(),
}));

describe("notification policy", () => {
  beforeEach(() => resetNotificationKeys());

  it("deduplicates the same notification key", () => {
    expect(markNotificationOnce("run:1:SUCCESS")).toBe(true);
    expect(markNotificationOnce("run:1:SUCCESS")).toBe(false);
    expect(markNotificationOnce("run:2:SUCCESS")).toBe(true);
  });

  it("only treats terminal task statuses as notification-worthy", () => {
    expect(isTerminalTaskStatus("DONE")).toBe(true);
    expect(isTerminalTaskStatus("BLOCKED")).toBe(true);
    expect(isTerminalTaskStatus("CANCELLED")).toBe(true);
    expect(isTerminalTaskStatus("IN_PROGRESS")).toBe(false);
    expect(isTerminalTaskStatus("IN_REVIEW")).toBe(false);
  });
});
