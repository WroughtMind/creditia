.bail on
PRAGMA key = "x'0707070707070707070707070707070707070707070707070707070707070707'";
PRAGMA cipher_version;
PRAGMA cipher_page_size;
PRAGMA cipher_hmac_algorithm;
PRAGMA cipher_kdf_algorithm;
CREATE TABLE records(id INTEGER PRIMARY KEY, body TEXT NOT NULL);
INSERT INTO records VALUES(1,'base');
WITH RECURSIVE ids(i) AS (SELECT 2 UNION ALL SELECT i+1 FROM ids WHERE i<6)
INSERT INTO records SELECT i, printf('row-%03d-',i)||hex(zeroblob(1000)) FROM ids;
PRAGMA integrity_check;
.shell cp source.db base.db
PRAGMA journal_mode=WAL;
PRAGMA wal_autocheckpoint=0;
UPDATE records SET body='committed' WHERE id=1;
INSERT INTO records VALUES(7,'wal-only');
.shell cp source.db wal.db
.shell cp source.db-wal committed.wal
PRAGMA cache_size=1;
BEGIN;
UPDATE records SET body='uncommitted-'||hex(zeroblob(3000));
.shell cp source.db-wal uncommitted.wal
ROLLBACK;
SELECT count(*), (SELECT body FROM records WHERE id=1) FROM records;
PRAGMA integrity_check;
