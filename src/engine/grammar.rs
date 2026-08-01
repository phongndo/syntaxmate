use std::{collections::BTreeMap, fmt, ops::Range, path::Path};

use serde::Deserialize;

use super::state::{GrammarId, PatternId, RuleId, ScopeId, StringId};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct CaptureSpec {
    pub entries: BTreeMap<u32, CaptureEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CaptureEntry {
    pub name: Option<ScopeId>,
    pub patterns: Vec<RuleRef>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuleBody {
    Match {
        pattern: PatternId,
        captures: CaptureSpec,
        name: Option<ScopeId>,
    },
    BeginEnd {
        begin: PatternId,
        end: PatternId,
        begin_captures: CaptureSpec,
        end_captures: CaptureSpec,
        name: Option<ScopeId>,
        content_name: Option<ScopeId>,
        apply_end_pattern_last: bool,
        patterns: Vec<RuleRef>,
    },
    BeginWhile {
        begin: PatternId,
        while_pattern: PatternId,
        begin_captures: CaptureSpec,
        while_captures: CaptureSpec,
        name: Option<ScopeId>,
        content_name: Option<ScopeId>,
        patterns: Vec<RuleRef>,
    },
    IncludeOnly {
        patterns: Vec<RuleRef>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum RuleRef {
    Rule(RuleId),
    Repository(String),
    SelfRef,
    BaseRef,
    External {
        scope: ScopeId,
        repository: Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rule {
    pub id: RuleId,
    /// Repository entries declared directly on this raw include-only rule.
    ///
    /// TextMate overlays these entries on the repository inherited by the
    /// rule at lazy-compilation time. The values are the collision-free names
    /// used in `CompiledGrammar::repository`.
    pub local_repository: BTreeMap<String, String>,
    pub body: RuleBody,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Injection {
    pub selector: String,
    pub selector_body: String,
    pub priority: InjectionPriority,
    pub patterns: Vec<RuleRef>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum InjectionPriority {
    Left,
    Right,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct GrammarMetadata {
    pub display_name: Option<String>,
    pub name: Option<String>,
    pub file_types: Vec<String>,
    pub first_line_match: Option<String>,
    /// Selector used when this grammar is registered as a standalone
    /// injection grammar. This is distinct from the grammar's inline
    /// `injections` map.
    pub injection_selector: Option<String>,
    /// Root scopes for which the registry should activate this standalone
    /// injection grammar.
    pub inject_to: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompiledGrammar {
    pub id: GrammarId,
    pub scope_name: String,
    pub metadata: GrammarMetadata,
    pub string_names: Vec<String>,
    pub patterns: Vec<String>,
    pub rules: Vec<Rule>,
    pub repository: BTreeMap<String, RuleRef>,
    pub top_level: Vec<RuleRef>,
    pub injections: Vec<Injection>,
    pub scope_names: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapturedText<'a> {
    pub range: Range<usize>,
    pub text: &'a str,
}

impl CompiledGrammar {
    pub fn rule(&self, id: RuleId) -> Option<&Rule> {
        self.rules.iter().find(|rule| rule.id == id)
    }

    pub fn pattern(&self, id: PatternId) -> Option<&str> {
        self.patterns.get(id.0 as usize).map(String::as_str)
    }

    pub fn scope(&self, id: ScopeId) -> Option<&str> {
        self.scope_names.get(id.0 as usize).map(String::as_str)
    }

    pub fn string(&self, id: StringId) -> Option<&str> {
        self.string_names.get(id.0 as usize).map(String::as_str)
    }

    pub fn validate_local_refs(&self) -> Result<(), GrammarValidationError> {
        self.validate_rule_refs(&self.top_level, "patterns")?;
        for (name, rule_ref) in &self.repository {
            self.validate_rule_ref(rule_ref, format!("repository.{name}").as_str(), false)?;
        }
        for injection in &self.injections {
            for (index, rule_ref) in injection.patterns.iter().enumerate() {
                self.validate_rule_ref(
                    rule_ref,
                    format!("injections.{}[{index}]", injection.selector).as_str(),
                    false,
                )?;
            }
        }
        for rule in &self.rules {
            self.validate_rule_body(rule)?;
        }
        Ok(())
    }

    pub fn debug_dump(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!("grammar {:?}\n", self.id));
        out.push_str(&format!("scopeName = {:?}\n", self.scope_name));
        out.push_str(&format!("metadata = {:?}\n", self.metadata));
        out.push_str("strings:\n");
        for (index, value) in self.string_names.iter().enumerate() {
            out.push_str(&format!("  {index}: {value:?}\n"));
        }
        out.push_str("scopes:\n");
        for (index, value) in self.scope_names.iter().enumerate() {
            out.push_str(&format!("  {index}: {value:?}\n"));
        }
        out.push_str("patterns:\n");
        for (index, pattern) in self.patterns.iter().enumerate() {
            out.push_str(&format!("  {index}: {pattern:?}\n"));
        }
        out.push_str("topLevel:\n");
        for (index, rule_ref) in self.top_level.iter().enumerate() {
            out.push_str(&format!("  {index}: {rule_ref:?}\n"));
        }
        out.push_str("repository:\n");
        for (name, rule_ref) in &self.repository {
            out.push_str(&format!("  {name}: {rule_ref:?}\n"));
        }
        out.push_str("injections:\n");
        for injection in &self.injections {
            out.push_str(&format!("  {injection:?}\n"));
        }
        out.push_str("rules:\n");
        for rule in &self.rules {
            out.push_str(&format!(
                "  {:?} repository={:?}: {:?}\n",
                rule.id, rule.local_repository, rule.body
            ));
        }
        out
    }

    fn validate_rule_body(&self, rule: &Rule) -> Result<(), GrammarValidationError> {
        match &rule.body {
            RuleBody::Match { captures, .. } => {
                self.validate_captures(captures, format!("rule.{}.captures", rule.id.0).as_str())
            }
            RuleBody::BeginEnd {
                begin_captures,
                end_captures,
                patterns,
                ..
            } => {
                self.validate_captures(
                    begin_captures,
                    format!("rule.{}.beginCaptures", rule.id.0).as_str(),
                )?;
                self.validate_captures(
                    end_captures,
                    format!("rule.{}.endCaptures", rule.id.0).as_str(),
                )?;
                self.validate_rule_refs(patterns, format!("rule.{}.patterns", rule.id.0).as_str())
            }
            RuleBody::BeginWhile {
                begin_captures,
                while_captures,
                patterns,
                ..
            } => {
                self.validate_captures(
                    begin_captures,
                    format!("rule.{}.beginCaptures", rule.id.0).as_str(),
                )?;
                self.validate_captures(
                    while_captures,
                    format!("rule.{}.whileCaptures", rule.id.0).as_str(),
                )?;
                self.validate_rule_refs(patterns, format!("rule.{}.patterns", rule.id.0).as_str())
            }
            RuleBody::IncludeOnly { patterns } => {
                self.validate_rule_refs(patterns, format!("rule.{}.patterns", rule.id.0).as_str())
            }
        }
    }

    fn validate_captures(
        &self,
        captures: &CaptureSpec,
        path: &str,
    ) -> Result<(), GrammarValidationError> {
        for (group, entry) in &captures.entries {
            self.validate_rule_refs(&entry.patterns, format!("{path}.{group}.patterns").as_str())?;
        }
        Ok(())
    }

    fn validate_rule_refs(
        &self,
        refs: &[RuleRef],
        path: &str,
    ) -> Result<(), GrammarValidationError> {
        for (index, rule_ref) in refs.iter().enumerate() {
            self.validate_rule_ref(rule_ref, format!("{path}[{index}]").as_str(), false)?;
        }
        Ok(())
    }

    pub(crate) fn validate_rule_ref(
        &self,
        rule_ref: &RuleRef,
        path: &str,
        validate_external_repository: bool,
    ) -> Result<(), GrammarValidationError> {
        match rule_ref {
            RuleRef::Rule(rule_id) => {
                if self.rule(*rule_id).is_none() {
                    return Err(GrammarValidationError::new(
                        self.scope_name.clone(),
                        path,
                        "rule",
                        format!("unknown rule id {}", rule_id.0),
                    ));
                }
            }
            RuleRef::Repository(name) => {
                if !self.repository.contains_key(name) {
                    return Err(GrammarValidationError::new(
                        self.scope_name.clone(),
                        path,
                        "include",
                        format!("unknown repository include #{name}"),
                    ));
                }
            }
            RuleRef::External { repository, .. } if validate_external_repository => {
                if let Some(repository) = repository
                    && !self.repository.contains_key(repository)
                {
                    return Err(GrammarValidationError::new(
                        self.scope_name.clone(),
                        path,
                        "include",
                        format!("unknown external repository #{repository}"),
                    ));
                }
            }
            RuleRef::SelfRef | RuleRef::BaseRef | RuleRef::External { .. } => {}
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GrammarValidationError {
    pub grammar: String,
    pub rule_path: String,
    pub field: String,
    pub message: String,
}

impl GrammarValidationError {
    pub fn new(
        grammar: impl Into<String>,
        rule_path: impl Into<String>,
        field: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            grammar: grammar.into(),
            rule_path: rule_path.into(),
            field: field.into(),
            message: message.into(),
        }
    }
}

impl fmt::Display for GrammarValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}:{}:{}: {}",
            self.grammar, self.rule_path, self.field, self.message
        )
    }
}

impl std::error::Error for GrammarValidationError {}

#[derive(Debug)]
pub enum GrammarLoadError {
    Json {
        path: Option<String>,
        source: serde_json::Error,
    },
    Validation {
        path: Option<String>,
        source: GrammarValidationError,
    },
}

impl fmt::Display for GrammarLoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Json { path, source } => match path {
                Some(path) => write!(f, "{path}: JSON parse error: {source}"),
                None => write!(f, "JSON parse error: {source}"),
            },
            Self::Validation { path, source } => match path {
                Some(path) => write!(f, "{path}: grammar validation error: {source}"),
                None => write!(f, "grammar validation error: {source}"),
            },
        }
    }
}

impl std::error::Error for GrammarLoadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Json { source, .. } => Some(source),
            Self::Validation { source, .. } => Some(source),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawGrammar {
    scope_name: String,
    #[serde(default)]
    display_name: Option<String>,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    file_types: Vec<String>,
    #[serde(default)]
    first_line_match: Option<String>,
    #[serde(default)]
    injection_selector: Option<String>,
    #[serde(default)]
    inject_to: Vec<String>,
    #[serde(default)]
    patterns: Vec<RawPattern>,
    #[serde(default)]
    repository: BTreeMap<String, RawRepositoryEntry>,
    #[serde(default)]
    injections: BTreeMap<String, RawPattern>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
enum RawRepositoryEntry {
    Pattern(Box<RawPattern>),
    Patterns(Vec<RawPattern>),
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct RawPattern {
    #[serde(rename = "match")]
    match_pattern: Option<String>,
    begin: Option<String>,
    end: Option<String>,
    #[serde(rename = "while")]
    while_pattern: Option<String>,
    include: Option<String>,
    name: Option<String>,
    content_name: Option<String>,
    #[serde(default)]
    patterns: Vec<RawPattern>,
    #[serde(default)]
    repository: BTreeMap<String, RawRepositoryEntry>,
    #[serde(default, deserialize_with = "deserialize_captures")]
    captures: BTreeMap<String, RawCapture>,
    #[serde(default, deserialize_with = "deserialize_captures")]
    begin_captures: BTreeMap<String, RawCapture>,
    #[serde(default, deserialize_with = "deserialize_captures")]
    end_captures: BTreeMap<String, RawCapture>,
    #[serde(default, deserialize_with = "deserialize_captures")]
    while_captures: BTreeMap<String, RawCapture>,
    #[serde(default, deserialize_with = "deserialize_boolish")]
    apply_end_pattern_last: bool,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(untagged)]
enum RawCapture {
    #[default]
    Empty,
    Name(String),
    Seq(Vec<RawCapture>),
    Full {
        name: Option<String>,
        #[serde(default)]
        patterns: Vec<RawPattern>,
    },
}

impl RawCapture {
    fn name(&self) -> Option<&str> {
        match self {
            Self::Empty => None,
            Self::Name(name) => Some(name),
            Self::Seq(captures) => captures.iter().find_map(Self::name),
            Self::Full { name, .. } => name.as_deref(),
        }
    }

    fn patterns(self) -> Vec<RawPattern> {
        match self {
            Self::Full { patterns, .. } => patterns,
            Self::Seq(captures) => captures.into_iter().flat_map(Self::patterns).collect(),
            Self::Empty | Self::Name(_) => Vec::new(),
        }
    }
}

fn deserialize_captures<'de, D>(deserializer: D) -> Result<BTreeMap<String, RawCapture>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum CaptureMap {
        Map(BTreeMap<String, RawCapture>),
        Seq(Vec<RawCapture>),
    }

    Ok(match Option::<CaptureMap>::deserialize(deserializer)? {
        Some(CaptureMap::Map(map)) => map,
        Some(CaptureMap::Seq(seq)) => seq
            .into_iter()
            .enumerate()
            .map(|(index, capture)| (index.to_string(), capture))
            .collect(),
        None => BTreeMap::new(),
    })
}

fn deserialize_boolish<'de, D>(deserializer: D) -> Result<bool, D::Error>
where
    D: serde::Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Boolish {
        Bool(bool),
        Int(u8),
    }

    Ok(match Option::<Boolish>::deserialize(deserializer)? {
        Some(Boolish::Bool(value)) => value,
        Some(Boolish::Int(value)) => value != 0,
        None => false,
    })
}

#[derive(Debug, Default)]
struct DevCompiler {
    next_rule: u32,
    strings: BTreeMap<String, StringId>,
    string_names: Vec<String>,
    patterns: Vec<String>,
    scopes: BTreeMap<String, ScopeId>,
    scope_names: Vec<String>,
    rules: Vec<Rule>,
    repository: BTreeMap<String, RuleRef>,
    local_repository_scopes: Vec<BTreeMap<String, String>>,
    next_local_repository: u32,
}

pub fn load_dev_grammar_from_str(
    id: GrammarId,
    contents: &str,
) -> Result<CompiledGrammar, GrammarLoadError> {
    load_dev_grammar_from_path(id, None::<&Path>, contents)
}

pub fn load_dev_grammar_from_path(
    id: GrammarId,
    path: Option<impl AsRef<Path>>,
    contents: &str,
) -> Result<CompiledGrammar, GrammarLoadError> {
    let path = path.map(|path| path.as_ref().display().to_string());
    let raw: RawGrammar =
        serde_json::from_str(contents).map_err(|source| GrammarLoadError::Json {
            path: path.clone(),
            source,
        })?;
    let mut compiler = DevCompiler::default();
    compiler.intern_grammar_header(&raw);
    let top_level = compiler.compile_patterns(raw.patterns);
    for (name, raw_rule) in raw.repository {
        compiler.string_id(&name);
        let rule_ref = compiler.compile_repository_entry(raw_rule);
        compiler.repository.insert(name, rule_ref);
    }
    let mut injections = Vec::new();
    if let Some(selector) = raw.injection_selector.as_deref() {
        compiler.string_id(selector);
    }
    for (selector, raw_rule) in raw.injections {
        compiler.string_id(&selector);
        let patterns = compiler.compile_patterns(vec![raw_rule]);
        for (priority, selector_body) in normalize_injection_selectors(&selector) {
            let selector = selector_body.clone();
            injections.push(Injection {
                selector,
                selector_body,
                priority,
                patterns: patterns.clone(),
            });
        }
    }

    Ok(CompiledGrammar {
        id,
        scope_name: raw.scope_name.clone(),
        metadata: GrammarMetadata {
            display_name: raw.display_name,
            name: raw.name,
            file_types: raw.file_types,
            first_line_match: raw.first_line_match,
            injection_selector: raw.injection_selector,
            inject_to: raw.inject_to,
        },
        string_names: compiler.string_names,
        patterns: compiler.patterns,
        rules: compiler.rules,
        repository: compiler.repository,
        top_level,
        injections,
        scope_names: compiler.scope_names,
    })
}

impl DevCompiler {
    fn intern_grammar_header(&mut self, raw: &RawGrammar) {
        self.string_id(&raw.scope_name);
        if let Some(value) = &raw.display_name {
            self.string_id(value);
        }
        if let Some(value) = &raw.name {
            self.string_id(value);
        }
        for value in &raw.file_types {
            self.string_id(value);
        }
        if let Some(value) = &raw.first_line_match {
            self.string_id(value);
        }
    }

    fn compile_patterns(&mut self, patterns: Vec<RawPattern>) -> Vec<RuleRef> {
        patterns
            .into_iter()
            .map(|pattern| self.compile_rule(pattern))
            .collect()
    }

    fn compile_include_only(&mut self, patterns: Vec<RawPattern>) -> RuleRef {
        let id = RuleId(self.next_rule);
        self.next_rule += 1;
        let patterns = self.compile_patterns(patterns);
        self.rules.push(Rule {
            id,
            local_repository: BTreeMap::new(),
            body: RuleBody::IncludeOnly { patterns },
        });
        RuleRef::Rule(id)
    }

    fn compile_rule(&mut self, raw: RawPattern) -> RuleRef {
        if let Some(include) = raw.include.as_deref() {
            return self.include_ref(include);
        }

        // RuleFactory overlays `repository` only for include-only rules.
        // Match and begin rules compile their captures/children against the
        // repository inherited from their first compilation path.
        let is_include_only = raw.match_pattern.is_none() && raw.begin.is_none();
        let local_repository = if is_include_only {
            self.push_local_repository(raw.repository)
        } else {
            BTreeMap::new()
        };
        let has_local_repository = !local_repository.is_empty();

        let id = RuleId(self.next_rule);
        self.next_rule += 1;
        let captures_default = raw.captures.clone();
        let body = if let Some(match_pattern) = raw.match_pattern {
            RuleBody::Match {
                pattern: self.pattern_id(match_pattern),
                captures: self.capture_spec(raw.captures),
                name: raw.name.as_deref().map(|scope| self.scope_id(scope)),
            }
        } else if let Some(begin) = raw.begin {
            if let Some(while_pattern) = raw.while_pattern {
                RuleBody::BeginWhile {
                    begin: self.pattern_id(begin),
                    while_pattern: self.pattern_id(while_pattern),
                    begin_captures: self.capture_spec(if raw.begin_captures.is_empty() {
                        captures_default.clone()
                    } else {
                        raw.begin_captures
                    }),
                    while_captures: self.capture_spec(if raw.while_captures.is_empty() {
                        captures_default.clone()
                    } else {
                        raw.while_captures
                    }),
                    name: raw.name.as_deref().map(|scope| self.scope_id(scope)),
                    content_name: raw
                        .content_name
                        .as_deref()
                        .map(|scope| self.scope_id(scope)),
                    patterns: self.compile_patterns(raw.patterns),
                }
            } else {
                RuleBody::BeginEnd {
                    begin: self.pattern_id(begin),
                    // The tokenizer recognizes an empty end as TextMate's
                    // non-matching sentinel. Keep it explicit rather than
                    // substituting a source scalar that could match real
                    // (albeit unusual) UTF-8 input.
                    end: self.pattern_id(raw.end.unwrap_or_default()),
                    begin_captures: self.capture_spec(if raw.begin_captures.is_empty() {
                        captures_default.clone()
                    } else {
                        raw.begin_captures
                    }),
                    end_captures: self.capture_spec(if raw.end_captures.is_empty() {
                        captures_default.clone()
                    } else {
                        raw.end_captures
                    }),
                    name: raw.name.as_deref().map(|scope| self.scope_id(scope)),
                    content_name: raw
                        .content_name
                        .as_deref()
                        .map(|scope| self.scope_id(scope)),
                    apply_end_pattern_last: raw.apply_end_pattern_last,
                    patterns: self.compile_patterns(raw.patterns),
                }
            }
        } else {
            RuleBody::IncludeOnly {
                patterns: self.compile_patterns(raw.patterns),
            }
        };
        self.rules.push(Rule {
            id,
            local_repository,
            body,
        });
        if has_local_repository {
            self.local_repository_scopes.pop();
        }
        RuleRef::Rule(id)
    }

    fn compile_repository_entry(&mut self, entry: RawRepositoryEntry) -> RuleRef {
        match entry {
            RawRepositoryEntry::Pattern(pattern) => self.compile_rule(*pattern),
            RawRepositoryEntry::Patterns(patterns) => self.compile_include_only(patterns),
        }
    }

    fn push_local_repository(
        &mut self,
        repository: BTreeMap<String, RawRepositoryEntry>,
    ) -> BTreeMap<String, String> {
        if repository.is_empty() {
            return BTreeMap::new();
        }
        let repository_id = self.next_local_repository;
        self.next_local_repository = self.next_local_repository.saturating_add(1);
        let aliases = repository
            .keys()
            .map(|name| (name.clone(), format!("$mark.local.{repository_id}.{name}")))
            .collect::<BTreeMap<_, _>>();
        self.local_repository_scopes.push(aliases.clone());
        for (name, entry) in repository {
            let alias = aliases
                .get(&name)
                .expect("local repository aliases cover every entry")
                .clone();
            self.string_id(&alias);
            let rule_ref = self.compile_repository_entry(entry);
            self.repository.insert(alias, rule_ref);
        }
        aliases
    }

    fn include_ref(&mut self, include: &str) -> RuleRef {
        self.string_id(include);
        match include {
            "$self" => RuleRef::SelfRef,
            "$base" => RuleRef::BaseRef,
            include if include.starts_with('#') => {
                let name = include.trim_start_matches('#');
                let resolved = self
                    .local_repository_scopes
                    .iter()
                    .rev()
                    .find_map(|repository| repository.get(name))
                    .cloned()
                    .unwrap_or_else(|| name.to_owned());
                self.string_id(&resolved);
                RuleRef::Repository(resolved)
            }
            include => {
                let (scope, repository) = include
                    .split_once('#')
                    .map(|(scope, repo)| (scope, Some(repo.to_owned())))
                    .unwrap_or((include, None));
                self.string_id(scope);
                if let Some(repository) = &repository {
                    self.string_id(repository);
                }
                RuleRef::External {
                    scope: self.scope_id(scope),
                    repository,
                }
            }
        }
    }

    fn pattern_id(&mut self, pattern: String) -> PatternId {
        self.string_id(&pattern);
        let id = PatternId(self.patterns.len() as u32);
        self.patterns.push(pattern);
        id
    }

    fn capture_spec(&mut self, captures: BTreeMap<String, RawCapture>) -> CaptureSpec {
        let entries = captures
            .into_iter()
            .filter_map(|(group, capture)| {
                let group = group.parse::<u32>().ok()?;
                Some((
                    group,
                    CaptureEntry {
                        name: capture.name().map(|scope| self.scope_id(scope)),
                        patterns: self.compile_patterns(capture.patterns()),
                    },
                ))
            })
            .collect();
        CaptureSpec { entries }
    }

    fn scope_id(&mut self, scope: &str) -> ScopeId {
        self.string_id(scope);
        if let Some(id) = self.scopes.get(scope) {
            return *id;
        }
        let id = ScopeId(self.scopes.len() as u32);
        self.scopes.insert(scope.to_owned(), id);
        self.scope_names.push(scope.to_owned());
        id
    }

    fn string_id(&mut self, value: &str) -> StringId {
        if let Some(id) = self.strings.get(value) {
            return *id;
        }
        let id = StringId(self.strings.len() as u32);
        self.strings.insert(value.to_owned(), id);
        self.string_names.push(value.to_owned());
        id
    }
}

pub(super) fn normalize_injection_selectors(selector: &str) -> Vec<(InjectionPriority, String)> {
    let mut left = Vec::new();
    let mut right = Vec::new();
    for alternative in split_selector_alternatives(selector) {
        let alternative = alternative.trim();
        if alternative.is_empty() {
            continue;
        }
        let (priority, body) = if let Some(rest) = alternative.strip_prefix("L:") {
            (InjectionPriority::Left, rest.trim())
        } else if let Some(rest) = alternative.strip_prefix("R:") {
            (InjectionPriority::Right, rest.trim())
        } else {
            (InjectionPriority::Right, alternative)
        };
        if !body.is_empty() {
            match priority {
                InjectionPriority::Left => left.push(body.to_owned()),
                InjectionPriority::Right => right.push(body.to_owned()),
            }
        }
    }
    let mut out = Vec::new();
    if !left.is_empty() {
        out.push((InjectionPriority::Left, left.join(", ")));
    }
    if !right.is_empty() {
        out.push((InjectionPriority::Right, right.join(", ")));
    }
    out
}

fn split_selector_alternatives(selector: &str) -> Vec<String> {
    let mut alternatives = Vec::new();
    let mut current = String::new();
    let mut depth = 0usize;
    for ch in selector.chars() {
        match ch {
            '(' => {
                depth = depth.saturating_add(1);
                current.push(ch);
            }
            ')' => {
                depth = depth.saturating_sub(1);
                current.push(ch);
            }
            ',' if depth == 0 => alternatives.push(std::mem::take(&mut current)),
            ch => current.push(ch),
        }
    }
    alternatives.push(current);
    alternatives
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rule_ids_preserve_order() {
        let grammar = CompiledGrammar {
            id: GrammarId(0),
            scope_name: "source.test".to_owned(),
            metadata: GrammarMetadata::default(),
            string_names: vec![],
            patterns: vec![],
            rules: vec![
                Rule {
                    id: RuleId(0),
                    local_repository: BTreeMap::new(),
                    body: RuleBody::IncludeOnly { patterns: vec![] },
                },
                Rule {
                    id: RuleId(1),
                    local_repository: BTreeMap::new(),
                    body: RuleBody::IncludeOnly { patterns: vec![] },
                },
            ],
            repository: BTreeMap::new(),
            top_level: vec![RuleRef::Rule(RuleId(0)), RuleRef::Rule(RuleId(1))],
            injections: vec![],
            scope_names: vec![],
        };
        assert_eq!(grammar.rules[0].id, RuleId(0));
        assert!(grammar.rule(RuleId(1)).is_some());
    }

    #[test]
    fn dev_loader_parses_match_and_begin_end_rules() {
        let grammar = load_dev_grammar_from_str(
            GrammarId(0),
            r##"{
                "scopeName": "source.fixture",
                "patterns": [
                    { "match": "\\btrue\\b", "name": "constant.language.fixture" },
                    { "begin": "\"", "end": "\"", "name": "string.quoted.double.fixture" }
                ]
            }"##,
        )
        .unwrap();
        assert_eq!(grammar.scope_name, "source.fixture");
        assert_eq!(grammar.top_level.len(), 2);
        assert_eq!(grammar.pattern(PatternId(0)), Some("\\btrue\\b"));
    }

    #[test]
    fn dev_loader_preserves_metadata_and_string_table() {
        let grammar = load_dev_grammar_from_str(
            GrammarId(0),
            r##"{
                "scopeName": "source.fixture",
                "displayName": "Fixture Lang",
                "name": "Fixture",
                "fileTypes": ["fixture", "fix"],
                "firstLineMatch": "^#!.*fixture",
                "patterns": [{ "match": "fixture", "name": "keyword.fixture" }]
            }"##,
        )
        .unwrap();
        assert_eq!(
            grammar.metadata.display_name.as_deref(),
            Some("Fixture Lang")
        );
        assert_eq!(grammar.metadata.name.as_deref(), Some("Fixture"));
        assert_eq!(grammar.metadata.file_types, vec!["fixture", "fix"]);
        assert_eq!(
            grammar.metadata.first_line_match.as_deref(),
            Some("^#!.*fixture")
        );
        assert_eq!(grammar.string(StringId(0)), Some("source.fixture"));
        assert!(
            grammar
                .string_names
                .iter()
                .any(|value| value == "keyword.fixture")
        );
    }

    #[test]
    fn dev_loader_models_begin_while_apply_end_last_and_nested_captures() {
        let grammar = load_dev_grammar_from_str(
            GrammarId(0),
            r##"{
                "scopeName": "source.fixture",
                "patterns": [
                    {
                        "begin": "^(>)",
                        "while": "^(>)",
                        "name": "markup.quote.fixture",
                        "captures": {"1": {"name": "punctuation.definition.quote.fixture"}},
                        "patterns": [{"match": "\\bTODO\\b", "name": "keyword.todo.fixture"}]
                    },
                    {
                        "begin": "(\\[)",
                        "end": "(\\])",
                        "applyEndPatternLast": true,
                        "contentName": "meta.brackets.fixture",
                        "captures": {
                            "1": {
                                "name": "punctuation.bracket.fixture",
                                "patterns": [{"match":"\\[", "name":"meta.nested.fixture"}]
                            }
                        }
                    }
                ]
            }"##,
        )
        .unwrap();
        assert!(matches!(
            &grammar.rule(RuleId(0)).unwrap().body,
            RuleBody::BeginWhile { while_captures, patterns, .. }
                if while_captures.entries.contains_key(&1) && !patterns.is_empty()
        ));
        assert!(matches!(
            &grammar.rule(RuleId(2)).unwrap().body,
            RuleBody::BeginEnd { apply_end_pattern_last: true, content_name: Some(_), begin_captures, .. }
                if begin_captures.entries.get(&1).is_some_and(|entry| !entry.patterns.is_empty())
        ));
    }

    #[test]
    fn dev_loader_preserves_repository_includes() {
        let grammar = load_dev_grammar_from_str(
            GrammarId(0),
            r##"{
                "scopeName": "source.fixture",
                "patterns": [{ "include": "#value" }],
                "repository": { "value": { "match": "true", "name": "constant.language.fixture" } }
            }"##,
        )
        .unwrap();
        assert_eq!(
            grammar.top_level,
            vec![RuleRef::Repository("value".to_owned())]
        );
        assert!(grammar.repository.contains_key("value"));
    }

    #[test]
    fn dev_loader_reports_missing_repository_with_context() {
        let grammar = load_dev_grammar_from_str(
            GrammarId(0),
            r##"{
                "scopeName": "source.fixture",
                "patterns": [{ "include": "#missing" }]
            }"##,
        )
        .unwrap();
        let error = grammar.validate_local_refs().unwrap_err();
        let message = error.to_string();
        assert!(message.contains("source.fixture"), "{message}");
        assert!(message.contains("patterns[0]"), "{message}");
        assert!(message.contains("#missing"), "{message}");
    }

    #[test]
    fn dev_loader_normalizes_injection_priority() {
        let grammar = load_dev_grammar_from_str(
            GrammarId(0),
            r##"{
                "scopeName": "source.fixture",
                "patterns": [],
                "injections": {
                    "L:source.fixture string": {"match":"todo", "name":"keyword.todo.fixture"},
                    "R:source.fixture comment": {"match":"note", "name":"keyword.note.fixture"}
                }
            }"##,
        )
        .unwrap();
        assert_eq!(grammar.injections.len(), 2);
        assert_eq!(grammar.injections[0].priority, InjectionPriority::Left);
        assert_eq!(grammar.injections[0].selector_body, "source.fixture string");
        assert_eq!(grammar.injections[1].priority, InjectionPriority::Right);
        assert_eq!(
            grammar.injections[1].selector_body,
            "source.fixture comment"
        );
    }

    #[test]
    fn dev_loader_splits_mixed_injection_selector_priorities() {
        let grammar = load_dev_grammar_from_str(
            GrammarId(0),
            r##"{
                "scopeName": "source.fixture",
                "patterns": [],
                "injections": {
                    "L:source.one, R:(source.two | source.three), source.four": {"match":"todo", "name":"keyword.todo.fixture"}
                }
            }"##,
        )
        .unwrap();
        assert_eq!(grammar.injections.len(), 2);
        assert_eq!(grammar.injections[0].priority, InjectionPriority::Left);
        assert_eq!(grammar.injections[0].selector_body, "source.one");
        assert_eq!(grammar.injections[1].priority, InjectionPriority::Right);
        assert_eq!(
            grammar.injections[1].selector_body,
            "(source.two | source.three), source.four"
        );
    }

    #[test]
    fn dev_loader_records_standalone_injection_registration() {
        let grammar = load_dev_grammar_from_str(
            GrammarId(0),
            r##"{
                "scopeName": "source.injected",
                "injectionSelector": "L:text.html - comment",
                "injectTo": ["text.html", "text.html.markdown"],
                "patterns": [{"match":"todo", "name":"keyword.todo.injected"}]
            }"##,
        )
        .unwrap();
        assert!(grammar.injections.is_empty());
        assert_eq!(
            grammar.metadata.injection_selector.as_deref(),
            Some("L:text.html - comment")
        );
        assert_eq!(
            grammar.metadata.inject_to,
            ["text.html", "text.html.markdown"]
        );
        assert!(
            grammar
                .string_names
                .iter()
                .any(|value| value == "L:text.html - comment")
        );
    }

    #[test]
    fn debug_dump_is_inspectable() {
        let grammar = load_dev_grammar_from_str(
            GrammarId(0),
            r##"{
                "scopeName": "source.fixture",
                "patterns": [{"match":"true", "name":"constant.fixture"}]
            }"##,
        )
        .unwrap();
        let dump = grammar.debug_dump();
        assert!(dump.contains("scopeName = \"source.fixture\""));
        assert!(dump.contains("patterns:"));
        assert!(dump.contains("rules:"));
    }

    #[test]
    fn dev_loader_keeps_json_begin_capture_zero() {
        let grammar = load_dev_grammar_from_str(
            GrammarId(0),
            include_str!("../../assets/grammars/languages/json.tmLanguage.json"),
        )
        .unwrap();
        let object_ref = grammar.repository.get("object").unwrap();
        let RuleRef::Rule(rule_id) = object_ref else {
            panic!("object rule");
        };
        let rule = grammar.rule(*rule_id).unwrap();
        let RuleBody::BeginEnd { begin_captures, .. } = &rule.body else {
            panic!("begin/end");
        };
        let capture = begin_captures.entries.get(&0).expect("capture 0");
        let name = capture.name.and_then(|id| grammar.scope(id));
        assert_eq!(name, Some("punctuation.definition.dictionary.begin.json"));
    }

    #[test]
    fn dev_loader_accepts_vendored_json_grammar() {
        let grammar = load_dev_grammar_from_str(
            GrammarId(0),
            include_str!("../../assets/grammars/languages/json.tmLanguage.json"),
        )
        .unwrap();
        assert_eq!(grammar.scope_name, "source.json");
        assert!(!grammar.top_level.is_empty());
    }
}
