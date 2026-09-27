use std::collections::{HashMap, HashSet};
use std::hash::{BuildHasherDefault, Hasher};

/// Multiply-mix hasher for the engine's hot lookup tables. The keys are
/// interned ids and small packed structs produced by the engine itself, so
/// SipHash's DoS resistance buys nothing while its per-lookup cost shows up
/// directly in tokenization profiles.
#[derive(Debug, Clone)]
pub(crate) struct FastHasher(u64);

impl Default for FastHasher {
    fn default() -> Self {
        Self(0x517c_c1b7_2722_0a95)
    }
}

impl Hasher for FastHasher {
    fn finish(&self) -> u64 {
        self.0
    }

    fn write(&mut self, bytes: &[u8]) {
        for byte in bytes {
            self.0 = (self.0 ^ u64::from(*byte)).wrapping_mul(0x0000_0100_0000_01b3);
        }
    }

    fn write_u64(&mut self, value: u64) {
        self.0 ^= value.wrapping_mul(0x9e37_79b1_85eb_ca87);
        self.0 = self.0.rotate_left(27).wrapping_mul(0x94d0_49bb_1331_11eb);
    }

    fn write_usize(&mut self, value: usize) {
        self.write_u64(value as u64);
    }

    fn write_u32(&mut self, value: u32) {
        self.write_u64(u64::from(value));
    }

    fn write_u16(&mut self, value: u16) {
        self.write_u64(u64::from(value));
    }

    fn write_u8(&mut self, value: u8) {
        self.write_u64(u64::from(value));
    }
}

/// Word-at-a-time hasher for the engine's string-keyed tables (scope names,
/// scope templates, grammar scopes). `FastHasher` mixes one byte at a time,
/// and SipHash's DoS resistance is unnecessary for grammar-derived keys.
#[derive(Debug, Clone, Default)]
pub(crate) struct StrHasher(u64);

impl Hasher for StrHasher {
    fn finish(&self) -> u64 {
        let mut hash = self.0;
        hash ^= hash >> 33;
        hash = hash.wrapping_mul(0xff51_afd7_ed55_8ccd);
        hash ^ (hash >> 33)
    }

    fn write(&mut self, bytes: &[u8]) {
        const K: u64 = 0xf135_7aea_2e62_a9c5;
        let mut hash = (self.0 ^ bytes.len() as u64).wrapping_mul(K);
        let (words, remainder) = bytes.as_chunks::<8>();
        for word in words {
            hash = (hash ^ u64::from_le_bytes(*word))
                .wrapping_mul(K)
                .rotate_left(26);
        }
        let mut tail = [0u8; 8];
        tail[..remainder.len()].copy_from_slice(remainder);
        self.0 = (hash ^ u64::from_le_bytes(tail)).wrapping_mul(K);
    }
}

pub(crate) type FastMap<K, V> = HashMap<K, V, BuildHasherDefault<FastHasher>>;
pub(crate) type StrMap<K, V> = HashMap<K, V, BuildHasherDefault<StrHasher>>;
pub(crate) type FastSet<T> = HashSet<T, BuildHasherDefault<FastHasher>>;

pub(crate) fn fast_map<K, V>() -> FastMap<K, V> {
    HashMap::with_hasher(BuildHasherDefault::default())
}

pub(crate) fn fast_set<T>() -> FastSet<T> {
    HashSet::with_hasher(BuildHasherDefault::default())
}
