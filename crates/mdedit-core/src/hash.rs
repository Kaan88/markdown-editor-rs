//! A fast 128-bit hash of bytes (two multiply-fold streams), for telling lines apart in a
//! comparison and for recognising a file's content again. Not cryptographic: the inputs are the
//! user's own files, and a collision would only mislead a display or skip a safety check that a
//! second test (the length, the stamp) also guards.

const K1: u64 = 0x9E37_79B9_7F4A_7C15;
const K2: u64 = 0xC2B2_AE3D_27D4_EB4F;

fn fold(a: u64, b: u64) -> u64 {
    let m = u128::from(a) * u128::from(b);
    (m as u64) ^ ((m >> 64) as u64)
}

/// Two 64-bit hashes of `bytes`, as one number.
pub fn hash128(bytes: &[u8]) -> u128 {
    let (mut h1, mut h2) = (
        K1 ^ bytes.len() as u64,
        K2.rotate_left(17) ^ bytes.len() as u64,
    );
    let (words, rest) = bytes.as_chunks::<8>();
    for w in words {
        let w = u64::from_le_bytes(*w);
        h1 = fold(h1 ^ w, K1);
        h2 = fold(h2.rotate_left(23) ^ w, K2);
    }
    let mut tail = 0u64;
    for (i, b) in rest.iter().enumerate() {
        tail |= u64::from(*b) << (8 * i);
    }
    h1 = fold(h1 ^ tail, K2);
    h2 = fold(h2 ^ tail ^ K1, K1);
    (u128::from(h1) << 64) | u128::from(fold(h1 ^ h2, K1) ^ h2)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn differs_and_repeats() {
        assert_eq!(hash128(b"abc"), hash128(b"abc"));
        let mut seen = std::collections::HashSet::new();
        for i in 0..20_000u32 {
            assert!(seen.insert(hash128(format!("line {i}").as_bytes())));
        }
        assert_ne!(hash128(b""), hash128(b"\0"));
        assert_ne!(hash128(b"a"), hash128(b"a\0"));
        assert_ne!(hash128(b"abcdefgh"), hash128(b"abcdefgi"));
    }
}
