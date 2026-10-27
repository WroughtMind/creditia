# Read-only SQLCipher 4 snapshots

Verified 2026-10-19. `crates/adapters/src/sqlcipher_pages.rs` exposes
`open(path, raw_key)` for SQLCipher 4's default encrypted page format. Its full
read path now passes against real encrypted databases and WAL files created
by the unmodified official SQLCipher 4.5.7 reference implementation. Test
provenance, reproduction steps, hashes, and independent official-shell query
results are in `crates/adapters/tests/fixtures/sqlcipher-4.5.7/README.md`.

This verifies the storage helper. TRAE's key derivation, native schema,
operation success receipts, and end-to-end real-account coverage belong to the
native reader and its own support documentation; reference fixtures alone do
not establish those product claims.

## Supported contract

- The caller supplies an already-derived **raw 32-byte encryption key**. This
  helper does not turn a password into the encryption key or guess a key.
- Page size is 4096 bytes, with 80 reserved bytes: a 16-byte IV followed by an
  HMAC-SHA512 tag. Page 1's first 16 bytes contain the database salt instead of
  the SQLite header. HMAC covers ciphertext, IV, and the little-endian page
  number. The MAC key uses PBKDF2-HMAC-SHA512 with two iterations and salt XOR
  `0x3a`. These parameters match the official 4.5.7 implementation.
- Main-file pages and every WAL frame through the last claimed commit in the
  current WAL generation must authenticate. Frame salts identify the contiguous
  current generation; a RESTART checkpoint may leave older generations and old
  commit markers in the physical tail, which are ignored. WAL headers and committed frame chains must pass SQLite's
  cumulative checksums. Claimed commits that omit newly allocated pages fail.
- Complete uncommitted WAL tail frames are ignored. SQLite can overwrite them
  before recomputing their cumulative checksums at commit, so authenticating
  the tail would incorrectly reject a valid prior committed snapshot. A
  claimed commit is never accepted without checksum and page authentication.
- Nonempty rollback journals, partial main-file pages, partial WAL frames,
  unsupported headers, wrong keys, and authenticated-data corruption fail
  explicitly. Nondefault page sizes, reserve sizes, or HMAC settings are outside
  this helper's contract; there is no fallback to unauthenticated plaintext.

## Read-only behavior

The helper reads the main file, `-wal`, and `-journal` directly as bytes and
requires two matching whole-file snapshots (size, mtime, and Unix device,
inode, ctime identity). It refuses observed concurrent changes. This is a
stable-file snapshot check, not a database lock; callers should retry a live
writer when the helper reports a changing source.

Only the in-memory image receives committed WAL pages and a switch from WAL
to rollback-format SQLite header bytes. `sqlite3_deserialize` opens that image
read-only, `query_only` is enabled, and `PRAGMA integrity_check` must return
`ok`. SQLite is never opened on the source path; the helper creates no WAL,
SHM, journal, decrypted database file, or key file. Reference tests compare
source bytes and metadata before/after successful reads and check that writes
to the returned connection fail.

Validation command:

```sh
CARGO_TARGET_DIR=/private/tmp/creditia-sqlcipher-target \
  cargo test -p creditia-adapters sqlcipher_pages
```

Nine targeted tests pass: five official-reference tests and four lower-level
synthetic tests. The official tests cover the complete decrypt → deserialize
→ integrity-check path, ordinary database data, committed WAL visibility,
real uncommitted WAL tail exclusion, RESTART WAL reuse with stale committed
frames in the physical tail, read-only behavior, and corruption/error boundaries.
A bad checksum or HMAC in a newer current-generation commit reports an error,
even if an earlier valid commit exists; it never silently returns older data. The synthetic tests provide additional constructed layout cases;
they are not the evidence for reference-format agreement.

The helper uses existing dependencies `aes`, `cbc`, `hmac`, `pbkdf2`, `sha2`,
and the `serialize` feature of `rusqlite`. No SQLCipher installation, external
process, new dependency, or platform-specific crypto library is required at
runtime.

Official references: [SQLCipher design](https://www.zetetic.net/sqlcipher/design/),
[4.5.7 raw-key handling and page MAC](https://github.com/sqlcipher/sqlcipher/blob/v4.5.7/src/crypto_impl.c),
[4.5.7 WAL writing and checksum rewrite](https://github.com/sqlcipher/sqlcipher/blob/v4.5.7/src/wal.c).
