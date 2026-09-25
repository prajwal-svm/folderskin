import { afterEach, describe, expect, it } from "vitest";
import { formatBytes, formatDate, formatDuration, formatList, formatNumber } from "./format";
import { getLocale, i18n } from "./index";

const sept25 = new Date(Date.UTC(2026, 8, 25, 12));

/** The spaces `Intl` writes (no-break, narrow no-break) as plain ones, which differ by ICU version. */
const plain = (s: string) => s.replace(/[\u00a0\u202f]/g, " ");

describe("numbers, sizes, lengths of time, dates and lists", () => {
  afterEach(async () => {
    await i18n.setLocale("en");
  });

  it("are written the British way in English", () => {
    expect(getLocale()).toBe("en");
    expect(formatNumber(1234.5)).toBe("1,234.5");
    expect(formatBytes(1)).toBe("1 byte");
    expect(formatBytes(812)).toBe("812 bytes");
    expect(formatBytes(3_600_000)).toBe("3.6 MB");
    expect(formatBytes(88_000_000)).toBe("88 MB");
    expect(formatBytes(1_200_000_000)).toBe("1.2 GB");
    expect(formatDuration(46)).toBe("46 seconds");
    expect(formatDuration(1)).toBe("1 second");
    expect(formatDuration(170)).toBe("3 minutes");
    expect(formatDate(sept25, { dateStyle: "medium", timeZone: "UTC" })).toBe("25 Sept 2026");
    expect(formatList(["Trips", "Work", "Photos"])).toBe("Trips, Work and Photos");
    expect(formatList(["a", "b"], "disjunction")).toBe("a or b");
  });

  it("follow the language on show", async () => {
    await i18n.setLocale("fr");
    expect(plain(formatNumber(1234.5))).toBe("1 234,5");
    expect(plain(formatBytes(3_600_000))).toBe("3,6 MB");
    expect(plain(formatDuration(46))).toBe("46 secondes");
    expect(formatList(["Voyages", "Travail", "Photos"])).toBe("Voyages, Travail et Photos");

    await i18n.setLocale("ja");
    expect(plain(formatDuration(170))).toMatch(/^3 ?分$/);
    expect(formatDate(sept25, { dateStyle: "medium", timeZone: "UTC" })).toBe("2026/09/25");
    expect(formatList(["旅行", "仕事", "写真"])).toBe("旅行、仕事、写真");

    await i18n.setLocale("es");
    expect(formatNumber(12345)).toBe("12.345");
  });
});
