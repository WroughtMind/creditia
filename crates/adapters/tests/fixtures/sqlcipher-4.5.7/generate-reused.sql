.bail on
PRAGMA key = "x'0707070707070707070707070707070707070707070707070707070707070707'";
PRAGMA journal_mode=WAL;
PRAGMA wal_autocheckpoint=0;
CREATE TABLE records(id INTEGER PRIMARY KEY, body TEXT NOT NULL);
WITH RECURSIVE ids(i) AS (SELECT 1 UNION ALL SELECT i+1 FROM ids WHERE i<20) INSERT INTO records SELECT i, hex(zeroblob(3000)) FROM ids;
PRAGMA wal_checkpoint(RESTART);
UPDATE records SET body='reused-wal-commit' WHERE id=1;
.shell cp source.db saved.db
.shell cp source.db-wal saved.db-wal
SELECT body FROM records WHERE id=1;
PRAGMA integrity_check;
