//! Public, release-wide database codec of official TRAE 3.6.2/2.3.91093 and 3.6.4/2.3.93439.
//! Fixed configuration mics_sj10gy is decoded with the release's three cyclic
//! masks, then PBKDF2-HMAC-SHA256 (100000 rounds, fixed 16-byte salt) produces
//! the 32-byte raw SQLCipher key. No account credentials or token are read.
use sha2::Sha256;
pub(super) fn raw_key() -> [u8; 32] {
    const CONFIG: [u8; 32] = [
        69, 42, 23, 53, 29, 25, 30, 19, 54, 9, 20, 1, 45, 78, 46, 0, 23, 91, 36, 29, 15, 10, 56, 9,
        31, 33, 33, 14, 36, 24, 18, 83,
    ];
    const SALT: [u8; 16] = [
        18, 52, 86, 120, 154, 188, 222, 240, 17, 34, 51, 68, 85, 102, 119, 136,
    ];
    let mut input = CONFIG;
    for (i, byte) in input.iter_mut().enumerate() {
        *byte ^= b"rust"[i % 4] ^ b"cpp"[i % 3] ^ b"electron"[i % 8];
    }
    let mut key = [0; 32];
    pbkdf2::pbkdf2_hmac::<Sha256>(&input, &SALT, 100_000, &mut key);
    input.fill(0);
    key
}
#[cfg(test)]
mod tests {
    use super::*;
    use sha2::Digest;
    #[test]
    fn matches_release_codec_vector() {
        // Vector independently derived from the official executable, without
        // persisting plaintext config or account-specific values in fixtures.
        assert_eq!(
            format!("{:x}", Sha256::digest(raw_key())),
            "416838763e4091d69dc77d4f721bd729fd89b325f438b9822bdf55fcd7710da7"
        );
    }
}
