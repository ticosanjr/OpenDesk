/** WebAuthn helpers for the remote PWA. Options and credentials use base64url. */

import { apiDelete, apiGet, apiPost } from "@/lib/api";

export interface PasskeyStatus {
  registered: number;
  enabled: boolean;
  origin?: string | null;
  origin_available: boolean;
  origin_error?: string | null;
}

export interface PasskeyRecord {
  id: string;
  credential_id: string;
  label: string;
  created_at: string;
  last_used_at?: string | null;
}

type JsonObject = Record<string, unknown>;

export function passkeySupported(): boolean {
  return (
    typeof window !== "undefined" &&
    "PublicKeyCredential" in window &&
    "credentials" in navigator
  );
}

export function hasSecureOrigin(): boolean {
  if (typeof window === "undefined") return false;
  const host = window.location.hostname;
  return (
    window.location.protocol === "https:" ||
    host === "localhost" ||
    host === "127.0.0.1" ||
    host === "[::1]"
  );
}

export function toBase64Url(value: ArrayBuffer | Uint8Array): string {
  const bytes = value instanceof Uint8Array ? value : new Uint8Array(value);
  let binary = "";
  for (const byte of bytes) binary += String.fromCharCode(byte);
  return window
    .btoa(binary)
    .replace(/\+/g, "-")
    .replace(/\//g, "_")
    .replace(/=+$/, "");
}

export function fromBase64Url(value: string): ArrayBuffer {
  const padding = "=".repeat((4 - (value.length % 4)) % 4);
  const base64 = (value + padding).replace(/-/g, "+").replace(/_/g, "/");
  const raw = window.atob(base64);
  const buffer = new ArrayBuffer(raw.length);
  const view = new Uint8Array(buffer);
  for (let index = 0; index < raw.length; index += 1) {
    view[index] = raw.charCodeAt(index);
  }
  return buffer;
}

export async function fetchPasskeyStatus(): Promise<PasskeyStatus> {
  const response = await fetch("/api/auth/passkey/status", {
    headers: { Accept: "application/json" },
  });
  if (!response.ok) {
    throw new Error("não foi possível consultar passkeys");
  }
  return (await response.json()) as PasskeyStatus;
}

async function publicPost<T>(path: string, body?: unknown): Promise<T> {
  const response = await fetch(path, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: body === undefined ? undefined : JSON.stringify(body),
  });
  const payload = (await response.json().catch(() => ({}))) as {
    message?: string;
  };
  if (!response.ok) {
    throw new Error(payload.message ?? `falha (${response.status})`);
  }
  return payload as T;
}

function decodeCreationOptions(options: JsonObject): PublicKeyCredentialCreationOptions {
  const publicKey = { ...(options.publicKey as JsonObject) };
  publicKey.challenge = fromBase64Url(publicKey.challenge as string);
  const user = { ...(publicKey.user as JsonObject) };
  user.id = fromBase64Url(user.id as string);
  publicKey.user = user;
  const exclude = publicKey.excludeCredentials as JsonObject[] | undefined;
  if (exclude) {
    publicKey.excludeCredentials = exclude.map((entry) => ({
      ...entry,
      id: fromBase64Url(entry.id as string),
    }));
  }
  return publicKey as unknown as PublicKeyCredentialCreationOptions;
}

function decodeRequestOptions(options: JsonObject): PublicKeyCredentialRequestOptions {
  const publicKey = { ...(options.publicKey as JsonObject) };
  publicKey.challenge = fromBase64Url(publicKey.challenge as string);
  const allow = publicKey.allowCredentials as JsonObject[] | undefined;
  if (allow) {
    publicKey.allowCredentials = allow.map((entry) => ({
      ...entry,
      id: fromBase64Url(entry.id as string),
    }));
  }
  return publicKey as unknown as PublicKeyCredentialRequestOptions;
}

function encodeRegistration(credential: PublicKeyCredential): JsonObject {
  const response = credential.response as AuthenticatorAttestationResponse;
  return {
    id: credential.id,
    rawId: toBase64Url(credential.rawId),
    type: credential.type,
    response: {
      clientDataJSON: toBase64Url(response.clientDataJSON),
      attestationObject: toBase64Url(response.attestationObject),
      transports: response.getTransports?.() ?? [],
    },
    authenticatorAttachment: credential.authenticatorAttachment ?? undefined,
    clientExtensionResults: credential.getClientExtensionResults(),
  };
}

function encodeAuthentication(credential: PublicKeyCredential): JsonObject {
  const response = credential.response as AuthenticatorAssertionResponse;
  return {
    id: credential.id,
    rawId: toBase64Url(credential.rawId),
    type: credential.type,
    response: {
      clientDataJSON: toBase64Url(response.clientDataJSON),
      authenticatorData: toBase64Url(response.authenticatorData),
      signature: toBase64Url(response.signature),
      userHandle: response.userHandle ? toBase64Url(response.userHandle) : null,
    },
    authenticatorAttachment: credential.authenticatorAttachment ?? undefined,
    clientExtensionResults: credential.getClientExtensionResults(),
  };
}

/** Registers a passkey for the already authenticated operator. */
export async function registerPasskey(label: string): Promise<PasskeyRecord> {
  if (!passkeySupported()) {
    throw new Error("Este navegador não suporta passkeys.");
  }
  const start = await apiPost<{ ceremony_id: string; options: JsonObject }>(
    "/api/auth/passkey/register/start",
  );
  const credential = (await navigator.credentials.create({
    publicKey: decodeCreationOptions(start.options),
  })) as PublicKeyCredential | null;
  if (!credential) throw new Error("Criação de passkey cancelada.");
  return apiPost<PasskeyRecord>("/api/auth/passkey/register/finish", {
    ceremony_id: start.ceremony_id,
    credential: encodeRegistration(credential),
    label,
  });
}

/** Passwordless browser login; stores the returned session token. */
export async function loginWithPasskey(): Promise<void> {
  if (!passkeySupported()) {
    throw new Error("Este navegador não suporta passkeys.");
  }
  const start = await publicPost<{ ceremony_id: string; options: JsonObject }>(
    "/api/auth/passkey/login/start",
  );
  const credential = (await navigator.credentials.get({
    publicKey: decodeRequestOptions(start.options),
  })) as PublicKeyCredential | null;
  if (!credential) throw new Error("Login com passkey cancelado.");
  const result = await publicPost<{ token: string }>(
    "/api/auth/passkey/login/finish",
    {
      ceremony_id: start.ceremony_id,
      credential: encodeAuthentication(credential),
    },
  );
  window.localStorage.setItem("opendesk_api_token", result.token);
  window.location.reload();
}

export async function listPasskeys(): Promise<PasskeyRecord[]> {
  try {
    return await apiGet<PasskeyRecord[]>("/api/auth/passkeys");
  } catch {
    return [];
  }
}

export async function deletePasskey(id: string): Promise<void> {
  await apiDelete(`/api/auth/passkeys/${encodeURIComponent(id)}`);
}
