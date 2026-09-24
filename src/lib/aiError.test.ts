import { describe, expect, it } from "vitest";
import { aiFailure, codeOf, worthRetrying } from "./aiError";

describe("aiFailure", () => {
  it("passes a structured error through, as a sentence", () => {
    expect(aiFailure({ code: "out_of_memory", message: "the graphics card ran out of memory", fix: ["Close other apps", 3], ask: "claude \"help\"" })).toEqual({
      code: "out_of_memory",
      message: "The graphics card ran out of memory.",
      fix: ["Close other apps"],
      ask: 'claude "help"',
    });
    expect(aiFailure({ code: "runtime_missing", what: "the runtime isn't installed", why: "setup was never run" }).message).toBe("The runtime isn't installed. Setup was never run.");
  });

  it("reads the code from a plain sentence's wording", () => {
    expect(aiFailure("add your openai API key first")).toEqual({ code: "missing_key", message: "Add your openai API key first." });
    expect(aiFailure(new Error("that key was rejected by OpenAI. Check it was copied whole and is still active.")).code).toBe("unauthorized");
    expect(codeOf("xAI is rate limiting you right now. Wait a moment and try again.")).toBe("rate_limited");
    expect(codeOf("OpenAI declined that prompt: safety system")).toBe("refused");
    expect(codeOf("couldn't reach Recraft: dns error")).toBe("network");
    expect(codeOf("couldn't reach OpenAI: the connection was refused")).toBe("network");
    expect(codeOf("BFL took too long to finish the image.")).toBe("timeout");
    expect(codeOf("the model drew a scene instead of a folder on a plain backdrop")).toBe("no_backdrop");
    expect(codeOf("something odd")).toBe("failed");
    expect(aiFailure(undefined)).toEqual({ code: "failed", message: "Something went wrong." });
  });

  it("knows when trying again can't help", () => {
    expect(worthRetrying("network")).toBe(true);
    expect(worthRetrying("refused")).toBe(false);
    expect(worthRetrying("missing_key")).toBe(false);
    expect(worthRetrying("no_build_for_platform")).toBe(false);
    expect(worthRetrying("out_of_memory")).toBe(true);
  });
});
