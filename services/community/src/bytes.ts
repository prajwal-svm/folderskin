/** Encodings the protocol uses: base64url without padding for keys and signatures, hex for digests. */

export function b64url(bytes: Uint8Array): string {
  let s = "";
  for (let i = 0; i < bytes.length; i += 0x8000) s += String.fromCharCode(...bytes.subarray(i, i + 0x8000));
  return btoa(s).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");
}

/** Decodes base64url without padding; `null` for anything else, including standard base64. */
export function fromB64url(text: string): Uint8Array | null {
  if (!/^[A-Za-z0-9_-]*$/.test(text) || text.length % 4 === 1) return null;
  const standard = text.replace(/-/g, "+").replace(/_/g, "/");
  try {
    const binary = atob(standard + "=".repeat((4 - (standard.length % 4)) % 4));
    const out = new Uint8Array(binary.length);
    for (let i = 0; i < binary.length; i++) out[i] = binary.charCodeAt(i);
    return out;
  } catch {
    return null;
  }
}

/** Standard base64 with padding, which data URLs use. */
export function b64(bytes: Uint8Array): string {
  let s = "";
  for (let i = 0; i < bytes.length; i += 0x8000) s += String.fromCharCode(...bytes.subarray(i, i + 0x8000));
  return btoa(s);
}

export function hex(bytes: Uint8Array): string {
  let s = "";
  for (const b of bytes) s += b.toString(16).padStart(2, "0");
  return s;
}

export async function sha256(data: Uint8Array | string): Promise<Uint8Array> {
  const bytes = typeof data === "string" ? new TextEncoder().encode(data) : data;
  return new Uint8Array(await crypto.subtle.digest("SHA-256", bytes));
}

export async function sha256Hex(data: Uint8Array | string): Promise<string> {
  return hex(await sha256(data));
}

/** A random id of `bytes` random bytes, as lower-case base32 so it reads well in a URL or a log. */
export function randomId(prefix: string, bytes = 12): string {
  const alphabet = "abcdefghijklmnopqrstuvwxyz234567";
  const random = crypto.getRandomValues(new Uint8Array(bytes));
  let bits = 0;
  let value = 0;
  let out = "";
  for (const b of random) {
    value = (value << 8) | b;
    bits += 8;
    while (bits >= 5) {
      out += alphabet[(value >>> (bits - 5)) & 31];
      bits -= 5;
    }
  }
  if (bits > 0) out += alphabet[(value << (5 - bits)) & 31];
  return `${prefix}${out}`;
}

async function hmacKey(secret: string | Uint8Array): Promise<CryptoKey> {
  const raw = typeof secret === "string" ? new TextEncoder().encode(secret) : secret;
  return crypto.subtle.importKey("raw", raw, { name: "HMAC", hash: "SHA-256" }, false, ["sign", "verify"]);
}

export async function hmac(secret: string | Uint8Array, message: string): Promise<Uint8Array> {
  const key = await hmacKey(secret);
  return new Uint8Array(await crypto.subtle.sign("HMAC", key, new TextEncoder().encode(message)));
}

/** Checks an HMAC in constant time, which a plain comparison of the two tags wouldn't be. */
export async function hmacVerify(secret: string, message: string, tag: Uint8Array): Promise<boolean> {
  const key = await hmacKey(secret);
  return crypto.subtle.verify("HMAC", key, tag, new TextEncoder().encode(message));
}

/** Unix seconds. */
export const now = () => Math.floor(Date.now() / 1000);

/** The UTC day a time falls on, as `2026-09-23`: what every daily quota counts by. */
export const dayOf = (seconds: number) => new Date(seconds * 1000).toISOString().slice(0, 10);
