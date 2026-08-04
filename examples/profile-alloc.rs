//! Allocation, peak-live-memory, output-digest, and latency profiler.
//!
//! ```text
//! cargo run --release --example profile-alloc -- rust path/to/source.rs
//! cargo run --release --example profile-alloc -- --json rust path/to/source.rs
//! ```

use std::{
    alloc::{GlobalAlloc, Layout, System},
    env, fs,
    hint::black_box,
    ops::Range,
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, Instant},
};

use syntaxmate::{
    HighlightSession, HighlightStatus, Highlighter, IncrementalHighlightedLine, PreparedLanguage,
    TokenizedDocument, TokenizedLine, Tokenizer, TokenizerOptions,
};

struct CountingAllocator;

static ALLOCATIONS: AtomicU64 = AtomicU64::new(0);
static DEALLOCATIONS: AtomicU64 = AtomicU64::new(0);
static REALLOCATIONS: AtomicU64 = AtomicU64::new(0);
static ALLOCATED_BYTES: AtomicU64 = AtomicU64::new(0);
static DEALLOCATED_BYTES: AtomicU64 = AtomicU64::new(0);
static LIVE_BYTES: AtomicU64 = AtomicU64::new(0);
static PEAK_LIVE_BYTES: AtomicU64 = AtomicU64::new(0);

fn record_live_increase(bytes: u64) {
    let live = LIVE_BYTES.fetch_add(bytes, Ordering::Relaxed) + bytes;
    PEAK_LIVE_BYTES.fetch_max(live, Ordering::Relaxed);
}

fn record_live_decrease(bytes: u64) {
    LIVE_BYTES.fetch_sub(bytes, Ordering::Relaxed);
}

unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let allocation = unsafe { System.alloc(layout) };
        if !allocation.is_null() {
            ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
            ALLOCATED_BYTES.fetch_add(layout.size() as u64, Ordering::Relaxed);
            record_live_increase(layout.size() as u64);
        }
        allocation
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        let allocation = unsafe { System.alloc_zeroed(layout) };
        if !allocation.is_null() {
            ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
            ALLOCATED_BYTES.fetch_add(layout.size() as u64, Ordering::Relaxed);
            record_live_increase(layout.size() as u64);
        }
        allocation
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        DEALLOCATIONS.fetch_add(1, Ordering::Relaxed);
        DEALLOCATED_BYTES.fetch_add(layout.size() as u64, Ordering::Relaxed);
        record_live_decrease(layout.size() as u64);
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, old: Layout, new_size: usize) -> *mut u8 {
        let allocation = unsafe { System.realloc(ptr, old, new_size) };
        if !allocation.is_null() {
            REALLOCATIONS.fetch_add(1, Ordering::Relaxed);
            ALLOCATED_BYTES.fetch_add(new_size as u64, Ordering::Relaxed);
            DEALLOCATED_BYTES.fetch_add(old.size() as u64, Ordering::Relaxed);
            if new_size >= old.size() {
                record_live_increase((new_size - old.size()) as u64);
            } else {
                record_live_decrease((old.size() - new_size) as u64);
            }
        }
        allocation
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
    live_bytes: u64,
}

impl Stats {
    fn now() -> Self {
        Self {
            allocations: ALLOCATIONS.load(Ordering::Relaxed),
            deallocations: DEALLOCATIONS.load(Ordering::Relaxed),
            reallocations: REALLOCATIONS.load(Ordering::Relaxed),
            allocated_bytes: ALLOCATED_BYTES.load(Ordering::Relaxed),
            deallocated_bytes: DEALLOCATED_BYTES.load(Ordering::Relaxed),
            live_bytes: LIVE_BYTES.load(Ordering::Relaxed),
        }
    }

    fn begin_phase() -> (Self, Instant) {
        let before = Self::now();
        PEAK_LIVE_BYTES.store(before.live_bytes, Ordering::Relaxed);
        (before, Instant::now())
    }

    fn since(self, earlier: Self) -> PhaseStats {
        PhaseStats {
            allocations: self.allocations - earlier.allocations,
            deallocations: self.deallocations - earlier.deallocations,
            reallocations: self.reallocations - earlier.reallocations,
            allocated_bytes: self.allocated_bytes - earlier.allocated_bytes,
            deallocated_bytes: self.deallocated_bytes - earlier.deallocated_bytes,
            retained_bytes: i128::from(self.live_bytes) - i128::from(earlier.live_bytes),
            peak_retained_bytes: PEAK_LIVE_BYTES
                .load(Ordering::Relaxed)
                .saturating_sub(earlier.live_bytes),
        }
    }
}

#[derive(Clone, Copy)]
struct PhaseStats {
    allocations: u64,
    deallocations: u64,
    reallocations: u64,
    allocated_bytes: u64,
    deallocated_bytes: u64,
    retained_bytes: i128,
    peak_retained_bytes: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct OutputDigests {
    token: u64,
    scope: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct OutputMeasurement {
    items: usize,
    complete: bool,
    digests: OutputDigests,
}

#[derive(Clone, Copy)]
struct StableDigest(u64);

impl StableDigest {
    const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0000_0100_0000_01b3;

    fn new() -> Self {
        Self(Self::OFFSET)
    }

    fn write(&mut self, bytes: &[u8]) {
        for byte in bytes {
            self.0 = (self.0 ^ u64::from(*byte)).wrapping_mul(Self::PRIME);
        }
    }

    fn write_u64(&mut self, value: u64) {
        self.write(&value.to_le_bytes());
    }

    fn finish(self) -> u64 {
        self.0
    }
}

struct OutputDigestBuilder {
    token: StableDigest,
    scope: StableDigest,
}

impl OutputDigestBuilder {
    fn new() -> Self {
        Self {
            token: StableDigest::new(),
            scope: StableDigest::new(),
        }
    }

    fn begin_line(&mut self, line_index: usize) {
        self.token.write(&[0xff]);
        self.token.write_u64(line_index as u64);
        self.scope.write(&[0xff]);
        self.scope.write_u64(line_index as u64);
    }

    fn push<'a>(&mut self, range: Range<usize>, scopes: impl Iterator<Item = &'a str>) {
        self.token.write(&[0x01]);
        self.token.write_u64(range.start as u64);
        self.token.write_u64(range.end as u64);

        self.scope.write(&[0x01]);
        self.scope.write_u64(range.start as u64);
        self.scope.write_u64(range.end as u64);
        for scope in scopes {
            self.scope.write(&[0x02]);
            self.scope.write_u64(scope.len() as u64);
            self.scope.write(scope.as_bytes());
        }
        self.scope.write(&[0x03]);
    }

    fn finish(self) -> OutputDigests {
        OutputDigests {
            token: self.token.finish(),
            scope: self.scope.finish(),
        }
    }
}

struct PhaseReport {
    label: &'static str,
    stats: PhaseStats,
    elapsed: Duration,
    items: usize,
    output: Option<OutputMeasurement>,
}

fn finish_phase(
    label: &'static str,
    before: Stats,
    elapsed: Duration,
    items: usize,
    output: Option<OutputMeasurement>,
) -> PhaseReport {
    let stats = Stats::now().since(before);
    PhaseReport {
        label,
        stats,
        elapsed,
        items,
        output,
    }
}

fn measure_document(document: &TokenizedDocument) -> OutputMeasurement {
    let mut digest = OutputDigestBuilder::new();
    let mut items = 0;
    for (line_index, line) in document.lines().iter().enumerate() {
        digest.begin_line(line_index);
        for span in line.spans() {
            items += 1;
            digest.push(span.range(), line.scope_names(span.scope_stack()));
        }
    }
    OutputMeasurement {
        items,
        complete: document.status() == HighlightStatus::Complete,
        digests: digest.finish(),
    }
}

fn measure_tokenized_line(
    digest: &mut OutputDigestBuilder,
    line_index: usize,
    line: &TokenizedLine,
) -> usize {
    digest.begin_line(line_index);
    for token in line.tokens() {
        digest.push(token.range(), token.scopes());
    }
    line.tokens().len()
}

fn measure_highlighted_line(
    digest: &mut OutputDigestBuilder,
    line_index: usize,
    line: &IncrementalHighlightedLine,
) -> usize {
    digest.begin_line(line_index);
    for span in line.spans() {
        digest.push(span.range(), span.scopes());
    }
    line.spans().len()
}

fn incremental_token_pass(
    tokenizer: &mut Tokenizer,
    source: &str,
    state: &mut syntaxmate::TokenizerState,
) -> syntaxmate::Result<(OutputMeasurement, Duration)> {
    let mut digest = OutputDigestBuilder::new();
    let mut elapsed = Duration::ZERO;
    let mut items = 0;
    let mut complete = true;
    for (line_index, line) in source.split('\n').enumerate() {
        let started = Instant::now();
        let output = tokenizer.tokenize_line(black_box(line), state)?;
        elapsed += started.elapsed();
        items += measure_tokenized_line(&mut digest, line_index, &output);
        complete &= output.status() == HighlightStatus::Complete;
        black_box(output);
    }
    Ok((
        OutputMeasurement {
            items,
            complete,
            digests: digest.finish(),
        },
        elapsed,
    ))
}

fn incremental_highlight_pass(
    session: &mut HighlightSession,
    source: &str,
) -> syntaxmate::Result<(OutputMeasurement, Duration)> {
    let mut digest = OutputDigestBuilder::new();
    let mut elapsed = Duration::ZERO;
    let mut items = 0;
    let mut complete = true;
    for (line_index, line) in source.split('\n').enumerate() {
        let started = Instant::now();
        let output = session.highlight_line(black_box(line))?;
        elapsed += started.elapsed();
        items += measure_highlighted_line(&mut digest, line_index, &output);
        complete &= output.status() == HighlightStatus::Complete;
        black_box(output);
    }
    Ok((
        OutputMeasurement {
            items,
            complete,
            digests: digest.finish(),
        },
        elapsed,
    ))
}

fn require_same_output(
    first_label: &str,
    first: OutputMeasurement,
    second_label: &str,
    second: OutputMeasurement,
) -> Result<(), Box<dyn std::error::Error>> {
    if first != second {
        return Err(format!(
            "output mismatch between {first_label} and {second_label}: {first:?} != {second:?}"
        )
        .into());
    }
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut json = false;
    let mut positional = Vec::new();
    for argument in env::args().skip(1) {
        if argument == "--json" {
            json = true;
        } else if argument.starts_with("--") {
            return Err(format!("unexpected option {argument}").into());
        } else {
            positional.push(argument);
        }
    }
    if positional.len() != 2 {
        return Err("usage: profile-alloc [--json] LANGUAGE SOURCE".into());
    }
    let language = &positional[0];
    let source = fs::read_to_string(&positional[1])?;
    let mut reports = Vec::new();

    let (before, started) = Stats::begin_phase();
    let mut tokenizer = Tokenizer::for_bundled_language(language, TokenizerOptions::default())?;
    reports.push(finish_phase(
        "construct",
        before,
        started.elapsed(),
        1,
        None,
    ));

    let (before, started) = Stats::begin_phase();
    let document = tokenizer.tokenize(black_box(&source));
    let elapsed = started.elapsed();
    let tokenize_first = measure_document(&document);
    black_box(&document);
    reports.push(finish_phase(
        "tokenize-first",
        before,
        elapsed,
        tokenize_first.items,
        Some(tokenize_first),
    ));
    drop(document);

    let (before, started) = Stats::begin_phase();
    let document = tokenizer.tokenize(black_box(&source));
    let elapsed = started.elapsed();
    let tokenize_warm = measure_document(&document);
    black_box(&document);
    reports.push(finish_phase(
        "tokenize-warm",
        before,
        elapsed,
        tokenize_warm.items,
        Some(tokenize_warm),
    ));
    drop(document);
    require_same_output(
        "tokenize-first",
        tokenize_first,
        "tokenize-warm",
        tokenize_warm,
    )?;

    let mut tokenizer = Tokenizer::for_bundled_language(language, TokenizerOptions::default())?;
    let initial_state = tokenizer.initial_state();
    let mut state = initial_state.clone();
    let (before, _) = Stats::begin_phase();
    let (incremental_first, elapsed) = incremental_token_pass(&mut tokenizer, &source, &mut state)?;
    reports.push(finish_phase(
        "incremental-first",
        before,
        elapsed,
        incremental_first.items,
        Some(incremental_first),
    ));

    state = initial_state;
    let (before, _) = Stats::begin_phase();
    let (incremental_warm, elapsed) = incremental_token_pass(&mut tokenizer, &source, &mut state)?;
    reports.push(finish_phase(
        "incremental-warm",
        before,
        elapsed,
        incremental_warm.items,
        Some(incremental_warm),
    ));
    require_same_output(
        "incremental-first",
        incremental_first,
        "incremental-warm",
        incremental_warm,
    )?;

    let highlighter = Highlighter::bundled()?;
    let mut session = highlighter.session(language, "github-dark")?;
    let (before, _) = Stats::begin_phase();
    let (highlight_first, elapsed) = incremental_highlight_pass(&mut session, &source)?;
    reports.push(finish_phase(
        "highlight-lines-first",
        before,
        elapsed,
        highlight_first.items,
        Some(highlight_first),
    ));

    session.reset();
    let (before, _) = Stats::begin_phase();
    let (highlight_warm, elapsed) = incremental_highlight_pass(&mut session, &source)?;
    reports.push(finish_phase(
        "highlight-lines-warm",
        before,
        elapsed,
        highlight_warm.items,
        Some(highlight_warm),
    ));
    require_same_output(
        "highlight-lines-first",
        highlight_first,
        "highlight-lines-warm",
        highlight_warm,
    )?;

    let (before, started) = Stats::begin_phase();
    let prepared = PreparedLanguage::for_bundled_language(language)?;
    reports.push(finish_phase(
        "prepare-language",
        before,
        started.elapsed(),
        prepared.stats().compiled_pattern_count(),
        None,
    ));

    let (before, started) = Stats::begin_phase();
    let mut tokenizer = prepared.tokenizer(TokenizerOptions::default());
    reports.push(finish_phase(
        "prepared-new",
        before,
        started.elapsed(),
        1,
        None,
    ));

    let (before, started) = Stats::begin_phase();
    let document = tokenizer.tokenize(black_box(&source));
    let elapsed = started.elapsed();
    let prepared_first = measure_document(&document);
    black_box(&document);
    reports.push(finish_phase(
        "prepared-first",
        before,
        elapsed,
        prepared_first.items,
        Some(prepared_first),
    ));
    drop(document);
    drop(tokenizer);

    let (before, started) = Stats::begin_phase();
    let mut tokenizer = prepared.tokenizer(TokenizerOptions::default());
    reports.push(finish_phase(
        "prepared-new-warm",
        before,
        started.elapsed(),
        1,
        None,
    ));

    let (before, started) = Stats::begin_phase();
    let document = tokenizer.tokenize(black_box(&source));
    let elapsed = started.elapsed();
    let prepared_reuse = measure_document(&document);
    black_box(&document);
    reports.push(finish_phase(
        "prepared-reuse",
        before,
        elapsed,
        prepared_reuse.items,
        Some(prepared_reuse),
    ));
    require_same_output(
        "prepared-first",
        prepared_first,
        "prepared-reuse",
        prepared_reuse,
    )?;

    if json {
        print_json_report(language, source.len(), &reports)?;
    } else {
        for report in &reports {
            print_human_report(report, source.len());
        }
    }
    Ok(())
}

fn print_human_report(report: &PhaseReport, source_bytes: usize) {
    let stats = report.stats;
    let retained = stats.retained_bytes;
    let allocations_per_kib =
        AllocationsPerKib(stats.allocations + stats.reallocations, source_bytes);
    let digests = report.output.map_or_else(
        || "no output digest".to_owned(),
        |output| {
            format!(
                "token {:016x}, scopes {:016x}, complete {}",
                output.digests.token, output.digests.scope, output.complete
            )
        },
    );
    println!(
        "{:>22}: {:>8} allocations + {:>6} reallocations, {:>10} bytes allocated, \
         {:>10} bytes retained, {:>10} bytes peak retained, {:>8.3} ms, {} items, \
         {} allocation calls/KiB, {digests}",
        report.label,
        stats.allocations,
        stats.reallocations,
        stats.allocated_bytes,
        retained,
        stats.peak_retained_bytes,
        report.elapsed.as_secs_f64() * 1_000.0,
        report.items,
        allocations_per_kib,
    );
}

fn print_json_report(
    language: &str,
    source_bytes: usize,
    reports: &[PhaseReport],
) -> Result<(), Box<dyn std::error::Error>> {
    let phases = reports
        .iter()
        .map(|report| {
            let stats = report.stats;
            let output = report.output;
            (
                report.label.to_owned(),
                serde_json::json!({
                    "allocations": stats.allocations,
                    "deallocations": stats.deallocations,
                    "reallocations": stats.reallocations,
                    "allocationCalls": stats.allocations + stats.reallocations,
                    "allocatedBytes": stats.allocated_bytes,
                    "deallocatedBytes": stats.deallocated_bytes,
                    "retainedBytes": i64::try_from(stats.retained_bytes).unwrap_or_else(|_| {
                        if stats.retained_bytes.is_negative() { i64::MIN } else { i64::MAX }
                    }),
                    "peakRetainedBytes": stats.peak_retained_bytes,
                    "elapsedNanoseconds": u64::try_from(report.elapsed.as_nanos()).unwrap_or(u64::MAX),
                    "items": report.items,
                    "complete": output.map(|output| output.complete),
                    "tokenDigest": output.map(|output| format!("{:016x}", output.digests.token)),
                    "scopeDigest": output.map(|output| format!("{:016x}", output.digests.scope)),
                }),
            )
        })
        .collect::<serde_json::Map<_, _>>();
    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "schemaVersion": 1,
            "language": language,
            "sourceBytes": source_bytes,
            "phases": phases,
        }))?
    );
    Ok(())
}
