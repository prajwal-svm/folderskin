/**
 * Where a request comes from, without keeping where it comes from. Quotas count per network (an
 * IPv4 /24 or an IPv6 /48, since one person easily has many addresses in either), and what is
 * stored is an HMAC of that prefix under a key made fresh each day from IP_SALT. The same network
 * counts as one within a day; across days, and to anyone reading the database, the rows can't be
 * tied together or back to an address.
 */
import { dayOf, hex, hmac, now } from "./bytes";
import type { Env } from "./env";
import { fail } from "./http";

/** The network an address belongs to: `203.0.113.0/24` or `2001:db8:1::/48`. */
export function networkOf(ip: string): string {
  const address = ip.trim();
  // An IPv4 address, or one mapped into IPv6 (::ffff:203.0.113.9).
  const v4 = address.match(/(\d{1,3})\.(\d{1,3})\.(\d{1,3})\.\d{1,3}$/);
  if (v4) return `${v4[1]}.${v4[2]}.${v4[3]}.0/24`;
  if (!address.includes(":")) return "unknown";
  const [head, tail = ""] = address.split("::");
  const front = head ? head.split(":") : [];
  const back = address.includes("::") ? (tail ? tail.split(":") : []) : [];
  const groups = [...front, ...Array(Math.max(0, 8 - front.length - back.length)).fill("0"), ...back];
  const first = groups.slice(0, 3).map((g) => (parseInt(g, 16) || 0).toString(16));
  return `${first.join(":")}::/48`;
}

/** The hashed network of `request`, as today's key sees it. */
export async function networkHash(env: Env, request: Request, at = now()): Promise<string> {
  if (!env.IP_SALT) throw fail(503, "not_configured", "FolderSkin's sharing service isn't set up yet. Please try again later.");
  const network = networkOf(request.headers.get("CF-Connecting-IP") ?? "");
  const daily = await hmac(env.IP_SALT, `network|${dayOf(at)}`);
  return hex(await hmac(daily, network)).slice(0, 32);
}
