import { describe, expect, it } from "vitest";
import { formatOf, inspect } from "../src/images";
import { jpeg, png, webpExtended, webpLossless, webpLossy } from "./helpers";

describe("a picture's headers", () => {
  it("give the size of a PNG, a JPEG and every kind of WebP", () => {
    expect(inspect(png(512, 300))).toMatchObject({ format: "png", width: 512, height: 300 });
    expect(inspect(jpeg(1024, 768))).toMatchObject({ format: "jpeg", width: 1024, height: 768 });
    expect(inspect(webpExtended(640, 480))).toMatchObject({ format: "webp", width: 640, height: 480 });
    expect(inspect(webpLossless(256, 1000))).toMatchObject({ format: "webp", width: 256, height: 1000 });
    expect(inspect(webpLossy(300, 700))).toMatchObject({ format: "webp", width: 300, height: 700 });
  });

  it("say whether a picture is lossless, from its headers alone", () => {
    const lossless = [
      ["a PNG", png(512, 512)],
      ["a simple lossless WebP (VP8L)", webpLossless(512, 512)],
      ["an extended WebP holding a VP8L picture", webpExtended(512, 512)],
    ] as const;
    for (const [what, bytes] of lossless) expect(inspect(bytes).lossless, what).toBe(true);
    const lossy = [
      ["a JPEG", jpeg(512, 512)],
      ["a simple lossy WebP (VP8)", webpLossy(512, 512)],
      ["an extended WebP holding a lossy picture with alpha", webpExtended(512, 512, { image: "VP8 " })],
      ["an extended WebP with no picture in it", webpExtended(512, 512, { image: "none" })],
    ] as const;
    for (const [what, bytes] of lossy) expect(inspect(bytes).lossless, what).toBe(false);
    // An animation is turned away whatever its frames are.
    expect(() => inspect(webpExtended(512, 512, { animated: true }))).toThrow(/animated WebP/);
  });

  it("turn away animation, which a folder icon can't show", () => {
    expect(() => inspect(png(512, 512, { animated: true }))).toThrow(/animated PNG/);
    expect(() => inspect(webpExtended(512, 512, { animated: true }))).toThrow(/animated WebP/);
  });

  it("turn away anything that isn't a PNG, JPEG or WebP, whatever it is called", () => {
    const gif = new Uint8Array([...new TextEncoder().encode("GIF89a"), 1, 0, 1, 0]);
    expect(() => inspect(gif)).toThrow(/isn't a PNG, JPEG or WebP/);
    expect(() => inspect(new TextEncoder().encode("<svg xmlns='http://www.w3.org/2000/svg'/>"))).toThrow();
    expect(() => inspect(new Uint8Array(0))).toThrow();
  });

  it("turn away headers that don't hold together", () => {
    const cut = png(512, 512).slice(0, 20);
    expect(() => inspect(cut)).toThrow(/damaged PNG/);
    const noSize = new Uint8Array([0xff, 0xd8, 0xff, 0xd9]);
    expect(() => inspect(noSize)).toThrow(/damaged JPEG/);
    const lying = webpExtended(512, 512);
    lying[4] = 0xff; // a RIFF size far past the end of the file
    lying[5] = 0xff;
    expect(() => inspect(lying)).toThrow(/damaged WebP/);
  });

  it("match file names to the format they promise", () => {
    expect(formatOf("koi.PNG")).toBe("png");
    expect(formatOf("koi.jpeg")).toBe("jpeg");
    expect(formatOf("koi.jpg")).toBe("jpeg");
    expect(formatOf("koi.webp")).toBe("webp");
    expect(formatOf("koi.gif")).toBeNull();
  });
});
