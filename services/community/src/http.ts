/**
 * Answers and errors. Every error the app can see is `{"error": {"code", "message"}}`, and the
 * message is a sentence it can show as it is.
 */

export class HttpError extends Error {
  constructor(
    readonly status: number,
    readonly code: string,
    message: string,
    readonly headers: Record<string, string> = {},
  ) {
    super(message);
  }
}

export const fail = (status: number, code: string, message: string, headers: Record<string, string> = {}) =>
  new HttpError(status, code, message, headers);

/** Headers every answer carries: nothing is cached on the way, and nothing may frame or sniff it. */
const COMMON: Record<string, string> = {
  "Cache-Control": "no-store",
  "X-Content-Type-Options": "nosniff",
  "Referrer-Policy": "no-referrer",
};

export function json(value: unknown, status = 200, headers: Record<string, string> = {}): Response {
  return new Response(JSON.stringify(value), {
    status,
    headers: { ...COMMON, "Content-Type": "application/json; charset=utf-8", ...headers },
  });
}

export function errorResponse(e: HttpError): Response {
  return json({ error: { code: e.code, message: e.message } }, e.status, e.headers);
}

/**
 * A page the service renders itself. The content security policy allows nothing the page doesn't
 * name: its own script, and Turnstile's where `extra` asks for it.
 */
export function html(body: string, status = 200, extra: { turnstile?: boolean } = {}): Response {
  const challenge = extra.turnstile ? " https://challenges.cloudflare.com" : "";
  const csp = [
    "default-src 'none'",
    `script-src 'self'${challenge}`,
    `frame-src${challenge || " 'none'"}`,
    "connect-src 'self'",
    "img-src 'self' data:",
    "style-src 'unsafe-inline'",
    "form-action 'self'",
    "base-uri 'none'",
    "frame-ancestors 'none'",
  ].join("; ");
  return new Response(body, {
    status,
    headers: {
      ...COMMON,
      "Content-Type": "text/html; charset=utf-8",
      "Content-Security-Policy": csp,
      "X-Frame-Options": "DENY",
      "X-Robots-Tag": "noindex, nofollow",
    },
  });
}

/** Text that goes into a page, with everything HTML could read as markup escaped. */
export function escapeHtml(text: string): string {
  return text.replace(/[&<>"']/g, (c) => `&#${c.charCodeAt(0)};`);
}

/**
 * The request's body, refusing anything over `max` bytes: by its Content-Length first, so a large
 * upload is turned away before it is read, and again as it arrives, for one that sends none.
 */
export async function readBody(request: Request, max: number): Promise<Uint8Array> {
  const declared = request.headers.get("Content-Length");
  if (declared !== null && Number(declared) > max) throw tooLarge(max);
  if (!request.body) return new Uint8Array(0);
  const reader = request.body.getReader();
  const chunks: Uint8Array[] = [];
  let size = 0;
  for (;;) {
    const { done, value } = await reader.read();
    if (done) break;
    size += value.byteLength;
    if (size > max) {
      await reader.cancel();
      throw tooLarge(max);
    }
    chunks.push(value);
  }
  if (chunks.length === 1) return chunks[0];
  const out = new Uint8Array(size);
  let at = 0;
  for (const chunk of chunks) {
    out.set(chunk, at);
    at += chunk.byteLength;
  }
  return out;
}

const tooLarge = (max: number) =>
  fail(413, "too_large", `That is bigger than the ${Math.round(max / 1024)} KB this accepts.`);

/** A JSON body of at most `max` bytes, as an object; anything else is a 400 in a sentence. */
export function parseJson(bytes: Uint8Array): Record<string, unknown> {
  try {
    const value: unknown = JSON.parse(new TextDecoder().decode(bytes));
    if (value && typeof value === "object" && !Array.isArray(value)) return value as Record<string, unknown>;
  } catch {
    // Falls through to the same answer as JSON that isn't an object.
  }
  throw fail(400, "bad_json", "The request wasn't JSON FolderSkin could read.");
}
