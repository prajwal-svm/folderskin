/**
 * The raw body `composer_save` and `composer_preview` take: a picture is too big to send as
 * JSON, so it goes as bytes after a small JSON header, `[u32 little-endian header length]
 * [header, UTF-8][PNG]`. The Rust side reads it back in `composer.rs`.
 */

export function frame(header: unknown, png: Uint8Array): Uint8Array {
  const head = new TextEncoder().encode(JSON.stringify(header));
  const out = new Uint8Array(4 + head.length + png.length);
  new DataView(out.buffer).setUint32(0, head.length, true);
  out.set(head, 4);
  out.set(png, 4 + head.length);
  return out;
}

/** The header and picture of a framed body, for the browser preview's stand-ins and the tests. */
export function unframe(body: Uint8Array): { header: unknown; png: Uint8Array } {
  if (body.length < 4) throw new Error("that design didn't arrive whole");
  const n = new DataView(body.buffer, body.byteOffset, body.byteLength).getUint32(0, true);
  if (4 + n > body.length) throw new Error("that design didn't arrive whole");
  const header = JSON.parse(new TextDecoder().decode(body.subarray(4, 4 + n))) as unknown;
  return { header, png: body.subarray(4 + n) };
}

/** A canvas as PNG bytes. */
export function canvasPng(canvas: HTMLCanvasElement): Promise<Uint8Array> {
  return new Promise((resolve, reject) => {
    canvas.toBlob((blob) => {
      if (!blob) return reject(new Error("couldn't draw the design"));
      // Blob.arrayBuffer is Safari 14+; FileReader works everywhere the app runs.
      const reader = new FileReader();
      reader.onload = () => resolve(new Uint8Array(reader.result as ArrayBuffer));
      reader.onerror = () => reject(new Error("couldn't draw the design"));
      reader.readAsArrayBuffer(blob);
    }, "image/png");
  });
}
