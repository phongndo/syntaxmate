use serde::{Deserialize, Serialize};

const MAX_PATTERN_HOTSPOTS: usize = 128;

/// Optional diagnostic counters accumulated by one tokenizer.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct EngineCounters {
    /// Time spent decoding grammars, in microseconds.
    pub grammar_decode_micros: u64,
    /// Number of logical lines processed.
    pub lines_tokenized: u64,
    /// Lines skipped because of resource limits.
    pub lines_skipped: u64,
    /// Lines replayed to reconstruct viewport continuation state.
    pub checkpoint_replay_lines: u64,
    /// Line-result cache hits.
    pub line_cache_hits: u64,
    /// Line-result cache misses.
    pub line_cache_misses: u64,
    /// Line-result cache evictions.
    pub line_cache_evictions: u64,
    /// Continuation-state cache hits.
    pub state_cache_hits: u64,
    /// Continuation-state cache misses.
    pub state_cache_misses: u64,
    /// Candidate-list cache hits.
    pub candidate_list_cache_hits: u64,
    /// Candidate-list cache misses.
    pub candidate_list_cache_misses: u64,
    /// Number of compiled regular expressions.
    #[serde(default)]
    pub regex_compile_count: u64,
    /// Number of constructed pattern-set matchers.
    #[serde(default)]
    pub pattern_set_construction_count: u64,
    /// Number of constructed inline candidate sets.
    #[serde(default)]
    pub inline_candidate_set_construction_count: u64,
    /// Compilation counts grouped by pattern.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pattern_compile_counts: Vec<PatternCompileCount>,
    /// DFA matcher attempts.
    pub regex_dfa_attempts: u64,
    /// Fallback matcher attempts.
    pub regex_fallback_attempts: u64,
    /// Candidate-set searches.
    #[serde(default)]
    pub candidate_searches: u64,
    /// Total patterns considered across candidate searches.
    #[serde(default)]
    pub candidate_patterns_considered: u64,
    /// Successful candidate searches.
    #[serde(default)]
    pub candidate_winners: u64,
    /// Capture matching replays.
    #[serde(default)]
    pub capture_replays: u64,
    /// Pattern prefilter checks.
    pub prefilter_checks: u64,
    /// Prefilter checks that allowed matching.
    pub prefilter_hits: u64,
    /// Matching attempts skipped by the prefilter.
    pub prefilter_skips: u64,
    /// Total fallback execution steps.
    pub fallback_steps_total: u64,
    /// Largest fallback execution step count in one attempt.
    pub fallback_steps_max: u64,
    /// Matching attempts stopped by the fallback budget.
    pub fallback_budget_kills: u64,
    /// Lines whose tokenization exceeded resource limits.
    pub degraded_lines: u64,
    /// Bounded collection of costly pattern execution records.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pattern_hotspots: Vec<PatternHotspot>,
}

/// Diagnostic execution costs for one pattern.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PatternHotspot {
    /// Root grammar scope name.
    pub root_scope: String,
    /// Internal grammar identity, when available.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub grammar_id: Option<u16>,
    /// Internal pattern identity, when available.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pattern_id: Option<u32>,
    /// Name of the matcher used.
    pub engine: String,
    /// Original regex pattern text.
    pub pattern: String,
    /// Number of matching attempts.
    pub attempts: u64,
    /// Number of successful matches.
    pub matches: u64,
    /// Total matching time, in microseconds.
    pub total_micros: u64,
    /// Total fallback execution steps.
    pub fallback_steps_total: u64,
    /// Largest fallback execution step count in one attempt.
    pub fallback_steps_max: u64,
    /// Matching attempts stopped by the fallback budget.
    pub fallback_budget_kills: u64,
    /// Prefilter checks that allowed matching.
    pub prefilter_hits: u64,
    /// Matching attempts skipped by the prefilter.
    pub prefilter_skips: u64,
}

/// Compilation count for one pattern identity.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PatternCompileCount {
    /// Internal grammar identity, when available.
    pub grammar_id: Option<u16>,
    /// Internal pattern identity, when available.
    pub pattern_id: Option<u32>,
    /// Original regex pattern text.
    pub pattern: String,
    /// Number of compilations.
    pub count: u64,
}

impl PatternHotspot {
    pub(crate) fn merge(&mut self, other: Self) {
        self.attempts = self.attempts.saturating_add(other.attempts);
        self.matches = self.matches.saturating_add(other.matches);
        self.total_micros = self.total_micros.saturating_add(other.total_micros);
        self.fallback_steps_total = self
            .fallback_steps_total
            .saturating_add(other.fallback_steps_total);
        self.fallback_steps_max = self.fallback_steps_max.max(other.fallback_steps_max);
        self.fallback_budget_kills = self
            .fallback_budget_kills
            .saturating_add(other.fallback_budget_kills);
        self.prefilter_hits = self.prefilter_hits.saturating_add(other.prefilter_hits);
        self.prefilter_skips = self.prefilter_skips.saturating_add(other.prefilter_skips);
    }
}

impl EngineCounters {
    pub(crate) fn record_line_tokenized(&mut self) {
        self.lines_tokenized = self.lines_tokenized.saturating_add(1);
    }

    pub(crate) fn record_line_skipped(&mut self) {
        self.lines_skipped = self.lines_skipped.saturating_add(1);
    }

    pub(crate) fn record_dfa_attempt(&mut self) {
        self.regex_dfa_attempts = self.regex_dfa_attempts.saturating_add(1);
    }

    pub(crate) fn record_fallback_attempt(&mut self) {
        self.regex_fallback_attempts = self.regex_fallback_attempts.saturating_add(1);
    }

    pub(crate) fn record_candidate_search(&mut self) {
        self.candidate_searches = self.candidate_searches.saturating_add(1);
    }

    pub(crate) fn record_candidate_pattern_considered(&mut self) {
        self.candidate_patterns_considered = self.candidate_patterns_considered.saturating_add(1);
    }

    pub(crate) fn record_candidate_winner(&mut self) {
        self.candidate_winners = self.candidate_winners.saturating_add(1);
    }

    pub(crate) fn record_capture_replay(&mut self) {
        self.capture_replays = self.capture_replays.saturating_add(1);
    }

    pub(crate) fn record_state_cache_hit(&mut self) {
        self.state_cache_hits = self.state_cache_hits.saturating_add(1);
    }

    pub(crate) fn record_state_cache_miss(&mut self) {
        self.state_cache_misses = self.state_cache_misses.saturating_add(1);
    }

    pub(crate) fn record_line_cache_hit(&mut self) {
        self.line_cache_hits = self.line_cache_hits.saturating_add(1);
    }

    pub(crate) fn record_line_cache_miss(&mut self) {
        self.line_cache_misses = self.line_cache_misses.saturating_add(1);
    }

    pub(crate) fn record_line_cache_eviction(&mut self) {
        self.line_cache_evictions = self.line_cache_evictions.saturating_add(1);
    }

    pub(crate) fn record_candidate_list_cache_hit(&mut self) {
        self.candidate_list_cache_hits = self.candidate_list_cache_hits.saturating_add(1);
    }

    pub(crate) fn record_candidate_list_cache_miss(&mut self) {
        self.candidate_list_cache_misses = self.candidate_list_cache_misses.saturating_add(1);
    }

    pub(crate) fn record_regex_compile(
        &mut self,
        grammar_id: Option<u16>,
        pattern_id: Option<u32>,
        pattern: &str,
    ) {
        self.regex_compile_count = self.regex_compile_count.saturating_add(1);
        if let Some(entry) = self.pattern_compile_counts.iter_mut().find(|entry| {
            entry.grammar_id == grammar_id
                && entry.pattern_id == pattern_id
                && entry.pattern == pattern
        }) {
            entry.count = entry.count.saturating_add(1);
        } else {
            self.pattern_compile_counts.push(PatternCompileCount {
                grammar_id,
                pattern_id,
                pattern: pattern.to_owned(),
                count: 1,
            });
        }
    }

    pub(crate) fn record_pattern_set_construction(&mut self) {
        self.pattern_set_construction_count = self.pattern_set_construction_count.saturating_add(1);
    }

    pub(crate) fn record_inline_candidate_set_construction(&mut self) {
        self.inline_candidate_set_construction_count = self
            .inline_candidate_set_construction_count
            .saturating_add(1);
    }

    pub(crate) fn record_prefilter_check(&mut self, may_match: bool) {
        self.prefilter_checks = self.prefilter_checks.saturating_add(1);
        if may_match {
            self.prefilter_hits = self.prefilter_hits.saturating_add(1);
        } else {
            self.prefilter_skips = self.prefilter_skips.saturating_add(1);
        }
    }

    pub(crate) fn record_checkpoint_replay_lines(&mut self, lines: usize) {
        self.checkpoint_replay_lines = self.checkpoint_replay_lines.saturating_add(lines as u64);
    }

    pub(crate) fn record_fallback_steps(&mut self, steps: usize) {
        let steps = steps as u64;
        self.fallback_steps_total = self.fallback_steps_total.saturating_add(steps);
        self.fallback_steps_max = self.fallback_steps_max.max(steps);
    }

    pub(crate) fn record_fallback_budget_kill(&mut self) {
        self.fallback_budget_kills = self.fallback_budget_kills.saturating_add(1);
    }

    pub(crate) fn record_degraded_line(&mut self) {
        self.degraded_lines = self.degraded_lines.saturating_add(1);
    }

    /// Merges counters using saturating totals and per-attempt maxima.
    pub fn merge(&mut self, other: Self) {
        self.grammar_decode_micros = self
            .grammar_decode_micros
            .saturating_add(other.grammar_decode_micros);
        self.lines_tokenized = self.lines_tokenized.saturating_add(other.lines_tokenized);
        self.lines_skipped = self.lines_skipped.saturating_add(other.lines_skipped);
        self.checkpoint_replay_lines = self
            .checkpoint_replay_lines
            .saturating_add(other.checkpoint_replay_lines);
        self.line_cache_hits = self.line_cache_hits.saturating_add(other.line_cache_hits);
        self.line_cache_misses = self
            .line_cache_misses
            .saturating_add(other.line_cache_misses);
        self.line_cache_evictions = self
            .line_cache_evictions
            .saturating_add(other.line_cache_evictions);
        self.state_cache_hits = self.state_cache_hits.saturating_add(other.state_cache_hits);
        self.state_cache_misses = self
            .state_cache_misses
            .saturating_add(other.state_cache_misses);
        self.candidate_list_cache_hits = self
            .candidate_list_cache_hits
            .saturating_add(other.candidate_list_cache_hits);
        self.candidate_list_cache_misses = self
            .candidate_list_cache_misses
            .saturating_add(other.candidate_list_cache_misses);
        self.regex_compile_count = self
            .regex_compile_count
            .saturating_add(other.regex_compile_count);
        self.pattern_set_construction_count = self
            .pattern_set_construction_count
            .saturating_add(other.pattern_set_construction_count);
        self.inline_candidate_set_construction_count = self
            .inline_candidate_set_construction_count
            .saturating_add(other.inline_candidate_set_construction_count);
        for other_count in other.pattern_compile_counts {
            if let Some(count) = self.pattern_compile_counts.iter_mut().find(|count| {
                count.grammar_id == other_count.grammar_id
                    && count.pattern_id == other_count.pattern_id
                    && count.pattern == other_count.pattern
            }) {
                count.count = count.count.saturating_add(other_count.count);
            } else {
                self.pattern_compile_counts.push(other_count);
            }
        }
        self.regex_dfa_attempts = self
            .regex_dfa_attempts
            .saturating_add(other.regex_dfa_attempts);
        self.regex_fallback_attempts = self
            .regex_fallback_attempts
            .saturating_add(other.regex_fallback_attempts);
        self.candidate_searches = self
            .candidate_searches
            .saturating_add(other.candidate_searches);
        self.candidate_patterns_considered = self
            .candidate_patterns_considered
            .saturating_add(other.candidate_patterns_considered);
        self.candidate_winners = self
            .candidate_winners
            .saturating_add(other.candidate_winners);
        self.capture_replays = self.capture_replays.saturating_add(other.capture_replays);
        self.prefilter_checks = self.prefilter_checks.saturating_add(other.prefilter_checks);
        self.prefilter_hits = self.prefilter_hits.saturating_add(other.prefilter_hits);
        self.prefilter_skips = self.prefilter_skips.saturating_add(other.prefilter_skips);
        self.fallback_steps_total = self
            .fallback_steps_total
            .saturating_add(other.fallback_steps_total);
        self.fallback_steps_max = self.fallback_steps_max.max(other.fallback_steps_max);
        self.fallback_budget_kills = self
            .fallback_budget_kills
            .saturating_add(other.fallback_budget_kills);
        self.degraded_lines = self.degraded_lines.saturating_add(other.degraded_lines);
        for hotspot in other.pattern_hotspots {
            self.merge_pattern_hotspot(hotspot);
        }
        self.prune_pattern_hotspots();
    }

    pub(crate) fn merge_pattern_hotspot(&mut self, hotspot: PatternHotspot) {
        if let Some(existing) = self.pattern_hotspots.iter_mut().find(|existing| {
            existing.root_scope == hotspot.root_scope
                && existing.engine == hotspot.engine
                && existing.pattern == hotspot.pattern
        }) {
            existing.merge(hotspot);
        } else {
            self.pattern_hotspots.push(hotspot);
        }
    }

    pub(crate) fn prune_pattern_hotspots(&mut self) {
        self.pattern_hotspots.sort_by(|left, right| {
            right
                .total_micros
                .cmp(&left.total_micros)
                .then_with(|| right.fallback_steps_total.cmp(&left.fallback_steps_total))
                .then_with(|| right.attempts.cmp(&left.attempts))
                .then_with(|| left.root_scope.cmp(&right.root_scope))
                .then_with(|| left.engine.cmp(&right.engine))
                .then_with(|| left.pattern.cmp(&right.pattern))
        });
        self.pattern_hotspots.truncate(MAX_PATTERN_HOTSPOTS);
    }
}
