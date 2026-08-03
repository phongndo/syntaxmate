//! Deterministic binary encoding for immutable compiled TextMate grammars.
//!
//! The bundle compiler is a separate binary crate and includes this module,
//! `grammar`, and `state` by path. Keep this codec independent of the rest of
//! the runtime so asset generation and runtime decoding always use exactly the
//! same format implementation.

use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
    sync::{Arc, OnceLock},
};

use super::{
    grammar::{
        CaptureEntry, CaptureSpec, CompiledGrammar, GrammarMetadata, Injection, InjectionPriority,
        Rule, RuleBody, RuleRef,
    },
    state::{GrammarId, PatternId, RuleId, ScopeId},
};

const MAGIC: &[u8; 4] = b"CGIR";
const VERSION: u16 = 1;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum GrammarIrError {
    TooShort,
    BadMagic,
    UnsupportedVersion(u16),
    BadUtf8,
    BadStringId(u32),
    BadPatternId(u32),
    BadScopeId(u32),
    BadRuleId(u32),
    InvalidTag { kind: &'static str, tag: u8 },
    InvalidBoolean(u8),
    CountOutOfBounds(&'static str),
    ValueTooLarge(&'static str),
    Malformed(&'static str),
    TrailingBytes,
}

impl fmt::Display for GrammarIrError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooShort => formatter.write_str("compiled grammar IR is truncated"),
            Self::BadMagic => formatter.write_str("compiled grammar IR has bad magic"),
            Self::UnsupportedVersion(version) => {
                write!(
                    formatter,
                    "unsupported compiled grammar IR version {version}"
                )
            }
            Self::BadUtf8 => formatter.write_str("compiled grammar IR contains invalid UTF-8"),
            Self::BadStringId(id) => {
                write!(formatter, "compiled grammar IR has bad string id {id}")
            }
            Self::BadPatternId(id) => {
                write!(formatter, "compiled grammar IR has bad pattern id {id}")
            }
            Self::BadScopeId(id) => write!(formatter, "compiled grammar IR has bad scope id {id}"),
            Self::BadRuleId(id) => write!(formatter, "compiled grammar IR has bad rule id {id}"),
            Self::InvalidTag { kind, tag } => {
                write!(
                    formatter,
                    "compiled grammar IR has invalid {kind} tag {tag}"
                )
            }
            Self::InvalidBoolean(value) => {
                write!(formatter, "compiled grammar IR has invalid boolean {value}")
            }
            Self::CountOutOfBounds(section) => {
                write!(
                    formatter,
                    "compiled grammar IR {section} count is out of bounds"
                )
            }
            Self::ValueTooLarge(value) => {
                write!(formatter, "compiled grammar IR {value} exceeds u32")
            }
            Self::Malformed(message) => {
                write!(formatter, "malformed compiled grammar IR: {message}")
            }
            Self::TrailingBytes => formatter.write_str("compiled grammar IR has trailing bytes"),
        }
    }
}

impl std::error::Error for GrammarIrError {}

/// Encode one grammar without its closure-local `GrammarId`.
///
/// The grammar's existing string table remains in insertion order because
/// `StringId` values are visible to internal diagnostics. Strings needed only
/// by owned metadata/maps are appended in lexical order.
pub(crate) fn encode_compiled_grammar(
    grammar: &CompiledGrammar,
) -> Result<Vec<u8>, GrammarIrError> {
    let strings = StringTable::from_grammar(grammar)?;
    let mut out = Vec::new();
    out.extend_from_slice(MAGIC);
    write_u16(&mut out, VERSION);
    write_u16(&mut out, 0);
    write_len(&mut out, strings.values.len(), "string count")?;
    write_len(&mut out, grammar.string_names.len(), "grammar string count")?;
    for value in &strings.values {
        write_len(&mut out, value.len(), "string length")?;
        out.extend_from_slice(value.as_bytes());
    }

    write_string(&mut out, &strings, &grammar.scope_name);
    write_metadata(&mut out, &strings, &grammar.metadata)?;
    write_string_vec(&mut out, &strings, &grammar.patterns)?;
    write_arc_string_vec(&mut out, &strings, &grammar.scope_names)?;

    write_len(&mut out, grammar.rules.len(), "rule count")?;
    for rule in &grammar.rules {
        write_rule(&mut out, &strings, rule)?;
    }
    write_repository(&mut out, &strings, &grammar.repository)?;
    write_rule_refs(&mut out, &strings, &grammar.top_level)?;
    write_len(&mut out, grammar.injections.len(), "injection count")?;
    for injection in &grammar.injections {
        write_injection(&mut out, &strings, injection)?;
    }
    Ok(out)
}

/// Decode one grammar and assign its closure-local `GrammarId`.
pub(crate) fn decode_compiled_grammar(
    id: GrammarId,
    bytes: &[u8],
) -> Result<CompiledGrammar, GrammarIrError> {
    let mut cursor = Cursor::new(bytes);
    if cursor.bytes(4)? != MAGIC {
        return Err(GrammarIrError::BadMagic);
    }
    let version = cursor.u16()?;
    if version != VERSION {
        return Err(GrammarIrError::UnsupportedVersion(version));
    }
    if cursor.u16()? != 0 {
        return Err(GrammarIrError::Malformed("nonzero header flags"));
    }
    let string_count = cursor.count(1, "string table")?;
    let grammar_string_count = cursor.u32()? as usize;
    if grammar_string_count > string_count {
        return Err(GrammarIrError::Malformed(
            "grammar string count exceeds string table",
        ));
    }
    let mut strings = Vec::with_capacity(string_count);
    for _ in 0..string_count {
        let len = cursor.u32()? as usize;
        let value = std::str::from_utf8(cursor.bytes(len)?).map_err(|_| GrammarIrError::BadUtf8)?;
        strings.push(Arc::<str>::from(value));
    }

    let scope_name = read_string(&mut cursor, &strings)?;
    let metadata = read_metadata(&mut cursor, &strings)?;
    let patterns = read_string_vec(&mut cursor, &strings)?;
    let scope_names = read_arc_string_vec(&mut cursor, &strings)?;
    let pattern_count = patterns.len();
    let scope_count = scope_names.len();

    let rule_count = cursor.count(3, "rules")?;
    let mut rules = Vec::with_capacity(rule_count);
    for _ in 0..rule_count {
        rules.push(read_rule(
            &mut cursor,
            &strings,
            pattern_count,
            scope_count,
            rule_count,
        )?);
    }
    for (index, rule) in rules.iter().enumerate() {
        if rule.id.0 as usize != index {
            return Err(GrammarIrError::Malformed("rule ids are not dense"));
        }
    }
    let repository = read_repository(&mut cursor, &strings, scope_count, rule_count)?;
    let top_level = read_rule_refs(&mut cursor, &strings, scope_count, rule_count)?;
    let injection_count = cursor.count(4, "injections")?;
    let mut injections = Vec::with_capacity(injection_count);
    for _ in 0..injection_count {
        injections.push(read_injection(
            &mut cursor,
            &strings,
            scope_count,
            rule_count,
        )?);
    }
    cursor.finish()?;

    Ok(CompiledGrammar {
        id,
        scope_name,
        metadata,
        string_names: strings[..grammar_string_count].to_vec(),
        patterns,
        rules,
        repository,
        top_level,
        injections,
        scope_names,
    })
}

struct StringTable {
    values: Vec<String>,
    indexes: BTreeMap<String, u32>,
}

impl StringTable {
    fn from_grammar(grammar: &CompiledGrammar) -> Result<Self, GrammarIrError> {
        let mut values = Vec::with_capacity(grammar.string_names.len());
        let mut indexes = BTreeMap::new();
        for value in &grammar.string_names {
            let id = u32::try_from(values.len())
                .map_err(|_| GrammarIrError::ValueTooLarge("string count"))?;
            indexes.entry(value.to_string()).or_insert(id);
            values.push(value.to_string());
        }

        let mut additional = BTreeSet::new();
        collect_grammar_strings(grammar, &mut additional);
        for value in additional {
            if indexes.contains_key(&value) {
                continue;
            }
            let id = u32::try_from(values.len())
                .map_err(|_| GrammarIrError::ValueTooLarge("string count"))?;
            indexes.insert(value.clone(), id);
            values.push(value);
        }
        Ok(Self { values, indexes })
    }

    fn id(&self, value: &str) -> u32 {
        *self
            .indexes
            .get(value)
            .expect("all compiled grammar strings collected before encoding")
    }
}

fn collect_grammar_strings(grammar: &CompiledGrammar, out: &mut BTreeSet<String>) {
    collect_string(&grammar.scope_name, out);
    collect_optional_string(grammar.metadata.display_name.as_deref(), out);
    collect_optional_string(grammar.metadata.name.as_deref(), out);
    collect_strings(&grammar.metadata.file_types, out);
    collect_optional_string(grammar.metadata.first_line_match.as_deref(), out);
    collect_optional_string(grammar.metadata.injection_selector.as_deref(), out);
    collect_strings(&grammar.metadata.inject_to, out);
    collect_strings(&grammar.patterns, out);
    for value in &grammar.scope_names {
        collect_string(value, out);
    }
    for rule in &grammar.rules {
        for (name, alias) in &rule.local_repository {
            collect_string(name, out);
            collect_string(alias, out);
        }
        collect_rule_body_strings(&rule.body, out);
    }
    for (name, rule_ref) in &grammar.repository {
        collect_string(name, out);
        collect_rule_ref_strings(rule_ref, out);
    }
    for rule_ref in &grammar.top_level {
        collect_rule_ref_strings(rule_ref, out);
    }
    for injection in &grammar.injections {
        collect_string(&injection.selector, out);
        collect_string(&injection.selector_body, out);
        for rule_ref in &injection.patterns {
            collect_rule_ref_strings(rule_ref, out);
        }
    }
}

fn collect_rule_body_strings(body: &RuleBody, out: &mut BTreeSet<String>) {
    match body {
        RuleBody::Match { captures, .. } => collect_capture_strings(captures, out),
        RuleBody::BeginEnd {
            begin_captures,
            end_captures,
            patterns,
            ..
        } => {
            collect_capture_strings(begin_captures, out);
            collect_capture_strings(end_captures, out);
            for rule_ref in patterns {
                collect_rule_ref_strings(rule_ref, out);
            }
        }
        RuleBody::BeginWhile {
            begin_captures,
            while_captures,
            patterns,
            ..
        } => {
            collect_capture_strings(begin_captures, out);
            collect_capture_strings(while_captures, out);
            for rule_ref in patterns {
                collect_rule_ref_strings(rule_ref, out);
            }
        }
        RuleBody::IncludeOnly { patterns } => {
            for rule_ref in patterns {
                collect_rule_ref_strings(rule_ref, out);
            }
        }
    }
}

fn collect_capture_strings(captures: &CaptureSpec, out: &mut BTreeSet<String>) {
    for capture in captures.entries.values() {
        for rule_ref in &capture.patterns {
            collect_rule_ref_strings(rule_ref, out);
        }
    }
}

fn collect_rule_ref_strings(rule_ref: &RuleRef, out: &mut BTreeSet<String>) {
    match rule_ref {
        RuleRef::Repository(name) => collect_string(name, out),
        RuleRef::External { repository, .. } => {
            collect_optional_string(repository.as_deref(), out);
        }
        RuleRef::Rule(_) | RuleRef::SelfRef | RuleRef::BaseRef => {}
    }
}

fn collect_string(value: &str, out: &mut BTreeSet<String>) {
    out.insert(value.to_owned());
}

fn collect_optional_string(value: Option<&str>, out: &mut BTreeSet<String>) {
    if let Some(value) = value {
        collect_string(value, out);
    }
}

fn collect_strings(values: &[String], out: &mut BTreeSet<String>) {
    for value in values {
        collect_string(value, out);
    }
}

fn write_metadata(
    out: &mut Vec<u8>,
    strings: &StringTable,
    metadata: &GrammarMetadata,
) -> Result<(), GrammarIrError> {
    write_optional_string(out, strings, metadata.display_name.as_deref());
    write_optional_string(out, strings, metadata.name.as_deref());
    write_string_vec(out, strings, &metadata.file_types)?;
    write_optional_string(out, strings, metadata.first_line_match.as_deref());
    write_optional_string(out, strings, metadata.injection_selector.as_deref());
    write_string_vec(out, strings, &metadata.inject_to)
}

fn read_metadata(
    cursor: &mut Cursor<'_>,
    strings: &[Arc<str>],
) -> Result<GrammarMetadata, GrammarIrError> {
    Ok(GrammarMetadata {
        display_name: read_optional_string(cursor, strings)?,
        name: read_optional_string(cursor, strings)?,
        file_types: read_string_vec(cursor, strings)?,
        first_line_match: read_optional_string(cursor, strings)?,
        injection_selector: read_optional_string(cursor, strings)?,
        inject_to: read_string_vec(cursor, strings)?,
    })
}

fn write_rule(out: &mut Vec<u8>, strings: &StringTable, rule: &Rule) -> Result<(), GrammarIrError> {
    write_u32(out, rule.id.0);
    write_len(out, rule.local_repository.len(), "local repository count")?;
    for (name, alias) in &rule.local_repository {
        write_string(out, strings, name);
        write_string(out, strings, alias);
    }
    match &rule.body {
        RuleBody::Match {
            pattern,
            captures,
            name,
        } => {
            write_u8(out, 0);
            write_u32(out, pattern.0);
            write_captures(out, strings, captures)?;
            write_optional_scope(out, *name);
        }
        RuleBody::BeginEnd {
            begin,
            end,
            begin_captures,
            end_captures,
            name,
            content_name,
            apply_end_pattern_last,
            patterns,
        } => {
            write_u8(out, 1);
            write_u32(out, begin.0);
            write_u32(out, end.0);
            write_captures(out, strings, begin_captures)?;
            write_captures(out, strings, end_captures)?;
            write_optional_scope(out, *name);
            write_optional_scope(out, *content_name);
            write_u8(out, u8::from(*apply_end_pattern_last));
            write_rule_refs(out, strings, patterns)?;
        }
        RuleBody::BeginWhile {
            begin,
            while_pattern,
            begin_captures,
            while_captures,
            name,
            content_name,
            patterns,
        } => {
            write_u8(out, 2);
            write_u32(out, begin.0);
            write_u32(out, while_pattern.0);
            write_captures(out, strings, begin_captures)?;
            write_captures(out, strings, while_captures)?;
            write_optional_scope(out, *name);
            write_optional_scope(out, *content_name);
            write_rule_refs(out, strings, patterns)?;
        }
        RuleBody::IncludeOnly { patterns } => {
            write_u8(out, 3);
            write_rule_refs(out, strings, patterns)?;
        }
    }
    Ok(())
}

fn read_rule(
    cursor: &mut Cursor<'_>,
    strings: &[Arc<str>],
    pattern_count: usize,
    scope_count: usize,
    rule_count: usize,
) -> Result<Rule, GrammarIrError> {
    let id = RuleId(cursor.u32()?);
    if id.0 as usize >= rule_count {
        return Err(GrammarIrError::BadRuleId(id.0));
    }
    let local_count = cursor.count(2, "local repository")?;
    let mut local_repository = BTreeMap::new();
    for _ in 0..local_count {
        let name = read_string(cursor, strings)?;
        let alias = read_string(cursor, strings)?;
        if local_repository.insert(name, alias).is_some() {
            return Err(GrammarIrError::Malformed("duplicate local repository key"));
        }
    }
    let tag = cursor.u8()?;
    let body = match tag {
        0 => RuleBody::Match {
            pattern: read_pattern(cursor, pattern_count)?,
            captures: read_captures(cursor, strings, scope_count, rule_count)?,
            name: read_optional_scope(cursor, scope_count)?,
        },
        1 => RuleBody::BeginEnd {
            begin: read_pattern(cursor, pattern_count)?,
            end: read_pattern(cursor, pattern_count)?,
            begin_captures: read_captures(cursor, strings, scope_count, rule_count)?,
            end_captures: read_captures(cursor, strings, scope_count, rule_count)?,
            name: read_optional_scope(cursor, scope_count)?,
            content_name: read_optional_scope(cursor, scope_count)?,
            apply_end_pattern_last: cursor.boolean()?,
            patterns: read_rule_refs(cursor, strings, scope_count, rule_count)?,
        },
        2 => RuleBody::BeginWhile {
            begin: read_pattern(cursor, pattern_count)?,
            while_pattern: read_pattern(cursor, pattern_count)?,
            begin_captures: read_captures(cursor, strings, scope_count, rule_count)?,
            while_captures: read_captures(cursor, strings, scope_count, rule_count)?,
            name: read_optional_scope(cursor, scope_count)?,
            content_name: read_optional_scope(cursor, scope_count)?,
            patterns: read_rule_refs(cursor, strings, scope_count, rule_count)?,
        },
        3 => RuleBody::IncludeOnly {
            patterns: read_rule_refs(cursor, strings, scope_count, rule_count)?,
        },
        tag => return Err(GrammarIrError::InvalidTag { kind: "rule", tag }),
    };
    Ok(Rule {
        id,
        local_repository,
        body,
    })
}

fn write_captures(
    out: &mut Vec<u8>,
    strings: &StringTable,
    captures: &CaptureSpec,
) -> Result<(), GrammarIrError> {
    write_len(out, captures.entries.len(), "capture count")?;
    for (group, capture) in &captures.entries {
        write_u32(out, *group);
        write_optional_scope(out, capture.name);
        write_rule_refs(out, strings, &capture.patterns)?;
    }
    Ok(())
}

fn read_captures(
    cursor: &mut Cursor<'_>,
    strings: &[Arc<str>],
    scope_count: usize,
    rule_count: usize,
) -> Result<Arc<CaptureSpec>, GrammarIrError> {
    let count = cursor.count(3, "captures")?;
    if count == 0 {
        static EMPTY: OnceLock<Arc<CaptureSpec>> = OnceLock::new();
        return Ok(Arc::clone(
            EMPTY.get_or_init(|| Arc::new(CaptureSpec::default())),
        ));
    }
    let mut entries = BTreeMap::new();
    for _ in 0..count {
        let group = cursor.u32()?;
        let entry = CaptureEntry {
            name: read_optional_scope(cursor, scope_count)?,
            patterns: read_rule_refs(cursor, strings, scope_count, rule_count)?,
        };
        if entries.insert(group, entry).is_some() {
            return Err(GrammarIrError::Malformed("duplicate capture group"));
        }
    }
    Ok(Arc::new(CaptureSpec { entries }))
}

fn write_repository(
    out: &mut Vec<u8>,
    strings: &StringTable,
    repository: &BTreeMap<String, RuleRef>,
) -> Result<(), GrammarIrError> {
    write_len(out, repository.len(), "repository count")?;
    for (name, rule_ref) in repository {
        write_string(out, strings, name);
        write_rule_ref(out, strings, rule_ref);
    }
    Ok(())
}

fn read_repository(
    cursor: &mut Cursor<'_>,
    strings: &[Arc<str>],
    scope_count: usize,
    rule_count: usize,
) -> Result<BTreeMap<String, RuleRef>, GrammarIrError> {
    let count = cursor.count(2, "repository")?;
    let mut repository = BTreeMap::new();
    for _ in 0..count {
        let name = read_string(cursor, strings)?;
        let rule_ref = read_rule_ref(cursor, strings, scope_count, rule_count)?;
        if repository.insert(name, rule_ref).is_some() {
            return Err(GrammarIrError::Malformed("duplicate repository key"));
        }
    }
    Ok(repository)
}

fn write_rule_refs(
    out: &mut Vec<u8>,
    strings: &StringTable,
    rule_refs: &[RuleRef],
) -> Result<(), GrammarIrError> {
    write_len(out, rule_refs.len(), "rule reference count")?;
    for rule_ref in rule_refs {
        write_rule_ref(out, strings, rule_ref);
    }
    Ok(())
}

fn read_rule_refs(
    cursor: &mut Cursor<'_>,
    strings: &[Arc<str>],
    scope_count: usize,
    rule_count: usize,
) -> Result<Vec<RuleRef>, GrammarIrError> {
    let count = cursor.count(1, "rule references")?;
    let mut rule_refs = Vec::with_capacity(count);
    for _ in 0..count {
        rule_refs.push(read_rule_ref(cursor, strings, scope_count, rule_count)?);
    }
    Ok(rule_refs)
}

fn write_rule_ref(out: &mut Vec<u8>, strings: &StringTable, rule_ref: &RuleRef) {
    match rule_ref {
        RuleRef::Rule(id) => {
            write_u8(out, 0);
            write_u32(out, id.0);
        }
        RuleRef::Repository(name) => {
            write_u8(out, 1);
            write_string(out, strings, name);
        }
        RuleRef::SelfRef => write_u8(out, 2),
        RuleRef::BaseRef => write_u8(out, 3),
        RuleRef::External { scope, repository } => {
            write_u8(out, 4);
            write_u32(out, scope.0);
            write_optional_string(out, strings, repository.as_deref());
        }
    }
}

fn read_rule_ref(
    cursor: &mut Cursor<'_>,
    strings: &[Arc<str>],
    scope_count: usize,
    rule_count: usize,
) -> Result<RuleRef, GrammarIrError> {
    match cursor.u8()? {
        0 => {
            let id = cursor.u32()?;
            if id as usize >= rule_count {
                return Err(GrammarIrError::BadRuleId(id));
            }
            Ok(RuleRef::Rule(RuleId(id)))
        }
        1 => Ok(RuleRef::Repository(read_string(cursor, strings)?)),
        2 => Ok(RuleRef::SelfRef),
        3 => Ok(RuleRef::BaseRef),
        4 => {
            let scope = cursor.u32()?;
            if scope as usize >= scope_count {
                return Err(GrammarIrError::BadScopeId(scope));
            }
            Ok(RuleRef::External {
                scope: ScopeId(scope),
                repository: read_optional_string(cursor, strings)?,
            })
        }
        tag => Err(GrammarIrError::InvalidTag {
            kind: "rule reference",
            tag,
        }),
    }
}

fn write_injection(
    out: &mut Vec<u8>,
    strings: &StringTable,
    injection: &Injection,
) -> Result<(), GrammarIrError> {
    write_string(out, strings, &injection.selector);
    write_string(out, strings, &injection.selector_body);
    write_u8(
        out,
        match injection.priority {
            InjectionPriority::Left => 0,
            InjectionPriority::Right => 1,
        },
    );
    write_rule_refs(out, strings, &injection.patterns)
}

fn read_injection(
    cursor: &mut Cursor<'_>,
    strings: &[Arc<str>],
    scope_count: usize,
    rule_count: usize,
) -> Result<Injection, GrammarIrError> {
    let selector = read_string(cursor, strings)?;
    let selector_body = read_string(cursor, strings)?;
    let priority = match cursor.u8()? {
        0 => InjectionPriority::Left,
        1 => InjectionPriority::Right,
        tag => {
            return Err(GrammarIrError::InvalidTag {
                kind: "injection priority",
                tag,
            });
        }
    };
    let patterns = read_rule_refs(cursor, strings, scope_count, rule_count)?;
    Ok(Injection {
        selector,
        selector_body,
        priority,
        patterns,
    })
}

fn write_string(out: &mut Vec<u8>, strings: &StringTable, value: &str) {
    write_u32(out, strings.id(value));
}

fn read_string(cursor: &mut Cursor<'_>, strings: &[Arc<str>]) -> Result<String, GrammarIrError> {
    let id = cursor.u32()?;
    strings
        .get(id as usize)
        .map(|value| value.to_string())
        .ok_or(GrammarIrError::BadStringId(id))
}

fn write_optional_string(out: &mut Vec<u8>, strings: &StringTable, value: Option<&str>) {
    write_u32(out, value.map_or(0, |value| strings.id(value) + 1));
}

fn read_optional_string(
    cursor: &mut Cursor<'_>,
    strings: &[Arc<str>],
) -> Result<Option<String>, GrammarIrError> {
    let encoded = cursor.u32()?;
    if encoded == 0 {
        Ok(None)
    } else {
        let id = encoded - 1;
        strings
            .get(id as usize)
            .map(|value| Some(value.to_string()))
            .ok_or(GrammarIrError::BadStringId(id))
    }
}

fn write_string_vec(
    out: &mut Vec<u8>,
    strings: &StringTable,
    values: &[String],
) -> Result<(), GrammarIrError> {
    write_len(out, values.len(), "string vector count")?;
    for value in values {
        write_string(out, strings, value);
    }
    Ok(())
}

fn read_string_vec(
    cursor: &mut Cursor<'_>,
    strings: &[Arc<str>],
) -> Result<Vec<String>, GrammarIrError> {
    let count = cursor.count(1, "string vector")?;
    let mut values = Vec::with_capacity(count);
    for _ in 0..count {
        values.push(read_string(cursor, strings)?);
    }
    Ok(values)
}

fn write_arc_string_vec(
    out: &mut Vec<u8>,
    strings: &StringTable,
    values: &[Arc<str>],
) -> Result<(), GrammarIrError> {
    write_len(out, values.len(), "scope vector count")?;
    for value in values {
        write_string(out, strings, value);
    }
    Ok(())
}

fn read_arc_string_vec(
    cursor: &mut Cursor<'_>,
    strings: &[Arc<str>],
) -> Result<Vec<Arc<str>>, GrammarIrError> {
    let count = cursor.count(1, "scope vector")?;
    let mut values = Vec::with_capacity(count);
    for _ in 0..count {
        let id = cursor.u32()?;
        values.push(
            strings
                .get(id as usize)
                .cloned()
                .ok_or(GrammarIrError::BadStringId(id))?,
        );
    }
    Ok(values)
}

fn read_pattern(
    cursor: &mut Cursor<'_>,
    pattern_count: usize,
) -> Result<PatternId, GrammarIrError> {
    let id = cursor.u32()?;
    if id as usize >= pattern_count {
        Err(GrammarIrError::BadPatternId(id))
    } else {
        Ok(PatternId(id))
    }
}

fn write_optional_scope(out: &mut Vec<u8>, value: Option<ScopeId>) {
    write_u32(out, value.map_or(0, |value| value.0 + 1));
}

fn read_optional_scope(
    cursor: &mut Cursor<'_>,
    scope_count: usize,
) -> Result<Option<ScopeId>, GrammarIrError> {
    let encoded = cursor.u32()?;
    if encoded == 0 {
        Ok(None)
    } else {
        let id = encoded - 1;
        if id as usize >= scope_count {
            Err(GrammarIrError::BadScopeId(id))
        } else {
            Ok(Some(ScopeId(id)))
        }
    }
}

fn write_len(out: &mut Vec<u8>, value: usize, name: &'static str) -> Result<(), GrammarIrError> {
    let value = u32::try_from(value).map_err(|_| GrammarIrError::ValueTooLarge(name))?;
    write_u32(out, value);
    Ok(())
}

fn write_u8(out: &mut Vec<u8>, value: u8) {
    out.push(value);
}

fn write_u16(out: &mut Vec<u8>, value: u16) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn write_u32(out: &mut Vec<u8>, mut value: u32) {
    while value >= 0x80 {
        out.push((value as u8 & 0x7f) | 0x80);
        value >>= 7;
    }
    out.push(value as u8);
}

struct Cursor<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> Cursor<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn remaining(&self) -> usize {
        self.bytes.len().saturating_sub(self.offset)
    }

    fn u8(&mut self) -> Result<u8, GrammarIrError> {
        let value = *self
            .bytes
            .get(self.offset)
            .ok_or(GrammarIrError::TooShort)?;
        self.offset += 1;
        Ok(value)
    }

    fn u16(&mut self) -> Result<u16, GrammarIrError> {
        Ok(u16::from_le_bytes(
            self.bytes(2)?.try_into().expect("slice length checked"),
        ))
    }

    fn u32(&mut self) -> Result<u32, GrammarIrError> {
        let mut value = 0u32;
        for shift in (0..=28).step_by(7) {
            let byte = self.u8()?;
            if shift == 28 && byte & 0xf0 != 0 {
                return Err(GrammarIrError::Malformed("u32 varint overflow"));
            }
            value |= u32::from(byte & 0x7f) << shift;
            if byte & 0x80 == 0 {
                if shift != 0 && byte == 0 {
                    return Err(GrammarIrError::Malformed("non-canonical u32 varint"));
                }
                return Ok(value);
            }
        }
        Err(GrammarIrError::Malformed("u32 varint overflow"))
    }

    fn boolean(&mut self) -> Result<bool, GrammarIrError> {
        match self.u8()? {
            0 => Ok(false),
            1 => Ok(true),
            value => Err(GrammarIrError::InvalidBoolean(value)),
        }
    }

    fn bytes(&mut self, len: usize) -> Result<&'a [u8], GrammarIrError> {
        let end = self
            .offset
            .checked_add(len)
            .ok_or(GrammarIrError::TooShort)?;
        let bytes = self
            .bytes
            .get(self.offset..end)
            .ok_or(GrammarIrError::TooShort)?;
        self.offset = end;
        Ok(bytes)
    }

    fn count(
        &mut self,
        minimum_item_bytes: usize,
        section: &'static str,
    ) -> Result<usize, GrammarIrError> {
        let count = self.u32()? as usize;
        if count > self.remaining() / minimum_item_bytes {
            return Err(GrammarIrError::CountOutOfBounds(section));
        }
        Ok(count)
    }

    fn finish(&self) -> Result<(), GrammarIrError> {
        if self.offset == self.bytes.len() {
            Ok(())
        } else {
            Err(GrammarIrError::TrailingBytes)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::grammar::load_dev_grammar_from_str;
    use super::*;

    fn fixture() -> CompiledGrammar {
        load_dev_grammar_from_str(
            GrammarId(7),
            r##"{
                "scopeName": "source.fixture",
                "displayName": "Fixture",
                "name": "fixture",
                "fileTypes": ["fixture"],
                "firstLineMatch": "^fixture",
                "injectionSelector": "L:source.fixture, R:text.html",
                "injectTo": ["source.host"],
                "patterns": [
                    {
                        "match": "(true)",
                        "name": "constant.language.fixture",
                        "captures": {"1": {"name": "capture.fixture"}}
                    },
                    {
                        "begin": "(\\\")",
                        "end": "(\\\")",
                        "name": "string.fixture",
                        "contentName": "meta.content.fixture",
                        "applyEndPatternLast": true,
                        "patterns": [{"include": "#word"}]
                    },
                    {
                        "begin": "^loop",
                        "while": "^\\s+",
                        "patterns": [{"include": "$base"}]
                    },
                    {
                        "patterns": [{"include": "source.external#entry"}],
                        "repository": {
                            "local": {"match": "local", "name": "local.fixture"}
                        }
                    }
                ],
                "repository": {
                    "word": {"match": "\\w+", "name": "word.fixture"}
                },
                "injections": {
                    "L:source.fixture": {"match": "TODO", "name": "todo.fixture"}
                }
            }"##,
        )
        .unwrap()
    }

    #[test]
    fn roundtrips_every_compiled_rule_shape() {
        let grammar = fixture();
        let bytes = encode_compiled_grammar(&grammar).unwrap();
        let decoded = decode_compiled_grammar(GrammarId(42), &bytes).unwrap();
        let mut expected = grammar;
        expected.id = GrammarId(42);
        assert_eq!(decoded, expected);
    }

    #[test]
    fn output_is_deterministic() {
        let grammar = fixture();
        assert_eq!(
            encode_compiled_grammar(&grammar).unwrap(),
            encode_compiled_grammar(&grammar).unwrap()
        );
    }

    #[test]
    fn rejects_stale_malformed_and_truncated_ir() {
        let grammar = fixture();
        let mut stale = encode_compiled_grammar(&grammar).unwrap();
        stale[4..6].copy_from_slice(&(VERSION + 1).to_le_bytes());
        assert_eq!(
            decode_compiled_grammar(GrammarId(0), &stale),
            Err(GrammarIrError::UnsupportedVersion(VERSION + 1))
        );

        let mut malformed = encode_compiled_grammar(&grammar).unwrap();
        malformed[8..13].copy_from_slice(&[0x80, 0x80, 0x80, 0x80, 0x10]);
        assert_eq!(
            decode_compiled_grammar(GrammarId(0), &malformed),
            Err(GrammarIrError::Malformed("u32 varint overflow"))
        );

        let mut truncated = encode_compiled_grammar(&grammar).unwrap();
        truncated.pop();
        assert!(matches!(
            decode_compiled_grammar(GrammarId(0), &truncated),
            Err(GrammarIrError::TooShort | GrammarIrError::CountOutOfBounds(_))
        ));
    }
}
