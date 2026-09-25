import { describe, expect, it } from "vitest";
import { canWithdraw, handleFrom, isHandle, shareProgressLabel, statusLabel } from "./share";

describe("a handle", () => {
  it("has the shape of a GitHub user name, 3 to 39 characters", () => {
    for (const good of ["sunny-otter", "abc", "A1-b2", "x".repeat(39)]) expect(isHandle(good), good).toBe(true);
    for (const bad of ["ab", "-otter", "otter-", "two--dashes", "with space", "émile", "x".repeat(40)]) expect(isHandle(bad), bad).toBe(false);
  });

  it("is made from what someone types as they type it", () => {
    expect(handleFrom("Sunny Otter!")).toBe("Sunny-Otter-");
    expect(handleFrom("  Émile   Zola ")).toBe("Emile-Zola-");
    expect(handleFrom("---lead")).toBe("lead");
    expect(handleFrom("x".repeat(50))).toHaveLength(39);
    // The trailing dash left while typing is what isHandle turns down, so the button says why.
    expect(isHandle(handleFrom("Sunny Otter!"))).toBe(false);
  });
});

describe("the dialog's words", () => {
  it("count the pictures as they go", () => {
    expect(shareProgressLabel({ stage: "uploading", done: 0, total: 12 })).toBe("Sending pictures (1 of 12)");
    expect(shareProgressLabel({ stage: "uploading", done: 12, total: 12 })).toBe("Sending pictures (12 of 12)");
    expect(shareProgressLabel({ stage: "finishing" })).toBe("Putting it in the review queue");
  });

  it("say when a request is tried again, and how soon", () => {
    expect(shareProgressLabel({ stage: "waiting", seconds: 60 })).toBe("Trying again in 60 seconds");
    expect(shareProgressLabel({ stage: "waiting", seconds: 1 })).toBe("Trying again");
    expect(shareProgressLabel({ stage: "waiting", seconds: 0 })).toBe("Trying again");
  });

  it("say where each submission is, and which can still be taken back", () => {
    expect(statusLabel("in_review")).toEqual({ label: "Waiting for review", tone: "accent" });
    expect(statusLabel("rejected").tone).toBe("danger");
    expect(canWithdraw("approved")).toBe(true);
    expect(canWithdraw("rejected")).toBe(false);
  });
});
