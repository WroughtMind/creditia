# SQLCipher 4.5.7 reference fixtures

Generated 2026-10-19 with the unmodified official
[v4.5.7 source](https://github.com/sqlcipher/sqlcipher/tree/v4.5.7), macOS arm64,
and Homebrew OpenSSL 3.6.3. These are synthetic `records` tables, with no TRAE
account data, tokens, personal paths, or real keys. The fixed test raw key is
`[7; 32]`; `generate.sql` supplies it in SQLCipher's documented raw-key hex form.
Random salts and IVs mean regeneration changes file hashes.

Official source archive:
`https://github.com/sqlcipher/sqlcipher/archive/refs/tags/v4.5.7.tar.gz`

Archive SHA256:
`b670845f28da0a3c717e991e9f18a334e08f6bc977190bbce6be864ca229f96d`.

## Reproduce

Build in the extracted official source directory (adjust the OpenSSL paths):

```sh
./configure --enable-tempstore=yes --disable-shared \
  CFLAGS='-O2 -DSQLITE_HAS_CODEC -I/opt/homebrew/opt/openssl@3/include' \
  LDFLAGS='-L/opt/homebrew/opt/openssl@3/lib -lcrypto'
make -j4 sqlcipher
```

Copy `generate.sql` into a new empty directory and run the resulting official
shell there:

```sh
/path/to/sqlcipher-4.5.7/sqlcipher source.db < generate.sql
```

The script reports `4.5.7 community`, page size `4096`, `HMAC_SHA512`, and
`PBKDF2_HMAC_SHA512`. Both its integrity checks report `ok`. It copies the
fixtures while the shell's database connection and transactions remain open.
No fixture was encrypted with Creditia's mirrored test encryptor.

| File | Contents | SHA256 |
| --- | --- | --- |
| `base.db` | Rollback-mode encrypted DB, 6 records, row 1 `base` | `67a58599da81d474a11eaf5fe92f88005cf61fa770af351f49de47c82fb811c7` |
| `wal.db` | Main DB before WAL commits, same 6 records | `47b8f2010e1d69687cc543b025a620e4797d78608e6778d6344e1bf628ed284d` |
| `committed.wal` | Two committed frames, 7 records, row 1 `committed`, row 7 `wal-only` | `fa6699d9fcb3e1f13614477ec603e57d150bbf29a595df26d258359786c58f32` |
| `uncommitted.wal` | Same commits plus 10 frames of an uncommitted update | `463667afdba17be98030a728ddff0d9dc11a93d11abe80920f7b0154896fcfc2` |
| `reused.db` | Main DB after RESTART checkpoint, 20 records | `8b728719012299f0d7f5ee63184f6059b95283d0e8256d89a01a5c1b755d3f53` |
| `reused.wal` | Three current-generation frames followed by 31 stale frames from the old generation | `e61fcb660000ad4bd772c1c4617742dd7cec89bb22d2add4448250cea5b214b6` |

The first two WAL snapshots have magic `0x377f0682` (little-endian checksum input). The committed
frame database sizes are `[7, 7]`; all following frame commit fields are zero.
The uncommitted update spills pages with a one-page cache. SQLite can overwrite
such frames and defer updating cumulative checksums until transaction commit
(`src/wal.c`, `walRewriteChecksums`). Thus this fixture covers a real tail that
a mirrored encryptor with immediately correct checksums did not model.

`generate-reused.sql` reproduces the independently discovered RESTART reuse
case. Run it in another empty directory with the same official shell; its
`saved.db` and `saved.db-wal` outputs are stored here as `reused.db` and
`reused.wal`. The header salt matches only the first three of 34 physical
frames. The third frame commits a 32-page database; the remaining old-generation
tail also contains a 32-page commit marker. Searching the entire physical file
for the last commit therefore incorrectly includes stale data. The reader must
first delimit the contiguous current generation by its salt, then locate and
authenticate its last commit. All current-generation committed checksum/HMAC
failures remain errors, including a corrupted newer commit following an earlier
valid commit.

Independent verification with the official shell used disposable copies named
`copy.db` and `copy.db-wal` (never the checked-in fixtures). After the same raw
key PRAGMA, querying `SELECT count(*), (SELECT body FROM records WHERE id=1)
FROM records; PRAGMA integrity_check;` returned:

| Snapshot | Official SQLCipher output | Creditia `open` output |
| --- | --- | --- |
| `base.db` | `6\|base`, `ok` | `6\|base`, `ok` |
| `wal.db` + `committed.wal` | `7\|committed`, `ok` | `7\|committed`, `ok` |
| `wal.db` + `uncommitted.wal` | `7\|committed`, `ok` | `7\|committed`, `ok` |
| `reused.db` + `reused.wal` | `20\|reused-wal-commit`, `ok` | `20\|reused-wal-commit`, `ok` |

Creditia tests also verify read-only queries, unchanged source bytes and
metadata, no created sidecars, wrong key, database tampering/truncation, WAL
header/frame tampering, authenticated-page tampering after recomputing ordinary
WAL checksums, partial WAL tail refusal, and rollback journal refusal. Synthetic
unit tests separately cover big-endian WAL checksums and missing new pages.
