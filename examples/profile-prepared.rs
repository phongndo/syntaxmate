//! Repeated independent-tokenizer profile for prepared-language work.
//!
//! ```text
//! cargo run --release --example profile-prepared -- \
//!   prepared-reuse markdown tests/fixtures/textmate/markdown/stress.md 4
//! ```

use std::{
    alloc::{GlobalAlloc, Layout, System},
    env, fs,
    hint::black_box,
    sync::atomic::{AtomicU64, Ordering},
    time::Instant,
};

use syntaxmate::{PreparedLanguage, Tokenizer, TokenizerOptions};

struct CountingAllocator;

static ALLOCATIONS: AtomicU64 = AtomicU64::new(0);
static DEALLOCATIONS: AtomicU64 = AtomicU64::new(0);
static REALLOCATIONS: AtomicU64 = AtomicU64::new(0);
static ALLOCATED_BYTES: AtomicU64 = AtomicU64::new(0);
static DEALLOCATED_BYTES: AtomicU64 = AtomicU64::new(0);

unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
        ALLOCATED_BYTES.fetch_add(layout.size() as u64, Ordering::Relaxed);
        unsafe { System.alloc(layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
        ALLOCATED_BYTES.fetch_add(layout.size() as u64, Ordering::Relaxed);
        unsafe { System.alloc_zeroed(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        DEALLOCATIONS.fetch_add(1, Ordering::Relaxed);
        DEALLOCATED_BYTES.fetch_add(layout.size() as u64, Ordering::Relaxed);
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, old: Layout, new_size: usize) -> *mut u8 {
        REALLOCATIONS.fetch_add(1, Ordering::Relaxed);
        ALLOCATED_BYTES.fetch_add(new_size as u64, Ordering::Relaxed);
        DEALLOCATED_BYTES.fetch_add(old.size() as u64, Ordering::Relaxed);
        unsafe { System.realloc(ptr, old, new_size) }
    }
}

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

#[derive(Clone, Copy)]
struct Stats {
    allocations: u64,
    deallocations: u64,
    reallocations: u64,
    allocated_bytes: u64,
    deallocated_bytes: u64,
}

impl Stats {
    fn now() -> Self {
        Self {
            allocations: ALLOCATIONS.load(Ordering::Relaxed),
            deallocations: DEALLOCATIONS.load(Ordering::Relaxed),
            reallocations: REALLOCATIONS.load(Ordering::Relaxed),
            allocated_bytes: ALLOCATED_BYTES.load(Ordering::Relaxed),
            deallocated_bytes: DEALLOCATED_BYTES.load(Ordering::Relaxed),
        }
    }

    fn since(self, earlier: Self) -> Self {
        Self {
            allocations: self.allocations - earlier.allocations,
            deallocations: self.deallocations - earlier.deallocations,
            reallocations: self.reallocations - earlier.reallocations,
            allocated_bytes: self.allocated_bytes - earlier.allocated_bytes,
            deallocated_bytes: self.deallocated_bytes - earlier.deallocated_bytes,
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = env::args().skip(1);
    let mode = args
        .next()
        .ok_or("usage: profile-prepared MODE LANGUAGE SOURCE ITERATIONS")?;
    let language = args
        .next()
        .ok_or("usage: profile-prepared MODE LANGUAGE SOURCE ITERATIONS")?;
    let path = args
        .next()
        .ok_or("usage: profile-prepared MODE LANGUAGE SOURCE ITERATIONS")?;
    let iterations = args
        .next()
        .ok_or("usage: profile-prepared MODE LANGUAGE SOURCE ITERATIONS")?
        .parse::<usize>()?;
    if args.next().is_some()
        || !matches!(
            mode.as_str(),
            "direct" | "prepared-total" | "prepared-reuse"
        )
    {
        return Err("mode must be direct, prepared-total, or prepared-reuse".into());
    }
    let source = fs::read_to_string(path)?;

    let total_before = Stats::now();
    let total_started = Instant::now();
    let prepared = if mode == "direct" {
        None
    } else {
        Some(PreparedLanguage::for_bundled_language(&language)?)
    };
    let (before, started) = if mode == "prepared-reuse" {
        (Stats::now(), Instant::now())
    } else {
        (total_before, total_started)
    };
    let mut tokens = 0usize;
    let mut digest = 0xcbf2_9ce4_8422_2325u64;
    for _ in 0..iterations {
        let mut tokenizer = if let Some(prepared) = &prepared {
            prepared.tokenizer(TokenizerOptions::default())
        } else {
            Tokenizer::for_bundled_language(&language, TokenizerOptions::default())?
        };
        let document = tokenizer.tokenize(black_box(&source));
        for line in document.lines() {
            for span in line.spans() {
                tokens += 1;
                digest = hash_u64(digest, span.range().start as u64);
                digest = hash_u64(digest, span.range().end as u64);
                for scope in line.scope_names(span.scope_stack()) {
                    digest = hash_bytes(digest, scope.as_bytes());
                    digest = hash_bytes(digest, &[0xff]);
                }
            }
        }
        black_box(document);
    }
    let elapsed = started.elapsed();
    let stats = Stats::now().since(before);
    let prepared_stats = prepared.as_ref().map(|prepared| prepared.stats());
    println!(
        "{}",
        serde_json::json!({
            "schemaVersion": 1,
            "mode": mode,
            "language": language,
            "iterations": iterations,
            "sourceBytes": source.len(),
            "elapsedNanoseconds": u64::try_from(elapsed.as_nanos()).unwrap_or(u64::MAX),
            "allocations": stats.allocations,
            "reallocations": stats.reallocations,
            "allocatedBytes": stats.allocated_bytes,
            "retainedBytes": stats.allocated_bytes as i128 - stats.deallocated_bytes as i128,
            "tokens": tokens,
            "scopeDigest": format!("{digest:016x}"),
            "prepared": prepared_stats.map(|stats| serde_json::json!({
                "grammarCount": stats.grammar_count(),
                "staticPatternCapacity": stats.static_pattern_capacity(),
                "compiledPatternCount": stats.compiled_pattern_count(),
                "staticPatternByteCapacity": stats.static_pattern_byte_capacity(),
                "staticPatternRetainedBytes": stats.static_pattern_retained_bytes(),
                "staticCandidateCapacity": stats.static_candidate_capacity(),
                "staticCandidateCount": stats.static_candidate_count(),
                "staticCandidateByteCapacity": stats.static_candidate_byte_capacity(),
                "staticCandidateRetainedBytes": stats.static_candidate_retained_bytes(),
            })),
        })
    );
    Ok(())
}

fn hash_u64(hash: u64, value: u64) -> u64 {
    hash_bytes(hash, &value.to_le_bytes())
}

fn hash_bytes(mut hash: u64, bytes: &[u8]) -> u64 {
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}
