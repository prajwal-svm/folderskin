/**
 * Daily quotas and the switches that stop uploads altogether. Counters live in D1, one row per day,
 * scope and id, and each is taken with a single statement that only adds when the total stays
 * within the limit, so two requests at once can't both squeeze past it.
 */
import { dayOf, now } from "./bytes";
import type { Env } from "./env";
import { fail, HttpError } from "./http";
import { DEFAULT_GLOBAL_DAILY_PICTURES, DEFAULT_MAX_WAITING } from "./limits";

export type Take = {
  scope: string;
  id: string;
  amount: number;
  limit: number;
  /** What to answer when this one is over. */
  error: HttpError;
};

/** Adds `amount` to a counter if it stays within `limit`; false, changing nothing, if it wouldn't. */
export async function take(env: Env, day: string, scope: string, id: string, amount: number, limit: number): Promise<boolean> {
  if (amount > limit) return false;
  const result = await env.DB.prepare(
    `INSERT INTO counters (day, scope, id, n) VALUES (?1, ?2, ?3, ?4)
     ON CONFLICT (day, scope, id) DO UPDATE SET n = n + excluded.n WHERE n + excluded.n <= ?5`,
  )
    .bind(day, scope, id, amount, limit)
    .run();
  return result.meta.changes === 1;
}

export async function giveBack(env: Env, day: string, scope: string, id: string, amount: number): Promise<void> {
  await env.DB.prepare("UPDATE counters SET n = MAX(0, n - ?4) WHERE day = ?1 AND scope = ?2 AND id = ?3")
    .bind(day, scope, id, amount)
    .run();
}

/** Takes every quota or none: the first that is over gives the earlier ones back and throws its error. */
export async function takeAll(env: Env, takes: Take[], at = now()): Promise<void> {
  const day = dayOf(at);
  const taken: Take[] = [];
  for (const t of takes) {
    if (!(await take(env, day, t.scope, t.id, t.amount, t.limit))) {
      for (const done of taken) await giveBack(env, day, done.scope, done.id, done.amount);
      throw t.error;
    }
    taken.push(t);
  }
}

export async function used(env: Env, scope: string, id: string, at = now()): Promise<number> {
  const row = await env.DB.prepare("SELECT n FROM counters WHERE day = ?1 AND scope = ?2 AND id = ?3")
    .bind(dayOf(at), scope, id)
    .first<{ n: number }>();
  return row?.n ?? 0;
}

const number = (value: string | undefined, fallback: number) => {
  const n = Number(value);
  return Number.isFinite(n) && n >= 0 && value !== undefined && value !== "" ? n : fallback;
};

export const globalDailyPictures = (env: Env) => number(env.GLOBAL_DAILY_PICTURES, DEFAULT_GLOBAL_DAILY_PICTURES);
export const maxWaiting = (env: Env) => number(env.MAX_WAITING, DEFAULT_MAX_WAITING);
export const envNumber = number;

/** The "review queue is full" answer: the same words whether the day's pictures or the queue ran out. */
export const queueFull = () =>
  fail(503, "queue_full", "The review queue is full for today. Please try again tomorrow.", { "Retry-After": "3600" });

export type Pause = { paused: boolean; message: string };

/** Whether uploads are paused: by PAUSED=1 in the dashboard (which needs no database), or by the maintainer's switch. */
export async function pauseState(env: Env): Promise<Pause> {
  if (env.PAUSED === "1") return { paused: true, message: "" };
  const row = await env.DB.prepare("SELECT value FROM settings WHERE name = 'pause'").first<{ value: string }>();
  if (!row) return { paused: false, message: "" };
  try {
    const value = JSON.parse(row.value) as Partial<Pause>;
    return { paused: value.paused === true, message: typeof value.message === "string" ? value.message : "" };
  } catch {
    return { paused: false, message: "" };
  }
}

export async function setPause(env: Env, pause: Pause): Promise<void> {
  await env.DB.prepare(
    "INSERT INTO settings (name, value) VALUES ('pause', ?1) ON CONFLICT (name) DO UPDATE SET value = excluded.value",
  )
    .bind(JSON.stringify(pause))
    .run();
}

/** Turns away new work while sharing is paused, saying why when the maintainer gave a reason. */
export async function requireAccepting(env: Env): Promise<void> {
  const pause = await pauseState(env);
  if (pause.paused) {
    const why = pause.message ? ` ${pause.message}` : "";
    throw fail(503, "paused", `Sharing without GitHub is paused for now.${why} Please try again later.`, { "Retry-After": "3600" });
  }
}
