import { describe, expect, it } from "vitest";
import {
  fromBase64Url,
  hasSecureOrigin,
  passkeySupported,
  toBase64Url,
} from "./passkey";

describe("passkey helpers", () => {
  it("roundtrips base64url bytes", () => {
    const bytes = new Uint8Array([0, 1, 2, 250, 255, 128]);
    const encoded = toBase64Url(bytes);
    expect(encoded).not.toContain("=");
    expect(encoded).not.toContain("+");
    const decoded = new Uint8Array(fromBase64Url(encoded));
    expect(Array.from(decoded)).toEqual(Array.from(bytes));
  });

  it("detects WebAuthn support and the current origin", () => {
    expect(passkeySupported()).toBe(false);
    Object.defineProperty(window, "PublicKeyCredential", {
      value: function PublicKeyCredential() {},
      configurable: true,
    });
    Object.defineProperty(navigator, "credentials", {
      value: {},
      configurable: true,
    });
    expect(passkeySupported()).toBe(true);
    Reflect.deleteProperty(window, "PublicKeyCredential");
    Reflect.deleteProperty(navigator, "credentials");
    // Vitest's jsdom runs on http://localhost.
    expect(hasSecureOrigin()).toBe(true);
  });
});
