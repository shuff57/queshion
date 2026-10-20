// Structural image checks for PUT /blobs (docs/shared-pool.md "Images", server rules).
//
// share-core does not have its image checks yet; core.ts uses share-core's `image_check` as soon as
// the package exports one and falls back to this file until then. The rules are the spec's: the
// format is sniffed from the bytes; PNG, JPEG, GIF or WebP only, as ONE still image, each side
// 1..4096, a strict allow-list of chunks/segments/blocks (no metadata, no animation), every length
// field consistent with the file, and nothing after the format's end marker.

export interface ImageInfo {
  contentType: "image/png" | "image/jpeg" | "image/gif" | "image/webp";
  width: number;
  height: number;
}

export const MAX_SIDE = 4096;
export const MAX_IMAGE_BYTES = 2 * 1024 * 1024;

class Refuse extends Error {}
const no = (why: string): never => {
  throw new Refuse(why);
};

const u16be = (b: Uint8Array, i: number) => (b[i]! << 8) | b[i + 1]!;
const u16le = (b: Uint8Array, i: number) => b[i]! | (b[i + 1]! << 8);
const u24le = (b: Uint8Array, i: number) => b[i]! | (b[i + 1]! << 8) | (b[i + 2]! << 16);
const u32be = (b: Uint8Array, i: number) => ((b[i]! << 24) >>> 0) + ((b[i + 1]! << 16) | (b[i + 2]! << 8) | b[i + 3]!);
const u32le = (b: Uint8Array, i: number) => (b[i]! | (b[i + 1]! << 8) | (b[i + 2]! << 16)) + ((b[i + 3]! << 24) >>> 0);
const ascii = (b: Uint8Array, i: number, n: number) => String.fromCharCode(...b.subarray(i, i + n));
const need = (b: Uint8Array, end: number) => {
  if (end > b.length) no("truncated");
};
const side = (w: number, h: number) => {
  if (!(w >= 1 && w <= MAX_SIDE && h >= 1 && h <= MAX_SIDE)) no("dimensions");
};

// ---- PNG --------------------------------------------------------------------------------------------

const PNG_SIG = [0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a];
const PNG_ALLOWED = new Set(["IHDR", "PLTE", "tRNS", "gAMA", "cHRM", "sRGB", "pHYs", "IDAT", "IEND"]);
let crcTable: Uint32Array | null = null;
function crc32(b: Uint8Array, start: number, end: number): number {
  if (!crcTable) {
    crcTable = new Uint32Array(256);
    for (let n = 0; n < 256; n++) {
      let c = n;
      for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
      crcTable[n] = c >>> 0;
    }
  }
  let c = 0xffffffff;
  for (let i = start; i < end; i++) c = crcTable[(c ^ b[i]!) & 0xff]! ^ (c >>> 8);
  return (c ^ 0xffffffff) >>> 0;
}

const CHANNELS: Record<number, number> = { 0: 1, 2: 3, 3: 1, 4: 2, 6: 4 };

/** Bytes of the filtered, uncompressed image data (Adam7 passes when interlaced). */
export function pngRawSize(w: number, h: number, depth: number, color: number, interlace: number): number {
  const bits = CHANNELS[color]! * depth;
  const row = (cols: number) => (cols === 0 ? 0 : 1 + Math.ceil((cols * bits) / 8));
  if (interlace === 0) return h * row(w);
  const passes = [[0, 0, 8, 8], [4, 0, 8, 8], [0, 4, 4, 8], [2, 0, 4, 4], [0, 2, 2, 4], [1, 0, 2, 2], [0, 1, 1, 2]];
  let n = 0;
  for (const [x0, y0, dx, dy] of passes) {
    const cols = Math.max(0, Math.ceil((w - x0!) / dx!));
    const rows = Math.max(0, Math.ceil((h - y0!) / dy!));
    if (cols > 0) n += rows * row(cols);
  }
  return n;
}

/** Inflates `z` (zlib) and checks it is exactly `expected` bytes: no trailing data, no bomb. */
async function inflatesTo(z: Uint8Array, expected: number): Promise<boolean> {
  const reader = new Blob([z]).stream().pipeThrough(new DecompressionStream("deflate")).getReader();
  let n = 0;
  try {
    for (;;) {
      const { done, value } = await reader.read();
      if (done) break;
      n += value.byteLength;
      if (n > expected) {
        await reader.cancel().catch(() => {});
        return false;
      }
    }
  } catch {
    return false; // corrupt, truncated, or data after the end of the zlib stream
  }
  return n === expected;
}

interface PngParsed extends ImageInfo {
  idat: Uint8Array[];
  raw: number;
}

function png(b: Uint8Array): PngParsed {
  let i = 8;
  let w = 0;
  let h = 0;
  let color = 0;
  let raw = 0;
  let plte = 0;
  const idat: Uint8Array[] = [];
  let seenIdat = false;
  let afterIdat = false;
  let first = true;
  const seen = new Set<string>();
  for (;;) {
    need(b, i + 12);
    const len = u32be(b, i);
    const type = ascii(b, i + 4, 4);
    if (len > b.length) no("png_length");
    need(b, i + 12 + len);
    if (!PNG_ALLOWED.has(type)) no(`png_chunk`);
    if (crc32(b, i + 4, i + 8 + len) !== u32be(b, i + 8 + len)) no("png_crc");
    if (first && type !== "IHDR") no("png_ihdr_first");
    if (!first && type === "IHDR") no("png_ihdr_twice");
    if (type !== "IDAT" && type !== "IEND" && seen.has(type)) no("png_chunk_repeated");
    seen.add(type);
    if (type === "IHDR") {
      if (len !== 13) no("png_ihdr");
      w = u32be(b, i + 8);
      h = u32be(b, i + 12);
      const [depth, ct, comp, filter, interlace] = [b[i + 16]!, b[i + 17]!, b[i + 18]!, b[i + 19]!, b[i + 20]!];
      color = ct;
      const okDepth: Record<number, number[]> = { 0: [1, 2, 4, 8, 16], 2: [8, 16], 3: [1, 2, 4, 8], 4: [8, 16], 6: [8, 16] };
      if (!okDepth[ct]?.includes(depth) || comp !== 0 || filter !== 0 || interlace > 1) no("png_ihdr");
      side(w, h);
      raw = pngRawSize(w, h, depth, color, interlace);
    }
    // Ancillary chunks have fixed sizes: nothing can ride along in them.
    const FIXED: Record<string, number> = { gAMA: 4, cHRM: 32, sRGB: 1, pHYs: 9 };
    if (type in FIXED && len !== FIXED[type]) no("png_chunk_size");
    if (type === "PLTE") {
      // A palette only where it means something: required for indexed colour, refused for greyscale.
      // (a "suggested palette" on truecolour is legal PNG but carries nothing a re-encode needs)
      if (color === 0 || color === 4) no("png_plte_greyscale");
      if (color === 2 || color === 6) no("png_plte_truecolor");
      if (len === 0 || len % 3 !== 0 || len > 768) no("png_chunk_size");
      plte = len / 3;
    }
    if (type === "tRNS") {
      const okLen = color === 0 ? len === 2 : color === 2 ? len === 6 : color === 3 ? len >= 1 && len <= plte : false;
      if (!okLen) no("png_trns");
    }
    if (type === "IDAT") idat.push(b.subarray(i + 8, i + 8 + len));
    if (type === "IDAT") {
      if (afterIdat) no("png_idat_split");
      seenIdat = true;
    } else if (seenIdat) afterIdat = true;
    if (type === "PLTE" && seenIdat) no("png_order");
    first = false;
    i += 12 + len;
    if (type === "IEND") {
      if (len !== 0) no("png_iend");
      if (i !== b.length) no("trailing_bytes");
      break;
    }
  }
  if (!seenIdat) no("png_no_idat");
  if (color === 3 && plte === 0) no("png_no_plte");
  side(w, h);
  // Compressed data can be at most the raw size plus deflate's stored-block overhead.
  const z = idat.reduce((a, x) => a + x.length, 0);
  if (z > raw + 6 + 5 * Math.ceil(raw / 16383) + 64) no("png_idat_size");
  return { contentType: "image/png", width: w, height: h, idat, raw };
}

// ---- JPEG -------------------------------------------------------------------------------------------

function jpeg(b: Uint8Array): ImageInfo {
  let i = 2; // after SOI
  let w = 0;
  let h = 0;
  let frames = 0;
  let scans = 0;
  let dqt = 0;
  let dht = 0;
  let comps = 3;
  for (;;) {
    need(b, i + 2);
    if (b[i] !== 0xff) no("jpeg_marker");
    // Fill bytes (0xFF) may precede a marker.
    while (b[i + 1] === 0xff) {
      i++;
      need(b, i + 2);
    }
    const m = b[i + 1]!;
    i += 2;
    if (m === 0xd9) {
      if (scans === 0 || frames !== 1) no("jpeg_structure");
      if (i !== b.length) no("trailing_bytes");
      break;
    }
    if (m === 0xd8 || m === 0x01 || (m >= 0xd0 && m <= 0xd7)) no("jpeg_marker");
    need(b, i + 2);
    const len = u16be(b, i);
    if (len < 2) no("jpeg_length");
    need(b, i + len);
    const seg = b.subarray(i + 2, i + len);
    if (m === 0xe0) {
      // Exactly the 14-byte JFIF header with a 0x0 thumbnail: no JFXX, no embedded thumbnail, no payload.
      if (seg.length !== 14 || ascii(seg, 0, 5) !== "JFIF\0" || seg[12] !== 0 || seg[13] !== 0) no("jpeg_app0");
    } else if (m === 0xee) {
      if (seg.length !== 12 || ascii(seg, 0, 5) !== "Adobe") no("jpeg_app14");
    } else if (m === 0xc0 || m === 0xc1 || m === 0xc2) {
      frames++;
      if (frames > 1 || seg.length < 6) no("jpeg_sof");
      h = u16be(seg, 1);
      w = u16be(seg, 3);
      comps = seg[5]!;
      if (seg.length !== 6 + 3 * comps || (comps !== 1 && comps !== 3 && comps !== 4)) no("jpeg_sof");
    } else if (m === 0xdb) {
      // DQT: whole tables only (precision, id, 64 or 128 bytes), at most 4 per file.
      let k = 0;
      while (k < seg.length) {
        const pq = seg[k]! >> 4;
        if (pq > 1 || (seg[k]! & 15) > 3) no("jpeg_dqt");
        k += 1 + 64 * (pq + 1);
        if (++dqt > 4) no("jpeg_dqt");
      }
      if (k !== seg.length) no("jpeg_dqt");
    } else if (m === 0xc4) {
      // DHT: whole tables only (class, id, 16 counts, at most 256 symbols).
      let k = 0;
      while (k < seg.length) {
        if (seg[k]! >> 4 > 1 || (seg[k]! & 15) > 3 || k + 17 > seg.length) no("jpeg_dht");
        let sum = 0;
        for (let t = 1; t <= 16; t++) sum += seg[k + t]!;
        if (sum > 256) no("jpeg_dht");
        k += 17 + sum;
        if (++dht > 32) no("jpeg_dht");
      }
      if (k !== seg.length) no("jpeg_dht");
    } else if (m === 0xdd) {
      if (seg.length !== 2) no("jpeg_dri");
    } else if (m === 0xda) {
      if (frames !== 1) no("jpeg_structure");
      const ns = seg[0] ?? 0;
      if (ns < 1 || ns > 4 || seg.length !== 4 + 2 * ns) no("jpeg_sos");
      scans++;
      i += len;
      // Entropy-coded data: until a marker other than a stuffed 0x00 or a restart marker.
      for (;;) {
        need(b, i + 2);
        if (b[i] === 0xff) {
          const n = b[i + 1]!;
          if (n === 0x00 || (n >= 0xd0 && n <= 0xd7)) {
            i += 2;
            continue;
          }
          if (n === 0xff) {
            i++;
            continue;
          }
          break;
        }
        i++;
      }
      continue;
    } else {
      // APP1 (EXIF/XMP), APP2 (ICC), APP13 (IPTC), COM, every other APPn and marker.
      no("jpeg_segment");
    }
    i += len;
  }
  side(w, h);
  // Entropy-coded data cannot be checked without decoding; its size is bounded by the pixel count.
  if (b.length > 4096 + 2 * w * h * comps) no("jpeg_size");
  return { contentType: "image/jpeg", width: w, height: h };
}

// ---- GIF --------------------------------------------------------------------------------------------

function gif(b: Uint8Array): ImageInfo {
  need(b, 13);
  const W = u16le(b, 6);
  const H = u16le(b, 8);
  side(W, H);
  const packed = b[10]!;
  let i = 13;
  if (packed & 0x80) i += 3 * (1 << ((packed & 7) + 1));
  need(b, i);
  let images = 0;
  let gce = 0;
  const subBlocks = () => {
    for (;;) {
      need(b, i + 1);
      const n = b[i]!;
      i += 1;
      if (n === 0) return;
      need(b, i + n);
      i += n;
    }
  };
  for (;;) {
    need(b, i + 1);
    const t = b[i]!;
    if (t === 0x3b) {
      if (images !== 1) no("gif_structure");
      if (i + 1 !== b.length) no("trailing_bytes");
      break;
    }
    if (t === 0x21) {
      need(b, i + 2);
      const label = b[i + 1]!;
      if (label !== 0xf9) no("gif_extension"); // comment, application (looping, XMP), plain text
      if (images > 0 || ++gce > 1) no("gif_extension");
      need(b, i + 8);
      if (b[i + 2] !== 4 || b[i + 7] !== 0) no("gif_gce");
      i += 8;
      continue;
    }
    if (t === 0x2c) {
      if (++images > 1) no("gif_animated");
      need(b, i + 10);
      const [l, tp, w, h] = [u16le(b, i + 1), u16le(b, i + 3), u16le(b, i + 5), u16le(b, i + 7)];
      if (w < 1 || h < 1 || l + w > W || tp + h > H) no("gif_frame");
      const p = b[i + 9]!;
      i += 10;
      if (p & 0x80) i += 3 * (1 << ((p & 7) + 1));
      need(b, i + 1);
      const lzw = b[i]!;
      if (lzw < 2 || lzw > 8) no("gif_lzw");
      i += 1;
      const start = i;
      subBlocks();
      if (!lzwExact(b.subarray(start, i), lzw, w * h)) no("gif_lzw_data");
      continue;
    }
    no("gif_block");
  }
  return { contentType: "image/gif", width: W, height: H };
}

/**
 * Decodes a GIF LZW stream (sub-blocks still framed) far enough to count pixels: it must end with the
 * end-of-information code, decode to exactly `pixels`, and leave no whole byte unused after it.
 */
function lzwExact(blocks: Uint8Array, min: number, pixels: number): boolean {
  const data: number[] = [];
  for (let k = 0; k < blocks.length; ) {
    const n = blocks[k]!;
    if (n === 0) break;
    for (let t = 1; t <= n; t++) data.push(blocks[k + t]!);
    k += 1 + n;
  }
  const clear = 1 << min;
  const eoi = clear + 1;
  const len = new Uint16Array(4096);
  let size = min + 1;
  let next = eoi + 1;
  let prev = -1;
  let count = 0;
  let bit = 0;
  for (let t = 0; t < clear; t++) len[t] = 1;
  const total = data.length * 8;
  while (bit + size <= total) {
    let code = 0;
    for (let k = 0; k < size; k++, bit++) code |= ((data[bit >> 3]! >> (bit & 7)) & 1) << k;
    if (code === clear) {
      size = min + 1;
      next = eoi + 1;
      prev = -1;
      continue;
    }
    if (code === eoi) return count === pixels && total - bit < 8;
    let l: number;
    if (code < next && (code < clear || code > eoi)) l = len[code]!;
    else if (code === next && prev >= 0) l = len[prev]! + 1;
    else return false;
    count += l;
    if (count > pixels) return false;
    if (prev >= 0 && next < 4096) {
      len[next] = len[prev]! + 1;
      next++;
      if (next === 1 << size && size < 12) size++;
    }
    prev = code;
  }
  return false; // no end-of-information code
}

// ---- WebP -------------------------------------------------------------------------------------------

function vp8Size(d: Uint8Array): [number, number] {
  if (d.length < 10) no("webp_vp8");
  if ((d[0]! & 1) !== 0) no("webp_vp8_keyframe");
  if (d[3] !== 0x9d || d[4] !== 0x01 || d[5] !== 0x2a) no("webp_vp8_start");
  return [u16le(d, 6) & 0x3fff, u16le(d, 8) & 0x3fff];
}
function vp8lSize(d: Uint8Array): [number, number] {
  if (d.length < 5 || d[0] !== 0x2f) no("webp_vp8l");
  const bits = u32le(d, 1);
  const w = (bits & 0x3fff) + 1;
  const h = ((bits >>> 14) & 0x3fff) + 1;
  if (bits >>> 29 !== 0) no("webp_vp8l_version");
  return [w, h];
}

function webp(b: Uint8Array): ImageInfo {
  need(b, 12);
  if (u32le(b, 4) !== b.length - 8) no("webp_riff_size");
  let i = 12;
  const chunks: { type: string; data: Uint8Array }[] = [];
  while (i < b.length) {
    need(b, i + 8);
    const type = ascii(b, i, 4);
    const len = u32le(b, i + 4);
    need(b, i + 8 + len);
    chunks.push({ type, data: b.subarray(i + 8, i + 8 + len) });
    i += 8 + len;
    if (len & 1) {
      need(b, i + 1);
      i += 1;
    }
    if (chunks.length > 3) no("webp_layout");
  }
  if (i !== b.length) no("trailing_bytes");
  const types = chunks.map((c) => c.type).join(",");
  let w = 0;
  let h = 0;
  if (types === "VP8 ") [w, h] = vp8Size(chunks[0]!.data);
  else if (types === "VP8L") [w, h] = vp8lSize(chunks[0]!.data);
  else if (/^VP8X,(ALPH,)?(VP8 |VP8L)$/.test(types)) {
    const x = chunks[0]!.data;
    if (x.length !== 10) no("webp_vp8x");
    // ICC 0x20, EXIF 0x08, XMP 0x04, animation 0x02 must be clear (alpha 0x10 may be set).
    if (x[0]! & (0x20 | 0x08 | 0x04 | 0x02)) no("webp_vp8x_flags");
    const cw = u24le(x, 4) + 1;
    const ch = u24le(x, 7) + 1;
    const last = chunks[chunks.length - 1]!;
    [w, h] = last.type === "VP8 " ? vp8Size(last.data) : vp8lSize(last.data);
    if (cw !== w || ch !== h) no("webp_canvas");
  } else no("webp_layout");
  side(w, h);
  // Bitstreams and alpha bounded by the pixel count (uncompressed alpha is exactly 1 + w*h bytes).
  for (const ch of chunks) {
    if ((ch.type === "VP8 " || ch.type === "VP8L") && ch.data.length > 4 * w * h + 1024) no("webp_size");
    if (ch.type === "ALPH") {
      const method = ch.data[0]! & 3;
      if (method > 1 || (ch.data[0]! >> 6) !== 0) no("webp_alph");
      if (method === 0 ? ch.data.length !== 1 + w * h : ch.data.length > 1 + w * h + 1024) no("webp_alph_size");
    }
  }
  return { contentType: "image/webp", width: w, height: h };
}

/** The verified format and size, or the reason the bytes are refused (a code, never input text). */
export async function checkImage(b: Uint8Array): Promise<ImageInfo | { refused: string }> {
  try {
    if (b.length === 0 || b.length > MAX_IMAGE_BYTES) return { refused: "size" };
    if (PNG_SIG.every((x, k) => b[k] === x)) {
      const p = png(b);
      // The pixel data must inflate to exactly the image's size: nothing after the zlib stream.
      const z = new Uint8Array(p.idat.reduce((a, x) => a + x.length, 0));
      let o = 0;
      for (const x of p.idat) {
        z.set(x, o);
        o += x.length;
      }
      if (!(await inflatesTo(z, p.raw))) return { refused: "png_idat_data" };
      return { contentType: p.contentType, width: p.width, height: p.height };
    }
    if (b[0] === 0xff && b[1] === 0xd8 && b[2] === 0xff) return jpeg(b);
    if (b.length >= 6 && (ascii(b, 0, 6) === "GIF87a" || ascii(b, 0, 6) === "GIF89a")) return gif(b);
    if (b.length >= 12 && ascii(b, 0, 4) === "RIFF" && ascii(b, 8, 4) === "WEBP") return webp(b);
    return { refused: "format" };
  } catch (e) {
    if (e instanceof Refuse) return { refused: e.message };
    return { refused: "malformed" };
  }
}

export const EXT: Record<ImageInfo["contentType"], string> = { "image/png": "png", "image/jpeg": "jpg", "image/gif": "gif", "image/webp": "webp" };
