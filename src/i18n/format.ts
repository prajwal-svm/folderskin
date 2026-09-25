/**
 * Numbers, sizes, lengths of time, dates and lists written the way the language on show writes
 * them, through `Intl`. Each reads the language when it's called, like `t`.
 */
import { formatNumberIn } from "./core";
import { getLocale, INTL_LOCALES, t } from "./index";

const intl = () => INTL_LOCALES[getLocale()];

/** 1,234 in English, 1 234 in French, 1.234 in Spanish. */
export function formatNumber(value: number, options?: Intl.NumberFormatOptions): string {
  return options ? new Intl.NumberFormat(intl(), options).format(value) : formatNumberIn(intl(), value);
}

const UNITS = ["kb", "mb", "gb", "tb"] as const;

/**
 * A byte count people can read, counting in thousands as disks do: "812 bytes", "3.6 MB", "88 MB",
 * "1.2 GB". One decimal under ten, none from ten up.
 */
export function formatBytes(bytes: number): string {
  if (bytes < 1000) return t("common.size.bytes", { count: Math.max(0, Math.round(bytes)) });
  let value = bytes / 1000;
  let i = 0;
  while (value >= 1000 && i < UNITS.length - 1) {
    value /= 1000;
    i++;
  }
  const digits = value >= 10 ? 0 : 1;
  const shown = formatNumber(value, { maximumFractionDigits: digits, minimumFractionDigits: 0 });
  return t(`common.size.${UNITS[i]}`, { value: shown });
}

/** "46 seconds", or "3 minutes" once it's a minute or more, in whole units. */
export function formatDuration(seconds: number): string {
  const minutes = seconds >= 60;
  const value = Math.round(minutes ? seconds / 60 : seconds);
  return new Intl.NumberFormat(intl(), { style: "unit", unit: minutes ? "minute" : "second", unitDisplay: "long" }).format(value);
}

/** A date, a time or both, as `Intl.DateTimeFormat` writes them with `options`. */
export function formatDate(at: number | Date, options: Intl.DateTimeFormatOptions): string {
  return new Intl.DateTimeFormat(intl(), options).format(at);
}

/** "A, B and C" in English, "A、B、C" in Japanese. */
export function formatList(items: readonly string[], type: Intl.ListFormatType = "conjunction"): string {
  return new Intl.ListFormat(intl(), { style: "long", type }).format(items);
}
