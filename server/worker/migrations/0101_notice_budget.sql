-- verSHion: at most a few "new application" mails to the admins per hour; the rest are counted here
-- and rolled into one summary mail by the hourly cron, so many sign-ups cannot flood admin inboxes.
CREATE TABLE notice_budget (
  hour INTEGER PRIMARY KEY,
  sent INTEGER NOT NULL
);
CREATE TABLE notice_backlog (
  id      INTEGER PRIMARY KEY CHECK (id = 1),
  pending INTEGER NOT NULL
);
INSERT INTO notice_backlog (id, pending) VALUES (1, 0);
