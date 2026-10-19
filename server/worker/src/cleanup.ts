// Account-deletion content cleanup.
//
// PHASE 2 TODO: once the pool tables exist, return the statements docs/shared-pool.md asks for:
// delete ratings and favorites, anonymize filed reports, set removed_at on authored items, and
// start the circle claim window. Until then nothing references a user, but this logs loudly on
// every deletion so nobody mistakes it for working cleanup.
export function onDeleteContent(userId: string, _now: number): D1PreparedStatement[] {
  console.error(`PHASE 2 TODO: account ${userId} deleted; pool content cleanup is not implemented yet (server/worker/src/cleanup.ts)`);
  return [];
}

// PHASE 2 TODO: hide the suspended user's published items (and show them again on restore). Until
// the pool tables exist there is nothing to hide; this logs so nobody mistakes it for working.
export function onSuspendContent(userId: string, _now: number): D1PreparedStatement[] {
  console.error(`PHASE 2 TODO: account ${userId} suspended; hiding pool content is not implemented yet (server/worker/src/cleanup.ts)`);
  return [];
}
