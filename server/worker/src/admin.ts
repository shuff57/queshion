// verSHion-specific admin parts: staff-domain hints (labels for the reviewer, never a grant of
// trust) and their admin routes, served behind the auth package's admin guard.
import type { AdminRoute } from "../../auth/src/index.js";
import { cleanText } from "../../auth/src/validate.ts";

/**
 * Suffixes under which anyone can register, so a hint on them would label everyone. Not the full
 * Public Suffix List: a hint is only a label, so the cost of a miss is a misleading label, which the
 * reviewer still has to check against the staff directory.
 */
const PUBLIC_SUFFIXES = new Set([
  "com", "net", "org", "edu", "gov", "mil", "int", "info", "biz", "io", "co", "us", "uk", "ca", "au",
  "k12.ca.us", "ca.us", "co.uk", "ac.uk", "org.uk", "gov.uk", "sch.uk", "com.au", "edu.au", "gov.au",
  "gmail.com", "googlemail.com", "outlook.com", "hotmail.com", "live.com", "yahoo.com", "icloud.com",
  "me.com", "aol.com", "proton.me", "protonmail.com",
]);

/** Normalizes and validates a registrable domain (lower-case, labels, not a bare public suffix or a free-mail domain). */
export function registrableDomain(raw: unknown): string | null {
  if (typeof raw !== "string") return null;
  const d = raw.trim().toLowerCase().replace(/\.$/, "");
  if (d.length < 4 || d.length > 253) return null;
  const labels = d.split(".");
  if (labels.length < 2) return null;
  for (const l of labels) if (!/^[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?$/.test(l)) return null;
  const tld = labels[labels.length - 1]!;
  if (!/^(?:[a-z]{2,63}|xn--[a-z0-9-]{2,59})$/.test(tld)) return null;
  if (PUBLIC_SUFFIXES.has(d)) return null;
  return d;
}

export function staffDomainLabels(db: D1Database) {
  return async (domains: string[]): Promise<Record<string, string>> => {
    if (domains.length === 0) return {};
    const marks = domains.map((_, i) => `?${i + 1}`).join(", ");
    const rows = await db
      .prepare(`SELECT domain, note FROM staff_domain_hint WHERE domain IN (${marks})`)
      .bind(...domains)
      .all<{ domain: string; note: string }>();
    // The wording keeps it a hint: the reviewer still checks the directory or calls the office.
    return Object.fromEntries(rows.results.map((x) => [x.domain, `Staff-domain hint (not proof): ${x.note}`]));
  };
}

const ok = (body: unknown, status = 200) => Response.json(body, { status, headers: { "Cache-Control": "no-store" } });
const bad = (field: string) => ok({ error: "invalid_request", field }, 400);

export function staffDomainRoutes(db: D1Database): AdminRoute[] {
  return [
    {
      method: "GET",
      path: "/admin/staff-domains",
      write: false,
      handler: async () => {
        const rows = await db.prepare(`SELECT domain, note, updated_at FROM staff_domain_hint ORDER BY domain`).all();
        return ok({ domains: rows.results });
      },
    },
    {
      method: "PUT",
      path: "/admin/staff-domains",
      write: true,
      handler: async (_req, c) => {
        const domain = registrableDomain(c.body?.domain);
        if (!domain) return bad("domain");
        const note = cleanText(c.body?.note, 200);
        if (!note) return bad("note");
        await db.batch([
          db
            .prepare(
              `INSERT INTO staff_domain_hint (domain, note, updated_at, updated_by) VALUES (?1, ?2, ?3, ?4)
               ON CONFLICT (domain) DO UPDATE SET note = ?2, updated_at = ?3, updated_by = ?4`,
            )
            .bind(domain, note, c.now, c.admin.userId),
          c.audit({ action: "staff_domain.put", detail: { domain, note } }),
        ]);
        return ok({ ok: true, domain });
      },
    },
    {
      method: "DELETE",
      path: "/admin/staff-domains",
      write: true,
      handler: async (_req, c) => {
        const domain = registrableDomain(c.body?.domain);
        if (!domain) return bad("domain");
        const [del] = await db.batch([
          db.prepare(`DELETE FROM staff_domain_hint WHERE domain = ?1`).bind(domain),
          c.audit({ action: "staff_domain.delete", detail: { domain } }),
        ]);
        return (del!.meta.changes ?? 0) === 1 ? ok({ ok: true }) : ok({ error: "not_found" }, 404);
      },
    },
  ];
}
