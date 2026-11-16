import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("@/lib/api", () => ({
  apiGet: vi.fn(),
  apiPost: vi.fn(),
  apiDelete: vi.fn(),
}));

import { apiDelete, apiGet, apiPost } from "@/lib/api";
import {
  deletePasskey,
  fetchPasskeyStatus,
  fromBase64Url,
  hasSecureOrigin,
  listPasskeys,
  loginWithPasskey,
  passkeySupported,
  registerPasskey,
  toBase64Url,
} from "./passkey";

function defineWindow(name: string, value: unknown) {
  Object.defineProperty(window, name, { value, configurable: true });
}

function defineNavigator(name: string, value: unknown) {
  Object.defineProperty(navigator, name, { value, configurable: true });
}

function attestation() {
  return {
    id: "credential-id",
    rawId: new Uint8Array([1, 2]).buffer,
    type: "public-key",
    authenticatorAttachment: null,
    getClientExtensionResults: () => ({}),
    response: {
      clientDataJSON: new Uint8Array([3]).buffer,
      attestationObject: new Uint8Array([4]).buffer,
      getTransports: () => ["internal"],
    },
  } as unknown as PublicKeyCredential;
}

function assertion() {
  return {
    id: "credential-id",
    rawId: new Uint8Array([1, 2]).buffer,
    type: "public-key",
    authenticatorAttachment: null,
    getClientExtensionResults: () => ({}),
    response: {
      clientDataJSON: new Uint8Array([3]).buffer,
      authenticatorData: new Uint8Array([4]).buffer,
      signature: new Uint8Array([5]).buffer,
      userHandle: null,
    },
  } as unknown as PublicKeyCredential;
}

function installWebauthn() {
  defineWindow("PublicKeyCredential", function PublicKeyCredential() {});
  defineNavigator("credentials", {
    create: vi.fn(),
    get: vi.fn(),
  });
  return navigator.credentials as unknown as {
    create: ReturnType<typeof vi.fn>;
    get: ReturnType<typeof vi.fn>;
  };
}

beforeEach(() => {
  vi.clearAllMocks();
  window.localStorage.clear();
});

afterEach(() => {
  vi.unstubAllGlobals();
  Reflect.deleteProperty(window, "PublicKeyCredential");
  Reflect.deleteProperty(navigator, "credentials");
});

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
    installWebauthn();
    expect(passkeySupported()).toBe(true);
    expect(hasSecureOrigin()).toBe(true);
  });

  it("reads the public status and surfaces errors", async () => {
    const fetchMock = vi
      .fn()
      .mockResolvedValueOnce(
        new Response(JSON.stringify({ registered: 2, enabled: true }), {
          status: 200,
        }),
      );
    vi.stubGlobal("fetch", fetchMock);
    expect(await fetchPasskeyStatus()).toMatchObject({ registered: 2 });

    fetchMock.mockResolvedValueOnce(new Response("{}", { status: 500 }));
    await expect(fetchPasskeyStatus()).rejects.toThrow(/consultar passkeys/);

    fetchMock.mockResolvedValueOnce(new Response("{}", { status: 400 }));
    await expect(fetchPasskeyStatus()).rejects.toThrow(/consultar passkeys/);
  });

  it("registers a passkey with encoded options and credential", async () => {
    const credentials = installWebauthn();
    credentials.create.mockResolvedValue(attestation());
    vi.mocked(apiPost)
      .mockResolvedValueOnce({
        ceremony_id: "ceremony-1",
        options: {
          publicKey: {
            challenge: "AQID",
            user: { id: "AQID", name: "opendesk", displayName: "OpenDesk" },
            excludeCredentials: [{ id: "AQID", type: "public-key" }],
            rp: { id: "localhost", name: "OpenDesk" },
            pubKeyCredParams: [],
          },
        },
      })
      .mockResolvedValueOnce({ id: "passkey-1", label: "note" });

    const record = await registerPasskey("note");
    expect(record).toMatchObject({ id: "passkey-1" });
    expect(apiPost).toHaveBeenNthCalledWith(
      1,
      "/api/auth/passkey/register/start",
    );
    expect(apiPost).toHaveBeenNthCalledWith(
      2,
      "/api/auth/passkey/register/finish",
      expect.objectContaining({
        ceremony_id: "ceremony-1",
        label: "note",
        credential: expect.objectContaining({
          id: "credential-id",
          rawId: toBase64Url(new Uint8Array([1, 2])),
          response: expect.objectContaining({
            clientDataJSON: toBase64Url(new Uint8Array([3])),
            attestationObject: toBase64Url(new Uint8Array([4])),
            transports: ["internal"],
          }),
        }),
      }),
    );
  });

  it("rejects a cancelled registration", async () => {
    const credentials = installWebauthn();
    credentials.create.mockResolvedValue(null);
    vi.mocked(apiPost).mockResolvedValueOnce({
      ceremony_id: "c",
      options: { publicKey: { challenge: "AQID", user: { id: "AQID" } } },
    });
    await expect(registerPasskey("x")).rejects.toThrow(/cancelada/);
  });

  it("logs in and stores the session, rejecting cancellation", async () => {
    const credentials = installWebauthn();
    credentials.get.mockResolvedValue(assertion());
    const fetchMock = vi
      .fn()
      .mockResolvedValueOnce(
        new Response(
          JSON.stringify({
            ceremony_id: "c2",
            options: {
              publicKey: {
                challenge: "AQID",
                allowCredentials: [{ id: "AQID", type: "public-key" }],
              },
            },
          }),
          { status: 200 },
        ),
      )
      .mockResolvedValueOnce(
        new Response(JSON.stringify({ token: "session-token" }), {
          status: 200,
        }),
      );
    vi.stubGlobal("fetch", fetchMock);
    const reload = vi.fn();
    vi.stubGlobal("location", {
      hostname: "localhost",
      protocol: "http:",
      reload,
    });
    await loginWithPasskey();
    expect(window.localStorage.getItem("opendesk_api_token")).toBe(
      "session-token",
    );
    expect(reload).toHaveBeenCalled();

    fetchMock.mockResolvedValueOnce(
      new Response(
        JSON.stringify({
          ceremony_id: "c3",
          options: { publicKey: { challenge: "AQID" } },
        }),
        { status: 200 },
      ),
    );
    credentials.get.mockResolvedValue(null);
    await expect(loginWithPasskey()).rejects.toThrow(/cancelado/);
  });

  it("lists and deletes passkeys, returning empty on list errors", async () => {
    vi.mocked(apiGet).mockResolvedValueOnce([]);
    expect(await listPasskeys()).toEqual([]);
    vi.mocked(apiGet).mockRejectedValueOnce(new Error("offline"));
    expect(await listPasskeys()).toEqual([]);

    vi.mocked(apiDelete).mockResolvedValueOnce(undefined);
    await deletePasskey("p1");
    expect(apiDelete).toHaveBeenCalledWith("/api/auth/passkeys/p1");
  });

  it("surfaces a status fallback when the error body has no message", async () => {
    installWebauthn();
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValueOnce(new Response("{}", { status: 418 })),
    );
    await expect(loginWithPasskey()).rejects.toThrow(/falha \(418\)/);
  });

  it("rejects flows when WebAuthn is unavailable", async () => {
    await expect(registerPasskey("x")).rejects.toThrow(/não suporta/);
    await expect(loginWithPasskey()).rejects.toThrow(/não suporta/);
  });
});
