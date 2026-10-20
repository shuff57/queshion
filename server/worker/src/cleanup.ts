// Account hooks for the pool (docs/shared-pool.md "Account deletion", "Content follows the author's
// standing"). The statements live with the pool tables in src/pool/index.ts.
import { deleteContentStatements, standingStatements } from "./pool/index.ts";

/** Account deletion: the author's items are removed at once and purged after 14 days; see deleteContentStatements. */
export function onDeleteContent(db: D1Database, userId: string, now: number): D1PreparedStatement[] {
  return deleteContentStatements(db, userId, now);
}

/** Suspension hides nothing by writing: every non-author read joins the author's standing. Audited only. */
export function onSuspendContent(db: D1Database, userId: string, now: number): D1PreparedStatement[] {
  return standingStatements(db, userId, now, "author_suspended");
}

export function onRestoreContent(db: D1Database, userId: string, now: number): D1PreparedStatement[] {
  return standingStatements(db, userId, now, "author_restored");
}
