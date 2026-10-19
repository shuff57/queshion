-- verSHion: staff-domain hints shown in the approval queue. A label for the reviewer only; it NEVER
-- grants trust (many districts give students addresses on the staff domain).
CREATE TABLE staff_domain_hint (
  domain     TEXT PRIMARY KEY,
  note       TEXT NOT NULL,
  updated_at INTEGER NOT NULL,
  updated_by TEXT
);
