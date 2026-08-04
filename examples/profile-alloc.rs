//! Allocation and latency profile for cold, warm, and incremental APIs.
//!
//! ```text
//! cargo run --release --example profile-alloc -- rust path/to/source.rs
//! ```

use std::{
    alloc::{GlobalAlloc, Layout, System},
    env, fs,
    hint::black_box,
    sync::atomic::{AtomicU64, Ordering},
    time::Instant,
};

use syntaxmate::{Highlighter, PreparedLanguage, Tokenizer, TokenizerOptions};

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

struct AllocationsPerKib(u64, usize);

impl std::fmt::Display for AllocationsPerKib {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.1 == 0 {
            formatter.write_str("n/a")
        } else {
            write!(formatter, "{:.2}", self.0 as f64 / (self.1 as f64 / 1024.0))
        }
    }
}

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
    let language = args.next().ok_or("usage: profile-alloc LANGUAGE SOURCE")?;
    let path = args.next().ok_or("usage: profile-alloc LANGUAGE SOURCE")?;
    if args.next().is_some() {
        return Err("usage: profile-alloc LANGUAGE SOURCE".into());
    }
    let source = fs::read_to_string(&path)?;

    let before = Stats::now();
    let started = Instant::now();
    let mut tokenizer = Tokenizer::for_bundled_language(&language, TokenizerOptions::default())?;
    report("construct", before, started, 1, source.len());

    let before = Stats::now();
    let started = Instant::now();
    let document = tokenizer.tokenize(black_box(&source));
    let tokens = document
        .lines()
        .iter()
        .map(|line| line.spans().len())
        .sum::<usize>();
    black_box(&document);
    report("tokenize-first", before, started, tokens, source.len());
    drop(document);

    let before = Stats::now();
    let started = Instant::now();
    let document = tokenizer.tokenize(black_box(&source));
    let tokens = document
        .lines()
        .iter()
        .map(|line| line.spans().len())
        .sum::<usize>();
    black_box(&document);
    report("tokenize-warm", before, started, tokens, source.len());
    drop(document);

    let mut tokenizer = Tokenizer::for_bundled_language(&language, TokenizerOptions::default())?;
    let mut state = tokenizer.initial_state();
    let before = Stats::now();
    let started = Instant::now();
    let mut tokens = 0;
    for line in source.split('\n') {
        let output = tokenizer.tokenize_line(black_box(line), &mut state)?;
        tokens += output.tokens().len();
        black_box(output);
    }
    report("incremental", before, started, tokens, source.len());

    let highlighter = Highlighter::bundled()?;
    let mut session = highlighter.session(&language, "github-dark")?;
    let before = Stats::now();
    let started = Instant::now();
    let mut spans = 0;
    for line in source.split('\n') {
        let output = session.highlight_line(black_box(line))?;
        spans += output.spans().len();
        black_box(output);
    }
    report("highlight-lines", before, started, spans, source.len());

    let before = Stats::now();
    let started = Instant::now();
    let prepared = PreparedLanguage::for_bundled_language(&language)?;
    report(
        "prepare-language",
        before,
        started,
        prepared.stats().compiled_pattern_count(),
        source.len(),
    );

    let before = Stats::now();
    let started = Instant::now();
    let mut tokenizer = prepared.tokenizer(TokenizerOptions::default());
    report("prepared-new", before, started, 1, source.len());

    let before = Stats::now();
    let started = Instant::now();
    let document = tokenizer.tokenize(black_box(&source));
    let tokens = document
        .lines()
        .iter()
        .map(|line| line.spans().len())
        .sum::<usize>();
    black_box(&document);
    report("prepared-first", before, started, tokens, source.len());
    drop(document);
    drop(tokenizer);

    let before = Stats::now();
    let started = Instant::now();
    let mut tokenizer = prepared.tokenizer(TokenizerOptions::default());
    report("prepared-new-warm", before, started, 1, source.len());

    let before = Stats::now();
    let started = Instant::now();
    let document = tokenizer.tokenize(black_box(&source));
    let tokens = document
        .lines()
        .iter()
        .map(|line| line.spans().len())
        .sum::<usize>();
    black_box(&document);
    report("prepared-reuse", before, started, tokens, source.len());

    Ok(())
}

fn report(label: &str, before: Stats, started: Instant, items: usize, source_bytes: usize) {
    let elapsed = started.elapsed();
    let stats = Stats::now().since(before);
    let retained = stats.allocated_bytes as i128 - stats.deallocated_bytes as i128;
    let allocations_per_kib = AllocationsPerKib(stats.allocations, source_bytes);
    println!(
        "{label:>15}: {:>8} allocations + {:>6} reallocations, {:>10} bytes allocated, {:>10} bytes retained, {:>8.3} ms, {items} items, {allocations_per_kib} allocs/KiB",
        stats.allocations,
        stats.reallocations,
        stats.allocated_bytes,
        retained,
        elapsed.as_secs_f64() * 1_000.0,
    );
}
