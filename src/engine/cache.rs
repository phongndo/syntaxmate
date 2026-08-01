use std::{
    collections::{HashMap, VecDeque},
    hash::{BuildHasherDefault, Hash, Hasher},
    sync::Arc,
};

use crate::LineTextFingerprint;

use super::{state::StateId, tokenizer::CompactScopedToken};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LineCacheKey {
    pub entry: StateId,
    pub first_line: bool,
    pub fingerprint: LineTextFingerprint,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CachedLine {
    pub(crate) text: Arc<str>,
    pub(crate) tokens: Arc<[CompactScopedToken]>,
    pub(crate) exit: StateId,
}

#[derive(Debug, Clone)]
pub struct LineCache<K, V> {
    entries: HashMap<K, LruEntry<V>, BuildHasherDefault<LineCacheHasher>>,
    capacity: usize,
    tick: u64,
    lru: VecDeque<(K, u64)>,
}

#[derive(Debug, Clone)]
struct LruEntry<V> {
    value: V,
    last_used: u64,
}

#[derive(Debug, Clone)]
struct LineCacheHasher(u64);

impl Default for LineCacheHasher {
    fn default() -> Self {
        Self(0x517c_c1b7_2722_0a95)
    }
}

impl Hasher for LineCacheHasher {
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

    fn write_u8(&mut self, value: u8) {
        self.write_u64(u64::from(value));
    }
}

impl<K, V> LineCache<K, V>
where
    K: Clone + Eq + Hash,
    V: Clone,
{
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: HashMap::with_hasher(BuildHasherDefault::default()),
            capacity,
            tick: 0,
            lru: VecDeque::new(),
        }
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }

    pub fn set_capacity(&mut self, capacity: usize) {
        self.capacity = capacity;
        if capacity == 0 {
            self.clear();
            return;
        }
        while self.entries.len() > self.capacity {
            let Some(oldest) = self.pop_oldest_key() else {
                break;
            };
            self.entries.remove(&oldest);
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn is_enabled(&self) -> bool {
        self.capacity > 0
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.lru.clear();
    }

    pub fn get(&mut self, key: &K) -> Option<V> {
        let last_used = self.next_tick();
        let value = {
            let entry = self.entries.get_mut(key)?;
            entry.last_used = last_used;
            entry.value.clone()
        };
        self.lru.push_back((key.clone(), last_used));
        self.compact_lru_if_needed();
        Some(value)
    }

    pub fn insert(&mut self, key: K, value: V) -> bool {
        if self.capacity == 0 {
            return false;
        }

        let last_used = self.next_tick();
        if let Some(entry) = self.entries.get_mut(&key) {
            entry.value = value;
            entry.last_used = last_used;
            self.lru.push_back((key, last_used));
            self.compact_lru_if_needed();
            return false;
        }

        let mut evicted = false;
        if self.entries.len() >= self.capacity
            && let Some(oldest) = self.pop_oldest_key()
        {
            self.entries.remove(&oldest);
            evicted = true;
        }

        self.entries
            .insert(key.clone(), LruEntry { value, last_used });
        self.lru.push_back((key, last_used));
        self.compact_lru_if_needed();
        evicted
    }

    fn next_tick(&mut self) -> u64 {
        self.tick = self.tick.wrapping_add(1);
        self.tick
    }

    fn pop_oldest_key(&mut self) -> Option<K> {
        while let Some((key, tick)) = self.lru.pop_front() {
            if self
                .entries
                .get(&key)
                .is_some_and(|entry| entry.last_used == tick)
            {
                return Some(key);
            }
        }
        None
    }

    fn compact_lru_if_needed(&mut self) {
        let live = self.entries.len().max(self.capacity).max(16);
        if self.lru.len() <= live.saturating_mul(4) {
            return;
        }
        let mut current = self
            .entries
            .iter()
            .map(|(key, entry)| (key.clone(), entry.last_used))
            .collect::<Vec<_>>();
        current.sort_unstable_by_key(|(_, tick)| *tick);
        self.lru = current.into();
    }
}

#[cfg(test)]
mod tests {
    use super::LineCache;

    #[test]
    fn eviction_remains_exact_lru_after_a_hit() {
        let mut cache = LineCache::new(2);
        assert!(!cache.insert("a", 1));
        assert!(!cache.insert("b", 2));
        assert_eq!(cache.get(&"a"), Some(1));
        assert!(cache.insert("c", 3));
        assert_eq!(cache.get(&"b"), None);
        assert_eq!(cache.get(&"a"), Some(1));
        assert_eq!(cache.get(&"c"), Some(3));
    }
}
