/**
 * What a picture really is, read from its first bytes and its headers without decoding a single
 * pixel. Decoding is left to the maintainer's tools and the app, which do it with limits: here it
 * would cost far more CPU than a request on the Workers free plan has (10 ms), and a decoder is
 * exactly what a hostile file is aimed at.
 */

export type Format = "png" | "jpeg" | "webp";
/**
 * `lossless` is whether the picture keeps every pixel as it was made: a PNG always does, a JPEG
 * never, and a WebP when its picture is in a VP8L chunk, whether the file is the simple kind or
 * the extended one (VP8X). A lossy WebP's picture is in a "VP8 " chunk.
 */
export type Picture = { format: Format; width: number; height: number; lossless: boolean };

export class PictureError extends Error {}

const bad = (message: string) => new PictureError(message);

/**
 * The format and size of a PNG, JPEG or WebP picture, and whether it is lossless. Throws, in a
 * sentence, for anything else, for a header that doesn't hold together, and for an animated PNG or
 * WebP: a folder icon is one still picture.
 */
export function inspect(bytes: Uint8Array): Picture {
  if (starts(bytes, [0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a])) return png(bytes);
  if (starts(bytes, [0xff, 0xd8, 0xff])) return jpeg(bytes);
  if (ascii(bytes, 0, 4) === "RIFF" && ascii(bytes, 8, 4) === "WEBP") return webp(bytes);
  throw bad("isn't a PNG, JPEG or WebP picture");
}

/** The format a file name's extension promises. */
export function formatOf(file: string): Format | null {
  const ext = file.slice(file.lastIndexOf(".") + 1).toLowerCase();
  if (ext === "png") return "png";
  if (ext === "jpg" || ext === "jpeg") return "jpeg";
  if (ext === "webp") return "webp";
  return null;
}

export const CONTENT_TYPES: Record<Format, string> = { png: "image/png", jpeg: "image/jpeg", webp: "image/webp" };

function png(b: Uint8Array): Picture {
  // IHDR is always the first chunk: length 13, then the width and height.
  if (b.length < 33 || u32be(b, 8) !== 13 || ascii(b, 12, 4) !== "IHDR") throw bad("is a damaged PNG");
  const picture: Picture = { format: "png", width: u32be(b, 16), height: u32be(b, 20), lossless: true };
  // An animated PNG says so in an acTL chunk before its first IDAT. Walking chunk headers is a
  // handful of jumps; nothing is decompressed.
  let at = 8;
  for (let chunks = 0; at + 8 <= b.length && chunks < 10_000; chunks++) {
    const length = u32be(b, at);
    const type = ascii(b, at + 4, 4);
    if (type === "acTL") throw bad("is an animated PNG; a skin is one still picture");
    if (type === "IDAT" || type === "IEND") return picture;
    at += 12 + length;
  }
  throw bad("is a damaged PNG");
}

function jpeg(b: Uint8Array): Picture {
  let at = 2;
  while (at + 4 <= b.length) {
    if (b[at] !== 0xff) throw bad("is a damaged JPEG");
    const marker = b[at + 1];
    // Fill bytes, and markers that stand alone with no length after them.
    if (marker === 0xff) {
      at += 1;
      continue;
    }
    if (marker === 0x01 || (marker >= 0xd0 && marker <= 0xd7)) {
      at += 2;
      continue;
    }
    const length = u16be(b, at + 2);
    if (length < 2) throw bad("is a damaged JPEG");
    // SOF0 to SOF15 carry the size, except DHT (C4), JPG (C8) and DAC (CC), which share the range.
    if (marker >= 0xc0 && marker <= 0xcf && marker !== 0xc4 && marker !== 0xc8 && marker !== 0xcc) {
      if (at + 9 > b.length) break;
      return { format: "jpeg", height: u16be(b, at + 5), width: u16be(b, at + 7), lossless: false };
    }
    if (marker === 0xda || marker === 0xd9) break; // image data or the end, and still no size
    at += 2 + length;
  }
  throw bad("is a damaged JPEG");
}

function webp(b: Uint8Array): Picture {
  if (b.length < 25 || u32le(b, 4) + 8 > b.length) throw bad("is a damaged WebP");
  const first = ascii(b, 12, 4);
  let size: { width: number; height: number };
  if ((first === "VP8 " || first === "VP8X") && b.length < 30) throw bad("is a damaged WebP");
  if (first === "VP8 ") {
    // A lossy frame: a start code, then 14-bit width and height.
    if (b[23] !== 0x9d || b[24] !== 0x01 || b[25] !== 0x2a) throw bad("is a damaged WebP");
    size = { width: u16le(b, 26) & 0x3fff, height: u16le(b, 28) & 0x3fff };
  } else if (first === "VP8L") {
    // Lossless: a signature byte, then width - 1 and height - 1 in 14 bits each.
    if (b[20] !== 0x2f) throw bad("is a damaged WebP");
    const bits = u32le(b, 21);
    size = { width: (bits & 0x3fff) + 1, height: ((bits >>> 14) & 0x3fff) + 1 };
  } else if (first === "VP8X") {
    // Extended: flags (animation is bit 1), then the canvas size less one in 24 bits each.
    if (b[20] & 0x02) throw bad("is an animated WebP; a skin is one still picture");
    size = { width: u24le(b, 24) + 1, height: u24le(b, 27) + 1 };
  } else {
    throw bad("is a damaged WebP");
  }
  // The chunks' headers, a handful of jumps: frames without the flag would still be an
  // animation, which no still picture has, and the picture itself is in a VP8L chunk when it is
  // lossless (a signature byte first) and a "VP8 " chunk when it isn't.
  let lossless = false;
  let lossy = false;
  let at = 12;
  for (let chunks = 0; at + 8 <= b.length && chunks < 10_000; chunks++) {
    const type = ascii(b, at, 4);
    if (type === "ANIM" || type === "ANMF") throw bad("is an animated WebP; a skin is one still picture");
    if (type === "VP8L" && b[at + 8] === 0x2f) lossless = true;
    if (type === "VP8 ") lossy = true;
    const length = u32le(b, at + 4);
    at += 8 + length + (length & 1);
  }
  return { format: "webp", ...size, lossless: lossless && !lossy };
}

function starts(b: Uint8Array, prefix: number[]): boolean {
  return b.length >= prefix.length && prefix.every((v, i) => b[i] === v);
}

function ascii(b: Uint8Array, at: number, length: number): string {
  if (at + length > b.length) return "";
  return String.fromCharCode(...b.subarray(at, at + length));
}

const u16be = (b: Uint8Array, at: number) => (b[at] << 8) | b[at + 1];
const u16le = (b: Uint8Array, at: number) => b[at] | (b[at + 1] << 8);
const u24le = (b: Uint8Array, at: number) => b[at] | (b[at + 1] << 8) | (b[at + 2] << 16);
const u32be = (b: Uint8Array, at: number) => ((b[at] << 24) | (b[at + 1] << 16) | (b[at + 2] << 8) | b[at + 3]) >>> 0;
const u32le = (b: Uint8Array, at: number) => (b[at] | (b[at + 1] << 8) | (b[at + 2] << 16) | (b[at + 3] << 24)) >>> 0;
