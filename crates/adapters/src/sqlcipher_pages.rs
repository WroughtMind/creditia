//! Strict, read-only SQLCipher 4 default-format snapshots; see SUPPORT_SQLCIPHER.md.
use aes::Aes256;
use cbc::cipher::{block_padding::NoPadding, BlockDecryptMut, KeyIvInit};
use hmac::{Hmac, Mac};
use rusqlite::{Connection, MAIN_DB};
use sha2::Sha512;
use std::{
    fs,
    io::ErrorKind,
    path::{Path, PathBuf},
};

const PAGE: usize = 4096;
const RESERVE: usize = 80;
const DATA_END: usize = PAGE - RESERVE;
const IV_END: usize = DATA_END + 16;
const SQLITE_HEADER: &[u8; 16] = b"SQLite format 3\0";

/// Never opens SQLite on the source, creates sidecars, or writes decrypted bytes.
pub(crate) fn open(path: &Path, key: &[u8; 32]) -> Result<Connection, String> {
    let first = Snapshot::read(path)?;
    let second = Snapshot::read(path)?;
    if first != second {
        return Err("SQLCipher database changed while reading; retry when TRAE is idle".into());
    }
    if first.journal.as_ref().is_some_and(|j| !j.bytes.is_empty()) {
        return Err(
            "SQLCipher rollback journal is present; a consistent snapshot is unavailable".into(),
        );
    }
    let mut plain = decrypt_database(
        &first.database.bytes,
        first.wal.as_ref().map(|w| w.bytes.as_slice()),
        key,
    )?;
    // sqlite3_deserialize cannot open a WAL-mode image. All committed WAL frames
    // have already been applied and authenticated, so only the memory image is
    // converted to rollback format (SQLite serialization documentation).
    plain[18] = 1;
    plain[19] = 1;
    let mut connection = Connection::open_in_memory().map_err(|e| e.to_string())?;
    // rusqlite allocates with sqlite3_malloc and transfers ownership to SQLite.
    // A slice of known length cannot short-read or trigger its read-error path.
    connection
        .deserialize_read_exact(MAIN_DB, plain.as_slice(), plain.len(), true)
        .map_err(|e| format!("SQLCipher memory image: {e}"))?;
    connection
        .pragma_update(None, "query_only", true)
        .map_err(|e| e.to_string())?;
    let check: String = connection
        .query_row("PRAGMA integrity_check", [], |r| r.get(0))
        .map_err(|e| format!("SQLCipher SQLite integrity check: {e}"))?;
    if check != "ok" {
        return Err(format!("SQLCipher SQLite integrity check: {check}"));
    }
    Ok(connection)
}

#[derive(PartialEq, Eq)]
struct FileImage {
    bytes: Vec<u8>,
    modified: Option<std::time::SystemTime>,
    #[cfg(unix)]
    identity: (u64, u64, i64, i64),
}

impl FileImage {
    fn read(path: &Path) -> Result<Option<Self>, String> {
        let mut file = match fs::File::open(path) {
            Ok(file) => file,
            Err(e) if e.kind() == ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(format!("SQLCipher source read: {e}")),
        };
        let before = file.metadata().map_err(|e| e.to_string())?;
        let mut bytes = Vec::new();
        std::io::Read::read_to_end(&mut file, &mut bytes).map_err(|e| e.to_string())?;
        let after = file.metadata().map_err(|e| e.to_string())?;
        if before.len() != after.len()
            || before.modified().ok() != after.modified().ok()
            || after.len() != bytes.len() as u64
        {
            return Err("SQLCipher source changed during read".into());
        }
        #[cfg(unix)]
        let identity = {
            use std::os::unix::fs::MetadataExt;
            if before.ctime() != after.ctime() || before.ctime_nsec() != after.ctime_nsec() {
                return Err("SQLCipher source changed during read".into());
            }
            (after.dev(), after.ino(), after.ctime(), after.ctime_nsec())
        };
        Ok(Some(Self {
            bytes,
            modified: after.modified().ok(),
            #[cfg(unix)]
            identity,
        }))
    }
}

#[derive(PartialEq, Eq)]
struct Snapshot {
    database: FileImage,
    wal: Option<FileImage>,
    journal: Option<FileImage>,
}
impl Snapshot {
    fn read(path: &Path) -> Result<Self, String> {
        let database = FileImage::read(path)?.ok_or("SQLCipher database does not exist")?;
        let wal = FileImage::read(&sidecar(path, "-wal"))?;
        let journal = FileImage::read(&sidecar(path, "-journal"))?;
        Ok(Self {
            database,
            wal,
            journal,
        })
    }
}
fn sidecar(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.as_os_str().to_os_string();
    name.push(suffix);
    name.into()
}

fn hmac_key(key: &[u8; 32], salt: &[u8]) -> [u8; 32] {
    let masked: Vec<_> = salt.iter().map(|b| b ^ 0x3a).collect();
    let mut out = [0; 32];
    pbkdf2::pbkdf2_hmac::<Sha512>(key, &masked, 2, &mut out);
    out
}

fn decrypt_page(
    page: &[u8],
    number: u32,
    key: &[u8; 32],
    mac_key: &[u8; 32],
    salt: &[u8],
) -> Result<Vec<u8>, String> {
    if page.len() != PAGE || number == 0 {
        return Err("SQLCipher invalid page".into());
    }
    let start = if number == 1 { 16 } else { 0 };
    if number == 1 && &page[..16] != salt {
        return Err("SQLCipher page 1 salt changed".into());
    }
    let mut mac = <Hmac<Sha512> as Mac>::new_from_slice(mac_key).map_err(|e| e.to_string())?;
    mac.update(&page[start..IV_END]);
    mac.update(&number.to_le_bytes());
    mac.verify_slice(&page[IV_END..]).map_err(|_| format!("SQLCipher page {number} authentication failed (wrong key, unsupported format, or corruption)"))?;
    let mut result = vec![0; PAGE];
    result[start..DATA_END].copy_from_slice(&page[start..DATA_END]);
    cbc::Decryptor::<Aes256>::new(key.into(), (&page[DATA_END..IV_END]).into())
        .decrypt_padded_mut::<NoPadding>(&mut result[start..DATA_END])
        .map_err(|_| format!("SQLCipher page {number} decryption failed"))?;
    if number == 1 {
        result[..16].copy_from_slice(SQLITE_HEADER);
    }
    Ok(result)
}

fn decrypt_database(
    database: &[u8],
    wal: Option<&[u8]>,
    key: &[u8; 32],
) -> Result<Vec<u8>, String> {
    if database.is_empty() || !database.len().is_multiple_of(PAGE) {
        return Err("SQLCipher expected complete 4096-byte pages".into());
    }
    let count = database.len() / PAGE;
    if count > u32::MAX as usize {
        return Err("SQLCipher page number overflow".into());
    }
    let salt = &database[..16];
    let mac_key = hmac_key(key, salt);
    let mut plain = Vec::with_capacity(database.len());
    for (index, page) in database.as_chunks::<PAGE>().0.iter().enumerate() {
        plain.extend(decrypt_page(page, index as u32 + 1, key, &mac_key, salt)?);
    }
    if let Some(wal) = wal.filter(|w| !w.is_empty()) {
        apply_wal(&mut plain, wal, key, &mac_key, salt)?;
    }
    if &plain[..16] != SQLITE_HEADER
        || plain[16..18] != [0x10, 0]
        || plain[20] != RESERVE as u8
        || ![1, 2].contains(&plain[18])
        || ![1, 2].contains(&plain[19])
    {
        return Err("SQLCipher unsupported SQLite page header".into());
    }
    Ok(plain)
}

fn be32(bytes: &[u8]) -> u32 {
    u32::from_be_bytes(bytes[..4].try_into().unwrap())
}
fn checksum(bytes: &[u8], big_endian: bool, state: &mut [u32; 2]) {
    for pair in bytes.as_chunks::<8>().0 {
        let word = |v: &[u8]| {
            if big_endian {
                be32(v)
            } else {
                u32::from_le_bytes(v[..4].try_into().unwrap())
            }
        };
        state[0] = state[0].wrapping_add(word(pair)).wrapping_add(state[1]);
        state[1] = state[1]
            .wrapping_add(word(&pair[4..]))
            .wrapping_add(state[0]);
    }
}

fn apply_wal(
    plain: &mut Vec<u8>,
    wal: &[u8],
    key: &[u8; 32],
    mac_key: &[u8; 32],
    salt: &[u8],
) -> Result<(), String> {
    const FRAME: usize = 24 + PAGE;
    if wal.len() < 32 || !(wal.len() - 32).is_multiple_of(FRAME) {
        return Err("SQLCipher WAL is incomplete".into());
    }
    let magic = be32(wal);
    if magic & !1 != 0x377f0682 || be32(&wal[4..]) != 3_007_000 || be32(&wal[8..]) != PAGE as u32 {
        return Err("SQLCipher unsupported WAL header".into());
    }
    let big_endian = magic & 1 != 0;
    let mut state = [0; 2];
    checksum(&wal[..24], big_endian, &mut state);
    if state != [be32(&wal[24..]), be32(&wal[28..])] {
        return Err("SQLCipher WAL header checksum failed".into());
    }
    // SQLite can overwrite uncommitted frames and defer recomputing their
    // cumulative checksums until commit (walRewriteChecksums). Those tail
    // frames are not visible and must not prevent reading the last commit.
    // A claimed commit is trusted only after every frame through it passes
    // both the WAL checksum and SQLCipher authentication below.
    // RESTART checkpoints may reuse only the beginning of a WAL and leave
    // older generations, including old commit markers, in its physical tail.
    // Frame salts delimit the contiguous current generation. A checksum or
    // authentication failure within its committed prefix still fails closed;
    // it is never treated as a reason to fall back to an earlier commit.
    let current_frames = wal[32..]
        .as_chunks::<FRAME>().0.iter()
        .take_while(|frame| frame[8..16] == wal[16..24])
        .count();
    let current_end = 32 + current_frames * FRAME;
    if (current_frames == 0 && wal.len() > 32)
        || wal[current_end..]
            .as_chunks::<FRAME>().0.iter()
            .any(|frame| frame[8..16] == wal[16..24])
    {
        return Err("SQLCipher WAL page number or salt is invalid".into());
    }
    let current = &wal[32..current_end];
    let committed = current
        .as_chunks::<FRAME>().0.iter()
        .enumerate()
        .rev()
        .find_map(|(index, frame)| {
            let pages = be32(&frame[4..]);
            (pages != 0).then_some((index + 1, pages))
        });
    let end = committed.map_or(0, |(end, _)| end);
    let mut frames = Vec::new();
    for frame in current.as_chunks::<FRAME>().0.iter().take(end) {
        let number = be32(frame);
        if number == 0 || frame[8..16] != wal[16..24] {
            return Err("SQLCipher WAL page number or salt is invalid".into());
        }
        checksum(&frame[..8], big_endian, &mut state);
        checksum(&frame[24..], big_endian, &mut state);
        if state != [be32(&frame[16..]), be32(&frame[20..])] {
            return Err("SQLCipher WAL frame checksum failed".into());
        }
        let page = decrypt_page(&frame[24..], number, key, mac_key, salt)?;
        frames.push((number, page));
    }
    if let Some((end, pages)) = committed {
        let length = (pages as usize)
            .checked_mul(PAGE)
            .ok_or("SQLCipher WAL size overflow")?;
        // Require every newly allocated page at this commit to have an actual
        // authenticated frame. Never synthesize zero pages to fill missing data.
        let original = plain.len() / PAGE;
        let mut new_pages = std::collections::BTreeSet::new();
        for (number, _) in &frames[..end] {
            if *number as usize > original && *number <= pages {
                new_pages.insert(*number);
            }
        }
        if pages as usize > original && new_pages.len() != pages as usize - original {
            return Err("SQLCipher WAL commit is missing new database pages".into());
        }
        plain.resize(length, 0);
        for (number, page) in &frames[..end] {
            if *number <= pages {
                let start = (*number as usize - 1) * PAGE;
                plain[start..start + PAGE].copy_from_slice(page);
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use cbc::cipher::{block_padding::NoPadding, BlockEncryptMut, KeyIvInit};

    // The mirrored encryptor below is only for targeted layout/error tests.
    // Independent SQLCipher 4.5.7 fixtures exercise the complete open path,
    // including deserialize, integrity_check, and actual committed/uncommitted
    // WAL frames. See tests/fixtures/sqlcipher-4.5.7/README.md for provenance.
    fn encrypt_page(
        plain: &[u8],
        number: u32,
        key: &[u8; 32],
        mac_key: &[u8; 32],
        salt: &[u8],
        iv: [u8; 16],
    ) -> Vec<u8> {
        let start = if number == 1 { 16 } else { 0 };
        assert!(
            start <= DATA_END && DATA_END - start <= PAGE && (DATA_END - start).is_multiple_of(16)
        );
        let mut page = vec![0u8; PAGE];
        page[start..DATA_END].copy_from_slice(&plain[start..DATA_END]);
        cbc::Encryptor::<Aes256>::new(key.into(), (&iv).into())
            .encrypt_padded_mut::<NoPadding>(&mut page[start..DATA_END], DATA_END - start)
            .unwrap();
        page[DATA_END..IV_END].copy_from_slice(&iv);
        if number == 1 {
            page[..16].copy_from_slice(salt);
        }
        let mut mac = <Hmac<Sha512> as Mac>::new_from_slice(mac_key).unwrap();
        mac.update(&page[start..IV_END]);
        mac.update(&number.to_le_bytes());
        page[IV_END..].copy_from_slice(&mac.finalize().into_bytes());
        page
    }
    fn encrypt_database(plain: &[u8], key: &[u8; 32], salt: &[u8]) -> Vec<u8> {
        let mac_key = hmac_key(key, salt);
        plain
            .as_chunks::<PAGE>().0.iter()
            .enumerate()
            .flat_map(|(i, page)| {
                encrypt_page(page, i as u32 + 1, key, &mac_key, salt, [i as u8 + 1; 16])
            })
            .collect()
    }
    /// Deterministic bytes with the page-1 header fields decrypt_database checks:
    /// page size 4096, write/read version 1, reserved bytes 80.
    fn plain_image(pages: usize) -> Vec<u8> {
        let mut plain = vec![0u8; pages * PAGE];
        for (i, b) in plain.iter_mut().enumerate() {
            *b = (i % 251) as u8;
        }
        plain[16..18].copy_from_slice(&[0x10, 0]);
        plain[18] = 1;
        plain[19] = 1;
        plain[20] = RESERVE as u8;
        plain
    }
    const KEY: [u8; 32] = [7; 32];
    const SALT: [u8; 16] = [9; 16];

    fn fixture_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "creditia-sqlcipher-{name}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    const REFERENCE_BASE: &[u8] = include_bytes!("../tests/fixtures/sqlcipher-4.5.7/base.db");
    const REFERENCE_WAL_DB: &[u8] = include_bytes!("../tests/fixtures/sqlcipher-4.5.7/wal.db");
    const REFERENCE_COMMITTED_WAL: &[u8] =
        include_bytes!("../tests/fixtures/sqlcipher-4.5.7/committed.wal");
    const REFERENCE_UNCOMMITTED_WAL: &[u8] =
        include_bytes!("../tests/fixtures/sqlcipher-4.5.7/uncommitted.wal");

    fn assert_reference_rows(connection: &Connection, committed: bool) {
        let count: i64 = connection
            .query_row("SELECT count(*) FROM records", [], |r| r.get(0))
            .unwrap();
        let body: String = connection
            .query_row("SELECT body FROM records WHERE id=1", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, if committed { 7 } else { 6 });
        assert_eq!(body, if committed { "committed" } else { "base" });
        if committed {
            let body: String = connection
                .query_row("SELECT body FROM records WHERE id=7", [], |r| r.get(0))
                .unwrap();
            assert_eq!(body, "wal-only");
        }
        let check: String = connection
            .query_row("PRAGMA integrity_check", [], |r| r.get(0))
            .unwrap();
        assert_eq!(check, "ok");
        assert!(connection
            .execute("UPDATE records SET body='must not write' WHERE id=1", [])
            .is_err());
    }

    #[test]
    fn opens_sqlcipher_reference_database_without_touching_source() {
        let dir = fixture_dir("reference-base");
        let path = dir.join("database.db");
        fs::write(&path, REFERENCE_BASE).unwrap();
        let before = Snapshot::read(&path).unwrap();
        let connection = open(&path, &KEY).unwrap();
        assert_reference_rows(&connection, false);
        drop(connection);
        assert!(before == Snapshot::read(&path).unwrap());
        assert_eq!(fs::read_dir(&dir).unwrap().count(), 1); // no journal/WAL/SHM
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn opens_sqlcipher_reference_wal_and_ignores_real_uncommitted_tail() {
        const FRAME: usize = 24 + PAGE;
        assert!(REFERENCE_UNCOMMITTED_WAL.starts_with(REFERENCE_COMMITTED_WAL));
        assert!(REFERENCE_UNCOMMITTED_WAL.len() > REFERENCE_COMMITTED_WAL.len());
        assert_eq!(
            be32(&REFERENCE_UNCOMMITTED_WAL[REFERENCE_UNCOMMITTED_WAL.len() - FRAME + 4..]),
            0
        );
        for wal in [REFERENCE_COMMITTED_WAL, REFERENCE_UNCOMMITTED_WAL] {
            let dir = fixture_dir("reference-wal");
            let path = dir.join("database.db");
            fs::write(&path, REFERENCE_WAL_DB).unwrap();
            // The main file alone is older than the committed WAL.
            assert_reference_rows(&open(&path, &KEY).unwrap(), false);
            fs::write(sidecar(&path, "-wal"), wal).unwrap();
            let before = Snapshot::read(&path).unwrap();
            let connection = open(&path, &KEY).unwrap();
            assert_reference_rows(&connection, true);
            drop(connection);
            assert!(before == Snapshot::read(&path).unwrap());
            assert_eq!(fs::read_dir(&dir).unwrap().count(), 2); // never creates SHM
            fs::remove_dir_all(dir).unwrap();
        }
    }

    #[test]
    fn opens_sqlcipher_reference_reused_wal_without_reading_stale_generation() {
        const FRAME: usize = 24 + PAGE;
        let database = include_bytes!("../tests/fixtures/sqlcipher-4.5.7/reused.db");
        let wal = include_bytes!("../tests/fixtures/sqlcipher-4.5.7/reused.wal");
        let current = wal[32..]
            .as_chunks::<FRAME>().0.iter()
            .take_while(|frame| frame[8..16] == wal[16..24])
            .count();
        assert_eq!(current, 3);
        assert_eq!((wal.len() - 32) / FRAME, 34);
        // The old physical tail has a commit marker of its own. It must not
        // override the shorter current generation's third-frame commit.
        assert_ne!(be32(&wal[wal.len() - FRAME + 4..]), 0);
        let dir = fixture_dir("reference-reused-wal");
        let path = dir.join("database.db");
        fs::write(&path, database).unwrap();
        fs::write(sidecar(&path, "-wal"), wal).unwrap();
        let before = Snapshot::read(&path).unwrap();
        let connection = open(&path, &KEY).unwrap();
        let body: String = connection
            .query_row("SELECT body FROM records WHERE id=1", [], |r| r.get(0))
            .unwrap();
        assert_eq!(body, "reused-wal-commit");
        let count: i64 = connection
            .query_row("SELECT count(*) FROM records", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 20);
        let check: String = connection
            .query_row("PRAGMA integrity_check", [], |r| r.get(0))
            .unwrap();
        assert_eq!(check, "ok");
        drop(connection);
        assert!(before == Snapshot::read(&path).unwrap());
        assert_eq!(fs::read_dir(&dir).unwrap().count(), 2);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn reference_database_rejects_wrong_key_corruption_truncation_and_journal() {
        let dir = fixture_dir("reference-reject");
        let path = dir.join("database.db");
        fs::write(&path, REFERENCE_BASE).unwrap();
        assert!(open(&path, &[8; 32])
            .unwrap_err()
            .contains("authentication"));
        let mut corrupt = REFERENCE_BASE.to_vec();
        corrupt[PAGE + 57] ^= 0x40;
        fs::write(&path, corrupt).unwrap();
        assert!(open(&path, &KEY).unwrap_err().contains("authentication"));
        fs::write(&path, &REFERENCE_BASE[..REFERENCE_BASE.len() - 1]).unwrap();
        assert!(open(&path, &KEY)
            .unwrap_err()
            .contains("complete 4096-byte pages"));
        fs::write(&path, REFERENCE_BASE).unwrap();
        fs::write(sidecar(&path, "-journal"), b"active rollback journal").unwrap();
        assert!(open(&path, &KEY).unwrap_err().contains("journal"));
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn reference_wal_rejects_corrupt_headers_frames_and_partial_tail() {
        let dir = fixture_dir("reference-wal-reject");
        let path = dir.join("database.db");
        fs::write(&path, REFERENCE_WAL_DB).unwrap();
        for (offset, expected) in [
            (16, "header checksum"),
            (32 + 24 + 50, "frame checksum"),
            // The second frame is a newer commit. Corruption must not fall
            // back to the first frame's valid older commit.
            (32 + (24 + PAGE) + 24 + 50, "frame checksum"),
        ] {
            let mut corrupt = REFERENCE_COMMITTED_WAL.to_vec();
            corrupt[offset] ^= 1;
            fs::write(sidecar(&path, "-wal"), corrupt).unwrap();
            assert!(open(&path, &KEY).unwrap_err().contains(expected));
        }
        // Recompute ordinary WAL checksums after ciphertext tampering so the
        // independently keyed SQLCipher page authentication must reject it.
        for frame_index in [0, 1] {
            let mut corrupt = REFERENCE_COMMITTED_WAL.to_vec();
            corrupt[32 + frame_index * (24 + PAGE) + 24 + 50] ^= 1;
            let mut state = [0; 2];
            checksum(&corrupt[..24], false, &mut state);
            for frame in corrupt[32..].as_chunks_mut::<{ 24 + PAGE }>().0 {
                checksum(&frame[..8], false, &mut state);
                checksum(&frame[24..], false, &mut state);
                frame[16..20].copy_from_slice(&state[0].to_be_bytes());
                frame[20..24].copy_from_slice(&state[1].to_be_bytes());
            }
            fs::write(sidecar(&path, "-wal"), corrupt).unwrap();
            assert!(open(&path, &KEY).unwrap_err().contains("authentication"));
        }
        fs::write(
            sidecar(&path, "-wal"),
            &REFERENCE_UNCOMMITTED_WAL[..REFERENCE_UNCOMMITTED_WAL.len() - 1],
        )
        .unwrap();
        assert!(open(&path, &KEY).unwrap_err().contains("WAL is incomplete"));
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn decrypt_roundtrips_a_self_encrypted_image() {
        let plain = plain_image(3);
        let cipher = encrypt_database(&plain, &KEY, &SALT);
        // Decrypted pages carry zeros in the reserved area; page 1 replaces
        // the salt with the standard SQLite header.
        let mut expected = plain.clone();
        for page in expected.as_chunks_mut::<PAGE>().0 {
            page[DATA_END..].fill(0);
        }
        expected[..16].copy_from_slice(SQLITE_HEADER);
        assert_eq!(decrypt_database(&cipher, None, &KEY).unwrap(), expected);
        // A wrong key fails page authentication before returning anything.
        let wrong = [8; 32];
        assert!(decrypt_database(&cipher, None, &wrong).is_err());
    }

    #[test]
    fn rejects_tampering_truncation_and_foreign_layouts() {
        let plain = plain_image(2);
        let dir = fixture_dir("reject");
        let db = dir.join("database.db");
        std::fs::write(&db, encrypt_database(&plain, &KEY, &SALT)).unwrap();
        let mut tampered = std::fs::read(&db).unwrap();
        tampered[PAGE / 2 + 3] ^= 0x40;
        std::fs::write(&db, tampered).unwrap();
        assert!(open(&db, &KEY).is_err());
        std::fs::write(&db, encrypt_database(&plain, &KEY, &SALT)).unwrap();
        assert!(open(&db, &[8; 32]).is_err());
        let mut truncated = std::fs::read(&db).unwrap();
        truncated.truncate(truncated.len() - 100);
        std::fs::write(&db, truncated).unwrap();
        assert!(open(&db, &KEY).is_err());
        std::fs::write(&db, b"short").unwrap();
        assert!(open(&db, &KEY).is_err());
        // A reserve-0 or page-size-mismatch plaintext is refused after decrypt.
        let mut bad = plain_image(1);
        bad[20] = 0;
        assert!(decrypt_database(&encrypt_database(&bad, &KEY, &SALT), None, &KEY).is_err());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn refuses_rollback_journals_and_reports_live_databases() {
        let plain = plain_image(1);
        let dir = fixture_dir("journal");
        let db = dir.join("database.db");
        std::fs::write(&db, encrypt_database(&plain, &KEY, &SALT)).unwrap();
        std::fs::write(dir.join("database.db-journal"), b"hot journal").unwrap();
        let err = open(&db, &KEY).unwrap_err();
        assert!(err.contains("journal"), "{err}");
        std::fs::remove_file(dir.join("database.db-journal")).unwrap();
        // Without the journal the same file proceeds past snapshot checks into
        // decryption; it then fails SQLite integrity_check because the fixture
        // is not a real table image, which is the expected gate here.
        let err = open(&db, &KEY).unwrap_err();
        assert!(err.contains("integrity") || err.contains("SQLite"), "{err}");
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn applies_committed_wal_frames_cumulatively_and_rejects_bad_ones() {
        let plain = plain_image(2);
        let mac_key = hmac_key(&KEY, &SALT);
        let mut page2 = plain[PAGE..2 * PAGE].to_vec();
        page2[100] ^= 0xAA;
        let enc_page = encrypt_page(&page2, 2, &KEY, &mac_key, &SALT, [3; 16]);
        let wal_salt: [u8; 8] = [5; 8];
        let mut wal = vec![0u8; 32];
        wal[..4].copy_from_slice(&0x377f0683u32.to_be_bytes());
        wal[4..8].copy_from_slice(&3_007_000u32.to_be_bytes());
        wal[8..12].copy_from_slice(&(PAGE as u32).to_be_bytes());
        wal[16..24].copy_from_slice(&wal_salt);
        let mut state = [0u32; 2];
        checksum(&wal[..24], true, &mut state);
        wal[24..28].copy_from_slice(&state[0].to_be_bytes());
        wal[28..32].copy_from_slice(&state[1].to_be_bytes());
        let push_frame =
            |wal: &mut Vec<u8>, number: u32, page: &[u8], size_after: u32, state: &mut [u32; 2]| {
                let start = wal.len();
                wal.extend_from_slice(&number.to_be_bytes());
                wal.extend_from_slice(&size_after.to_be_bytes());
                wal.extend_from_slice(&wal_salt);
                wal.extend_from_slice(&[0u8; 8]); // checksums, filled below
                wal.extend_from_slice(page);
                checksum(&wal[start..start + 8], true, state);
                checksum(&wal[start + 24..], true, state);
                let (c0, c1) = (state[0].to_be_bytes(), state[1].to_be_bytes());
                wal[start + 16..start + 20].copy_from_slice(&c0);
                wal[start + 20..start + 24].copy_from_slice(&c1);
            };
        push_frame(&mut wal, 2, &enc_page, 2, &mut state);
        push_frame(&mut wal, 2, &enc_page, 0, &mut state); // uncommitted tail

        let cipher = encrypt_database(&plain, &KEY, &SALT);
        let mut image = decrypt_database(&cipher, None, &KEY).unwrap();
        apply_wal(&mut image, &wal, &KEY, &mac_key, &SALT).unwrap();
        let mut expected = image.clone();
        page2[DATA_END..].fill(0);
        expected[PAGE..2 * PAGE].copy_from_slice(&page2);
        assert_eq!(image, expected);

        let mut bad_salt = wal.clone();
        bad_salt[16] ^= 1;
        assert!(apply_wal(&mut image.clone(), &bad_salt, &KEY, &mac_key, &SALT).is_err());
        let mut corrupt = wal.clone();
        corrupt[40] ^= 1;
        assert!(apply_wal(&mut image.clone(), &corrupt, &KEY, &mac_key, &SALT).is_err());
        // A commit claiming new pages without frames for them is refused.
        let mut grown = wal.clone();
        grown.truncate(32 + 24 + PAGE);
        let mut grow_state = [0u32; 2];
        checksum(&grown[..24], true, &mut grow_state);
        grown[24..28].copy_from_slice(&grow_state[0].to_be_bytes());
        grown[28..32].copy_from_slice(&grow_state[1].to_be_bytes());
        push_frame(&mut grown, 5, &enc_page, 5, &mut grow_state);
        assert!(apply_wal(&mut image.clone(), &grown, &KEY, &mac_key, &SALT).is_err());
    }
}
