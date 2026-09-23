import { describe, expect, it, vi } from "vitest";
import type { Vision } from "../src/env";
import { networkOf } from "../src/ip";
import { noticeText, webhookRequest } from "../src/notify";
import { readVerdict, triage } from "../src/triage";
import { jpeg, testEnv } from "./helpers";

describe("the triage model's answer", () => {
  it("adds nothing for a safe sheet", () => {
    expect(readVerdict({ response: '{"safe": true}' }, 0)).toEqual([]);
    expect(readVerdict({ response: { safe: true } }, 0)).toEqual([]);
  });

  it("flags what it names, the worst of it as urgent", () => {
    expect(readVerdict({ response: 'Sure. {"safe": false, "categories": ["sexual", "text", "made-up"]}' }, 1)).toEqual([
      { code: "ai:sexual", severity: "high", detail: "sheet 2" },
      { code: "ai:text", severity: "normal", detail: "sheet 2" },
    ]);
  });

  it("flags an answer it can't read rather than taking it as a pass", () => {
    expect(readVerdict({ response: "I cannot help with that." }, 0)).toEqual([
      { code: "ai:unclear", severity: "normal", detail: "sheet 1: the model's answer couldn't be read" },
    ]);
    expect(readVerdict(null, 0)[0].code).toBe("ai:unclear");
  });
});

describe("triage", () => {
  const sheet = jpeg(768, 768);

  it("looks at every sheet within the day's budget", async () => {
    const run = vi.fn<Vision["run"]>().mockResolvedValue({ response: '{"safe": false, "categories": ["hate"]}' });
    const at = 2_000_000_000;
    const result = await triage(testEnv({ AI: { run }, AI_NEURONS_PER_SHEET: "10", AI_DAILY_NEURONS: "1000" }), [sheet, sheet], at);
    expect(result).toEqual({
      looked: true,
      flags: [
        { code: "ai:hate", severity: "high", detail: "sheet 1" },
        { code: "ai:hate", severity: "high", detail: "sheet 2" },
      ],
    });
    const [model, inputs] = run.mock.calls[0];
    expect(model).toBe("@cf/meta/llama-4-scout-17b-16e-instruct");
    expect(JSON.stringify(inputs)).toContain("data:image/jpeg;base64,");
  });

  it("stops once the day's neurons are spent, and leaves the rest to a person", async () => {
    const run = vi.fn<Vision["run"]>().mockResolvedValue({ response: '{"safe": true}' });
    const at = 2_000_086_400;
    const env = testEnv({ AI: { run }, AI_NEURONS_PER_SHEET: "60", AI_DAILY_NEURONS: "100" });
    expect(await triage(env, [sheet, sheet], at)).toEqual({ looked: false, flags: [] });
    expect(run).toHaveBeenCalledTimes(1);
  });

  it("never fails a submission when the model does", async () => {
    const run = vi.fn<Vision["run"]>().mockRejectedValue(new Error("capacity"));
    vi.spyOn(console, "error").mockImplementation(() => {});
    expect(await triage(testEnv({ AI: { run } }), [sheet], 2_000_172_800)).toEqual({ looked: false, flags: [] });
  });

  it("does nothing without the binding", async () => {
    expect(await triage(testEnv(), [sheet])).toEqual({ looked: false, flags: [] });
  });
});

describe("notifications", () => {
  const notice = { title: "Urgent: \"Pack\" was flagged", lines: ["one", "two"], links: [{ label: "Review it", url: "https://community.test/l/x" }] };

  it("read as plain text with the links at the end", () => {
    expect(noticeText(notice)).toBe('Urgent: "Pack" was flagged\n\none\ntwo\n\nReview it: https://community.test/l/x');
  });

  it("take the shape each webhook expects, and mention nobody on Discord", () => {
    const discord = JSON.parse(String(webhookRequest("discord", notice, true).body));
    expect(discord).toEqual({ content: noticeText(notice), allowed_mentions: { parse: [] } });
    const telegram = JSON.parse(String(webhookRequest("telegram", notice, true).body));
    expect(telegram).toMatchObject({ disable_web_page_preview: true });
    const ntfy = webhookRequest("ntfy", { ...notice, title: "Café ☕ report" }, false);
    expect(ntfy.headers).toMatchObject({ Title: "Caf  report", Priority: "3", Click: "https://community.test/l/x" });
  });
});

describe("networks", () => {
  it("are counted by /24 and /48, never by the address itself", () => {
    expect(networkOf("203.0.113.77")).toBe("203.0.113.0/24");
    expect(networkOf("::ffff:203.0.113.9")).toBe("203.0.113.0/24");
    expect(networkOf("2001:db8:1:2:3:4:5:6")).toBe("2001:db8:1::/48");
    expect(networkOf("2001:db8::1")).toBe("2001:db8:0::/48");
    expect(networkOf("")).toBe("unknown");
  });
});
