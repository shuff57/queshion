// Argon2id spike: how long and how much memory per hash, and what happens under concurrency.
import wasm from "../../share-core/pkg/share_core_bg.wasm";
import { initSync, argon2id, memory_bytes } from "../../share-core/pkg/share_core.js";

let ready = false;
let inflight = 0;
let peakInflight = 0;

export default {
  async fetch(req: Request): Promise<Response> {
    if (!ready) { initSync({ module: wasm }); ready = true; }
    const u = new URL(req.url);
    const m = Number(u.searchParams.get("m") ?? 19456);
    const t = Number(u.searchParams.get("t") ?? 2);
    const p = Number(u.searchParams.get("p") ?? 1);
    inflight++; peakInflight = Math.max(peakInflight, inflight);
    try {
      const salt = crypto.getRandomValues(new Uint8Array(16));
      const tag = argon2id(new TextEncoder().encode("correct horse battery staple"), salt, m, t, p);
      return Response.json({ ok: true, m, t, p, tag: tag.length, wasmBytes: memory_bytes(), peakInflight });
    } catch (e) {
      return Response.json({ ok: false, error: String(e) }, { status: 500 });
    } finally { inflight--; }
  },
};
