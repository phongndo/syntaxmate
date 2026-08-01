use std::collections::HashMap;
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

pub(crate) type FastMap<K, V> = HashMap<K, V, BuildHasherDefault<FastHasher>>;

pub(crate) fn fast_map<K, V>() -> FastMap<K, V> {
    HashMap::with_hasher(BuildHasherDefault::default())
}
