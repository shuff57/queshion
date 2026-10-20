// PUT /blobs/:sha256 (docs/shared-pool.md "Blob rules", "Images"). Full bytes always; the hash is
// recomputed; the format is sniffed from the bytes; the blob is charged to the uploader as pending
// (quota checked in the same INSERT); the answer is the same whether or not the blob existed, so an
// upload is never an existence oracle.
import { MAX_IMAGE_BYTES } from "./image.ts";
import { json, limitOf, PoolError, usage, readBytes, requireDaily, SHA_RE, sha256Hex, USAGE_SQL, DAY_MS, type Ctx, type PoolPrincipal } from "./shared.ts";

/** A pending upload stays charged (and publishable) this long. */
export const PENDING_MS = DAY_MS;

export async function putBlob(c: Ctx, req: Request, p: PoolPrincipal, sha: string): Promise<Response> {
  const me = p.userId;
  if (!SHA_RE.test(sha)) throw new PoolError(400, "invalid_request", { field: "sha256" });
  const bytes = await readBytes(req, MAX_IMAGE_BYTES);
  await requireDaily(c, me, "blob_uploads");
  await requireDaily(c, me, "blob_bytes", bytes.length);
  if ((await sha256Hex(bytes)) !== sha) throw new PoolError(422, "hash_mismatch");
  // Bytes confirmed as student data are refused, with the same answer as any other refused image.
  if (await c.db.prepare(`SELECT 1 AS x FROM privacy_blocks WHERE kind = 'blob' AND hash = ?1`).bind(sha).first()) throw new PoolError(422, "image_refused", { reason: "not_accepted" });
  const info = await c.core.imageInfo(bytes);
  if ("refused" in info) throw new PoolError(422, "image_refused", { reason: info.refused });
  const now = c.now();
  const quota = await limitOf(c, me, "storage_bytes");
  // Charged once per (blob, uploader): a blob this author already pays for (pending or in one of
  // their publishes) costs nothing more. Another author's copy of the same image costs this one too.
  const pend = await c.db
    .prepare(
      `INSERT INTO pending_blobs (sha256, uploader_id, size, until)
       SELECT ?3, ?1, ?4, ?5
        WHERE (CASE WHEN EXISTS (SELECT 1 FROM pending_blobs WHERE sha256 = ?3 AND uploader_id = ?1 AND until >= ?2)
                      OR EXISTS (SELECT 1 FROM item_blobs ib JOIN item_publishes ip ON ip.publish_id = ib.publish_id WHERE ib.sha256 = ?3 AND ip.author_id = ?1)
                    THEN 0 ELSE ?4 END) + ${USAGE_SQL} <= ?6
       ON CONFLICT (sha256, uploader_id) DO UPDATE SET until = excluded.until, size = excluded.size`,
    )
    .bind(me, now, sha, bytes.length, now + PENDING_MS, quota)
    .run();
  if ((pend.meta.changes ?? 0) === 0) throw new PoolError(413, "quota_exceeded", { used: await usage(c, me), needed: bytes.length, limit: quota });
  // Content-addressed: rewriting identical bytes is harmless, and it repairs a GC that raced us.
  await c.r2.put(`blobs/${sha}`, bytes, { httpMetadata: { contentType: info.contentType } });
  await c.db
    .prepare(`INSERT INTO blobs (sha256, size, content_type, width, height, first_uploaded_at, checked_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6)
             ON CONFLICT (sha256) DO UPDATE SET missing_at = NULL, checked_at = ?6`)
    .bind(sha, bytes.length, info.contentType, info.width, info.height, now)
    .run();
  return json(201, { sha256: sha, size: bytes.length, width: info.width, height: info.height });
}
