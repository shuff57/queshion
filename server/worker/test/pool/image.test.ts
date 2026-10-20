// The server's image rules (docs/shared-pool.md "Images"): one valid and one invalid file per rule.
// Run: npm test
import assert from "node:assert/strict";
import { test } from "node:test";
import { checkImage } from "../../src/pool/image.ts";
import { chunk, concat, enc, png } from "./harness.ts";

const refused = async (b: Uint8Array) => {
  const r = await checkImage(b);
  return "refused" in r ? r.refused : null;
};

test("PNG: the chunk allow-list, CRCs, IHDR first, IEND last, nothing after, sides 1..4096", async () => {
  assert.deepEqual(await checkImage(png(10, 20)), { contentType: "image/png", width: 10, height: 20 });
  for (const t of ["tEXt", "zTXt", "iTXt", "eXIf", "iCCP", "tIME", "acTL", "fcTL", "abcd"]) assert.equal(await refused(png(4, 4, 0, [chunk(t, enc("x"))])), "png_chunk", t);
  assert.equal(await refused(png(4, 4, 0, [chunk("gAMA", new Uint8Array(4)), chunk("pHYs", new Uint8Array(9))])), null, "allowed ancillaries");
  assert.equal(await refused(concat(png(4, 4), new Uint8Array([0]))), "trailing_bytes");
  const bad = png(4, 4);
  bad[bad.length - 13] ^= 1; // a byte inside the IDAT data: its CRC no longer matches
  assert.equal(await refused(bad), "png_crc");
  assert.equal(await refused(png(4097, 1)), "dimensions");
  assert.equal(await refused(png(1, 0)), "dimensions");
  assert.equal(await refused(png(4, 4).subarray(0, 40)), "truncated");
  const noIend = png(4, 4).subarray(0, png(4, 4).length - 12);
  assert.equal(await refused(noIend), "truncated");
});

// ---- JPEG fixtures -----------------------------------------------------------------------------------
const seg = (m: number, data: Uint8Array) => concat(new Uint8Array([0xff, m, (data.length + 2) >> 8, (data.length + 2) & 255]), data);
function jpeg(w: number, h: number, extra: Uint8Array[] = [], tail = new Uint8Array(0)): Uint8Array {
  const sof = new Uint8Array([8, h >> 8, h & 255, w >> 8, w & 255, 3, 1, 0x22, 0, 2, 0x11, 1, 3, 0x11, 1]);
  return concat(
    new Uint8Array([0xff, 0xd8]),
    seg(0xe0, concat(enc("JFIF\0"), new Uint8Array([1, 1, 0, 0, 1, 0, 1, 0, 0]))),
    ...extra,
    seg(0xdb, new Uint8Array(65)),
    seg(0xc0, sof),
    seg(0xc4, new Uint8Array(17)), // one Huffman table with no codes
    seg(0xda, new Uint8Array([3, 1, 0, 2, 0x11, 3, 0x11, 0, 63, 0])),
    new Uint8Array([0x12, 0xff, 0x00, 0x34, 0xff, 0xd0, 0x56]), // entropy data with a stuffed byte and a restart marker
    new Uint8Array([0xff, 0xd9]),
    tail,
  );
}

test("JPEG: only SOI, APP0 (JFIF), APP14 (Adobe), DQT, DHT, DRI, SOF0-2, SOS, RSTn, EOI; nothing after EOI", async () => {
  assert.deepEqual(await checkImage(jpeg(640, 480)), { contentType: "image/jpeg", width: 640, height: 480 });
  assert.equal(await refused(jpeg(8, 8, [seg(0xee, concat(enc("Adobe"), new Uint8Array(7)))])), null);
  assert.equal(await refused(jpeg(8, 8, [seg(0xe1, concat(enc("Exif\0\0"), new Uint8Array(10)))])), "jpeg_segment", "EXIF");
  assert.equal(await refused(jpeg(8, 8, [seg(0xe2, enc("ICC_PROFILE\0"))])), "jpeg_segment", "ICC");
  assert.equal(await refused(jpeg(8, 8, [seg(0xed, enc("Photoshop 3.0\0"))])), "jpeg_segment", "IPTC");
  assert.equal(await refused(jpeg(8, 8, [seg(0xfe, enc("a comment"))])), "jpeg_segment", "COM");
  assert.equal(await refused(jpeg(8, 8, [seg(0xe5, enc("x"))])), "jpeg_segment", "APP5");
  assert.equal(await refused(jpeg(8, 8, [], enc("PK"))), "trailing_bytes");
  assert.equal(await refused(jpeg(5000, 8)), "dimensions");
  assert.equal(await refused(jpeg(8, 0)), "dimensions", "DNL-deferred height");
  const twoSof = jpeg(8, 8, [seg(0xc0, new Uint8Array([8, 0, 8, 0, 8, 1, 1, 0x11, 0]))]);
  assert.equal(await refused(twoSof), "jpeg_sof");
});

// ---- GIF fixtures ------------------------------------------------------------------------------------
function gif(o: { w?: number; h?: number; frames?: number; ext?: Uint8Array[]; frameW?: number; tail?: Uint8Array } = {}): Uint8Array {
  const W = o.w ?? 10;
  const H = o.h ?? 10;
  const head = concat(enc("GIF89a"), new Uint8Array([W & 255, W >> 8, H & 255, H >> 8, 0x80, 0, 0]), new Uint8Array(6));
  const frame = () => {
    const fw = o.frameW ?? W;
    return concat(new Uint8Array([0x2c, 0, 0, 0, 0, fw & 255, fw >> 8, H & 255, H >> 8, 0]), new Uint8Array([2]), lzwEncode(2, fw * H));
  };
  return concat(head, ...(o.ext ?? []), ...Array.from({ length: o.frames ?? 1 }, frame), new Uint8Array([0x3b]), o.tail ?? new Uint8Array(0));
}
const gce = new Uint8Array([0x21, 0xf9, 4, 1, 0, 0, 0, 0]);

/** A valid GIF LZW stream of `pixels` zero pixels (literal codes only), in sub-blocks. */
function lzwEncode(min: number, pixels: number, extra: number[] = []): Uint8Array {
  const clear = 1 << min;
  const eoi = clear + 1;
  const codes: [number, number][] = [];
  let size = min + 1;
  let next = eoi + 1;
  let first = true;
  codes.push([clear, size]);
  for (let p = 0; p < pixels; p++) {
    codes.push([0, size]);
    if (!first) {
      next++;
      if (next === 1 << size && size < 12) size++;
      if (next >= 4095) {
        codes.push([clear, size]);
        size = min + 1;
        next = eoi + 1;
        first = true;
        continue;
      }
    }
    first = false;
  }
  codes.push([eoi, size]);
  for (const x of extra) codes.push([x, size]);
  const bytes: number[] = [];
  let acc = 0;
  let nb = 0;
  for (const [c, n] of codes) {
    acc |= c << nb;
    nb += n;
    while (nb >= 8) {
      bytes.push(acc & 255);
      acc >>>= 8;
      nb -= 8;
    }
  }
  if (nb > 0) bytes.push(acc & 255);
  const out: number[] = [];
  for (let k = 0; k < bytes.length; k += 255) {
    const part = bytes.slice(k, k + 255);
    out.push(part.length, ...part);
  }
  out.push(0);
  return new Uint8Array(out);
}

test("GIF: one image, at most one graphic control extension before it, no comment/application/plain-text, trailer last", async () => {
  assert.deepEqual(await checkImage(gif()), { contentType: "image/gif", width: 10, height: 10 });
  assert.equal(await refused(gif({ ext: [gce] })), null);
  assert.equal(await refused(gif({ ext: [gce, gce] })), "gif_extension");
  assert.equal(await refused(gif({ frames: 2 })), "gif_animated");
  assert.equal(await refused(gif({ ext: [concat(new Uint8Array([0x21, 0xff, 11]), enc("NETSCAPE2.0"), new Uint8Array([3, 1, 0, 0, 0]))] })), "gif_extension", "looping");
  assert.equal(await refused(gif({ ext: [concat(new Uint8Array([0x21, 0xfe, 3]), enc("hey"), new Uint8Array([0]))] })), "gif_extension", "comment");
  assert.equal(await refused(gif({ ext: [new Uint8Array([0x21, 0x01, 1, 0, 0])] })), "gif_extension", "plain text");
  assert.equal(await refused(gif({ frameW: 11 })), "gif_frame", "frame outside the logical screen");
  assert.equal(await refused(gif({ tail: new Uint8Array([0]) })), "trailing_bytes");
  assert.equal(await refused(gif({ w: 4097 })), "dimensions");
  // Data hidden after the end-of-information code, or too few pixels, is refused (LZW decoded).
  const head = concat(enc("GIF89a"), new Uint8Array([10, 0, 10, 0, 0x80, 0, 0]), new Uint8Array(6), new Uint8Array([0x2c, 0, 0, 0, 0, 10, 0, 10, 0, 0, 2]));
  assert.equal(await refused(concat(head, lzwEncode(2, 100), new Uint8Array([0x3b]))), null);
  assert.equal(await refused(concat(head, lzwEncode(2, 99), new Uint8Array([0x3b]))), "gif_lzw_data");
  assert.equal(await refused(concat(head, lzwEncode(2, 100, [0, 0, 0, 0, 0, 0, 0, 0]), new Uint8Array([0x3b]))), "gif_lzw_data");
});

// ---- WebP fixtures -----------------------------------------------------------------------------------
function riff(chunks: [string, Uint8Array][]): Uint8Array {
  const body = concat(
    ...chunks.map(([t, d]) => {
      const h = new Uint8Array(8);
      h.set(enc(t), 0);
      new DataView(h.buffer).setUint32(4, d.length, true);
      return concat(h, d, d.length & 1 ? new Uint8Array(1) : new Uint8Array(0));
    }),
  );
  const head = new Uint8Array(12);
  head.set(enc("RIFF"), 0);
  new DataView(head.buffer).setUint32(4, body.length + 4, true);
  head.set(enc("WEBP"), 8);
  return concat(head, body);
}
function vp8l(w: number, h: number): Uint8Array {
  const d = new Uint8Array(10);
  d[0] = 0x2f;
  new DataView(d.buffer).setUint32(1, ((w - 1) & 0x3fff) | (((h - 1) & 0x3fff) << 14), true);
  return d;
}
function vp8(w: number, h: number): Uint8Array {
  const d = new Uint8Array(20);
  d.set([0x10, 0, 0, 0x9d, 0x01, 0x2a], 0);
  new DataView(d.buffer).setUint16(6, w, true);
  new DataView(d.buffer).setUint16(8, h, true);
  return d;
}
function vp8x(flags: number, w: number, h: number): Uint8Array {
  const d = new Uint8Array(10);
  d[0] = flags;
  const dv = new DataView(d.buffer);
  dv.setUint16(4, (w - 1) & 0xffff, true);
  d[6] = ((w - 1) >> 16) & 255;
  dv.setUint16(7, (h - 1) & 0xffff, true);
  d[9] = ((h - 1) >> 16) & 255;
  return d;
}

test("WebP: VP8 / VP8L / VP8X(+ALPH) layouts only, flags clear, canvas equals bitstream, RIFF size exact", async () => {
  assert.deepEqual(await checkImage(riff([["VP8L", vp8l(30, 20)]])), { contentType: "image/webp", width: 30, height: 20 });
  assert.deepEqual(await checkImage(riff([["VP8 ", vp8(31, 21)]])), { contentType: "image/webp", width: 31, height: 21 });
  assert.equal(await refused(riff([["VP8X", vp8x(0x10, 30, 20)], ["ALPH", new Uint8Array([1, 0, 0])], ["VP8 ", vp8(30, 20)]])), null);
  assert.equal(await refused(riff([["VP8X", vp8x(0x08, 30, 20)], ["VP8L", vp8l(30, 20)]])), "webp_vp8x_flags", "EXIF flag");
  assert.equal(await refused(riff([["VP8X", vp8x(0x02, 30, 20)], ["VP8L", vp8l(30, 20)]])), "webp_vp8x_flags", "animation flag");
  assert.equal(await refused(riff([["VP8X", vp8x(0, 99, 20)], ["VP8L", vp8l(30, 20)]])), "webp_canvas");
  assert.equal(await refused(riff([["VP8X", vp8x(0, 30, 20)], ["VP8L", vp8l(30, 20)], ["EXIF", new Uint8Array(4)]])), "webp_layout");
  assert.equal(await refused(riff([["VP8X", vp8x(0, 30, 20)], ["ANIM", new Uint8Array(6)], ["ANMF", new Uint8Array(16)]])), "webp_layout");
  assert.equal(await refused(riff([["VP8L", vp8l(30, 20)], ["XMP ", enc("<x/>")]])), "webp_layout");
  const sized = riff([["VP8L", vp8l(30, 20)]]);
  assert.equal(await refused(concat(sized, new Uint8Array(2))), "webp_riff_size");
  const wav = concat(enc("RIFF"), new Uint8Array([4, 0, 0, 0]), enc("WAVE"));
  assert.equal(await refused(wav), "format", "a non-WebP RIFF");
});

test("anything else is refused by its bytes, whatever it claims to be", async () => {
  for (const b of [enc("<svg/>"), enc("BM\0\0\0\0"), enc("II*\0"), new Uint8Array(0), enc("%PDF-1.7"), new Uint8Array(2 * 1024 * 1024 + 1)]) assert.notEqual(await refused(b), null);
});

test("real encoder output (Pillow 12, ImageMagick): stripped stills pass; EXIF, animation and metadata chunks are refused", async () => {
  const { readFileSync } = await import("node:fs");
  const f = (n: string) => new Uint8Array(readFileSync(new URL(`./fixtures/${n}`, import.meta.url)));
  const ok: [string, string, number, number][] = [
    ["pil.jpg", "image/jpeg", 64, 48],
    ["pil_prog.jpg", "image/jpeg", 64, 48],
    ["pil.gif", "image/gif", 64, 48],
    ["pil_lossless.webp", "image/webp", 64, 48],
    ["pil_lossy.webp", "image/webp", 64, 48],
    ["pil_alpha_lossy.webp", "image/webp", 64, 48],
  ];
  for (const [n, contentType, width, height] of ok) assert.deepEqual(await checkImage(f(n)), { contentType, width, height }, n);
  assert.equal(await refused(f("pil_exif.jpg")), "jpeg_segment");
  assert.equal(await refused(f("anim.gif")), "gif_extension");
  assert.equal(await refused(f("anim.webp")), "webp_layout");
  assert.equal(await refused(f("im_nostrip.png")), "png_chunk");
});
