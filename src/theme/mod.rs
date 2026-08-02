//! TextMate color-theme parsing and selector resolution.
//!
//! This is deliberately independent of the tokenizer: a theme can be changed
//! while reusing the immutable scope table in [`crate::HighlightedLine`].

use std::{cmp::Ordering, collections::HashMap};

#[cfg(feature = "bundled-themes")]
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering as AtomicOrdering};

use serde::Deserialize;

use crate::{HighlightScopeTable, ScopeStackRef, ThemeRule};

#[cfg(feature = "bundled-themes")]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BuiltinTextMateTheme {
    Dark,
    DarkHighContrast,
    Light,
    LightHighContrast,
}

#[cfg(feature = "bundled-themes")]
impl BuiltinTextMateTheme {
    pub fn from_name(name: &str) -> Option<Self> {
        Self::all()
            .iter()
            .copied()
            .find(|theme| theme.name() == name)
    }

    pub const fn name(self) -> &'static str {
        match self {
            Self::Dark => "github-dark",
            Self::DarkHighContrast => "github-dark-high-contrast",
            Self::Light => "github-light",
            Self::LightHighContrast => "github-light-high-contrast",
        }
    }

    pub fn get(self) -> &'static TextMateTheme {
        builtin_theme(self)
    }

    pub const fn all() -> &'static [Self] {
        &[
            Self::Dark,
            Self::DarkHighContrast,
            Self::Light,
            Self::LightHighContrast,
        ]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RgbColor {
    pub red: u8,
    pub green: u8,
    pub blue: u8,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct SyntaxModifiers(u8);

impl SyntaxModifiers {
    pub const BOLD: Self = Self(0b0001);
    pub const ITALIC: Self = Self(0b0010);
    pub const UNDERLINED: Self = Self(0b0100);
    pub const CROSSED_OUT: Self = Self(0b1000);

    pub const fn empty() -> Self {
        Self(0)
    }

    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    fn insert(&mut self, other: Self) {
        self.0 |= other.0;
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct ResolvedSyntaxStyle {
    pub foreground: Option<RgbColor>,
    pub background: Option<RgbColor>,
    pub modifiers: SyntaxModifiers,
}

/// Render-relevant theme resolution data, excluding diagnostic selector data.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct ResolvedThemeStyle {
    pub foreground_matched: bool,
    pub background_matched: bool,
    pub modifiers_matched: bool,
    pub style: ResolvedSyntaxStyle,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThemeMatch<'a> {
    pub selector: Option<&'a str>,
    pub score: Option<ThemeSelectorScore>,
    pub source_order: Option<usize>,
    pub foreground_matched: bool,
    pub background_matched: bool,
    pub modifiers_matched: bool,
    pub style: ResolvedSyntaxStyle,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThemeSelectorScore {
    pub target_depth: usize,
    pub parent_lengths: Vec<usize>,
    pub parent_count: usize,
    pub source_order: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextMateTheme {
    name: String,
    generation: u64,
    colors: HashMap<String, RgbColor>,
    default_style: ResolvedSyntaxStyle,
    rules: Vec<CompiledThemeRule>,
    candidates_by_head: HashMap<String, Vec<usize>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CompiledThemeRule {
    selector_text: String,
    target: String,
    // Deepest parent first, matching vscode-textmate's representation.
    parents: Vec<String>,
    foreground: Option<RgbColor>,
    background: Option<RgbColor>,
    modifiers: Option<SyntaxModifiers>,
    source_order: usize,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawTheme {
    #[serde(default)]
    name: String,
    #[serde(default)]
    colors: HashMap<String, serde_json::Value>,
    #[serde(default)]
    token_colors: Vec<RawRule>,
}

#[derive(Debug, Deserialize)]
struct RawRule {
    #[serde(default)]
    scope: Option<RawScopes>,
    settings: RawSettings,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum RawScopes {
    One(String),
    Many(Vec<String>),
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawSettings {
    #[serde(default)]
    foreground: Option<String>,
    #[serde(default)]
    background: Option<String>,
    #[serde(default)]
    font_style: Option<String>,
}

impl TextMateTheme {
    pub fn from_json(json: &str) -> Result<Self, String> {
        let raw: RawTheme = serde_json::from_str(json)
            .map_err(|error| format!("invalid TextMate theme JSON: {error}"))?;
        let editor_background = raw
            .colors
            .get("editor.background")
            .and_then(|value| value.as_str())
            .map(parse_editor_background)
            .transpose()?;
        let colors = raw
            .colors
            .iter()
            .filter_map(|(name, value)| {
                value
                    .as_str()
                    .and_then(|value| parse_color(value, editor_background).ok())
                    .map(|color| (name.clone(), color))
            })
            .collect();
        let default_style = ResolvedSyntaxStyle {
            foreground: parse_optional_color(
                raw.colors
                    .get("editor.foreground")
                    .and_then(|value| value.as_str()),
                editor_background,
            )?,
            background: editor_background,
            modifiers: SyntaxModifiers::empty(),
        };
        let mut rules = Vec::new();
        let mut defaults = default_style;
        for (source_order, raw_rule) in raw.token_colors.into_iter().enumerate() {
            let foreground =
                parse_optional_color(raw_rule.settings.foreground.as_deref(), editor_background)?;
            let background =
                parse_optional_color(raw_rule.settings.background.as_deref(), editor_background)?;
            let modifiers = raw_rule
                .settings
                .font_style
                .as_deref()
                .map(parse_modifiers)
                .transpose()?;
            let scopes = match raw_rule.scope {
                Some(RawScopes::One(scope)) => scope
                    .trim_matches(',')
                    .split(',')
                    .map(str::to_owned)
                    .collect(),
                Some(RawScopes::Many(scopes)) => scopes
                    .into_iter()
                    .flat_map(|scope| scope.split(',').map(str::to_owned).collect::<Vec<_>>())
                    .collect(),
                None => Vec::new(),
            };
            if scopes.is_empty() {
                if foreground.is_some() {
                    defaults.foreground = foreground;
                }
                if background.is_some() {
                    defaults.background = background;
                }
                if let Some(modifiers) = modifiers {
                    defaults.modifiers = modifiers;
                }
                continue;
            }
            for selector in scopes {
                let selector = selector.trim();
                if selector.is_empty() {
                    continue;
                }
                let mut parts = selector.split_whitespace().collect::<Vec<_>>();
                let target = parts
                    .pop()
                    .ok_or_else(|| format!("empty theme selector at rule {source_order}"))?;
                validate_scope_pattern(target, source_order)?;
                for parent in &parts {
                    if *parent != ">" {
                        validate_scope_pattern(parent, source_order)?;
                    }
                }
                parts.reverse();
                rules.push(CompiledThemeRule {
                    selector_text: selector.to_owned(),
                    target: target.to_owned(),
                    parents: parts.into_iter().map(str::to_owned).collect(),
                    foreground,
                    background,
                    modifiers,
                    source_order,
                });
            }
        }
        let mut candidates_by_head = HashMap::<String, Vec<usize>>::new();
        for (index, rule) in rules.iter().enumerate() {
            let head = rule.target.split('.').next().unwrap_or(&rule.target);
            candidates_by_head
                .entry(head.to_owned())
                .or_default()
                .push(index);
        }
        Ok(Self {
            name: raw.name,
            generation: next_theme_generation(),
            colors,
            default_style: defaults,
            rules,
            candidates_by_head,
        })
    }

    /// Compiles post-theme user selector rules through the same matcher as
    /// built-in TextMate themes.
    pub fn from_rules(rules: &[ThemeRule]) -> Result<Self, String> {
        let mut compiled = Vec::new();
        for (source_order, rule) in rules.iter().enumerate() {
            let foreground = parse_optional_syntax_rule_color(rule.foreground.as_deref())?;
            let background = parse_optional_syntax_rule_color(rule.background.as_deref())?;
            let modifiers = rule
                .font_style
                .as_deref()
                .map(parse_modifiers)
                .transpose()?;
            if foreground.is_none() && background.is_none() && modifiers.is_none() {
                return Err(format!(
                    "syntax rule {source_order} must set foreground, background, or font_style"
                ));
            }
            for selector in rule.scope.split(',').map(str::trim) {
                if selector.is_empty() {
                    return Err(format!("empty syntax rule selector at rule {source_order}"));
                }
                compiled.push(compile_rule(
                    selector,
                    foreground,
                    background,
                    modifiers,
                    source_order,
                )?);
            }
        }
        let mut candidates_by_head = HashMap::<String, Vec<usize>>::new();
        for (index, rule) in compiled.iter().enumerate() {
            candidates_by_head
                .entry(selector_head(&rule.target).to_owned())
                .or_default()
                .push(index);
        }
        Ok(Self {
            name: "user syntax rules".to_owned(),
            generation: next_theme_generation(),
            colors: HashMap::new(),
            default_style: ResolvedSyntaxStyle::default(),
            rules: compiled,
            candidates_by_head,
        })
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn default_style(&self) -> ResolvedSyntaxStyle {
        self.default_style
    }

    pub fn color(&self, name: &str) -> Option<RgbColor> {
        self.colors.get(name).copied()
    }

    pub fn resolve(
        &self,
        table: &HighlightScopeTable,
        stack: ScopeStackRef,
    ) -> ResolvedSyntaxStyle {
        self.resolve_style(table, stack).style
    }

    /// Resolves and caches all style and property-match data needed by a
    /// renderer. Diagnostic selector metadata remains available separately
    /// through [`Self::resolve_with_match`].
    pub fn resolve_style(
        &self,
        table: &HighlightScopeTable,
        stack: ScopeStackRef,
    ) -> ResolvedThemeStyle {
        let (slot, cached) = table.cached_style(self.generation, stack);
        if let Some(style) = cached {
            return unpack_style(style);
        }
        let matched = self.resolve_with_match(table, stack);
        let resolved = ResolvedThemeStyle {
            foreground_matched: matched.foreground_matched,
            background_matched: matched.background_matched,
            modifiers_matched: matched.modifiers_matched,
            style: matched.style,
        };
        table.cache_style(self.generation, stack, slot, pack_style(resolved));
        resolved
    }

    pub fn resolve_with_match<'a>(
        &'a self,
        table: &HighlightScopeTable,
        stack: ScopeStackRef,
    ) -> ThemeMatch<'a> {
        let Some(atoms) = table.stack(stack) else {
            return ThemeMatch {
                selector: None,
                score: None,
                source_order: None,
                foreground_matched: false,
                background_matched: false,
                modifiers_matched: false,
                style: self.default_style,
            };
        };
        // vscode-textmate applies theme attributes every time a scope is
        // pushed, merging only properties set by the new scope into the
        // attributes inherited from its parent. Resolving only the innermost
        // scope incorrectly turns punctuation nested in support.function back
        // to the editor default, which was the remaining LaTeX discrepancy.
        let mut style = self.default_style;
        let mut representative: Option<&CompiledThemeRule> = None;
        let mut foreground_matched = false;
        let mut background_matched = false;
        let mut modifiers_matched = false;
        for depth in 1..=atoms.len() {
            let active_stack = &atoms[..depth];
            let Some(scope) = table.atom(active_stack[depth - 1]) else {
                continue;
            };
            let head = scope.split('.').next().unwrap_or(scope);
            let mut foreground = None;
            let mut background = None;
            let mut modifiers = None;
            let mut level_representative: Option<&CompiledThemeRule> = None;
            for rule_index in self.candidates_by_head.get(head).into_iter().flatten() {
                let rule = &self.rules[*rule_index];
                if !rule.matches(table, active_stack) {
                    continue;
                }
                if level_representative
                    .is_none_or(|current| compare_specificity(rule, current).is_gt())
                {
                    level_representative = Some(rule);
                }
                update_property(&mut foreground, rule, rule.foreground);
                update_property(&mut background, rule, rule.background);
                update_property(&mut modifiers, rule, rule.modifiers);
            }
            if let Some((_, foreground)) = foreground {
                style.foreground = Some(foreground);
                foreground_matched = true;
            }
            if let Some((_, background)) = background {
                style.background = Some(background);
                background_matched = true;
            }
            if let Some((_, modifiers)) = modifiers {
                style.modifiers = modifiers;
                modifiers_matched = true;
            }
            if level_representative.is_some() {
                representative = level_representative;
            }
        }
        ThemeMatch {
            selector: representative.map(|rule| rule.selector_text.as_str()),
            score: representative.map(selector_score),
            source_order: representative.map(|rule| rule.source_order),
            foreground_matched,
            background_matched,
            modifiers_matched,
            style,
        }
    }
}

fn compile_rule(
    selector: &str,
    foreground: Option<RgbColor>,
    background: Option<RgbColor>,
    modifiers: Option<SyntaxModifiers>,
    source_order: usize,
) -> Result<CompiledThemeRule, String> {
    let mut parts = selector.split_whitespace().collect::<Vec<_>>();
    let target = parts
        .pop()
        .ok_or_else(|| format!("empty theme selector at rule {source_order}"))?;
    validate_scope_pattern(target, source_order)?;
    for parent in &parts {
        if *parent != ">" {
            validate_scope_pattern(parent, source_order)?;
        }
    }
    parts.reverse();
    Ok(CompiledThemeRule {
        selector_text: selector.to_owned(),
        target: target.to_owned(),
        parents: parts.into_iter().map(str::to_owned).collect(),
        foreground,
        background,
        modifiers,
        source_order,
    })
}

fn selector_head(target: &str) -> &str {
    target.split('.').next().unwrap_or(target)
}

fn selector_score(rule: &CompiledThemeRule) -> ThemeSelectorScore {
    let parent_lengths = rule
        .parents
        .iter()
        .filter(|scope| scope.as_str() != ">")
        .map(String::len)
        .collect::<Vec<_>>();
    ThemeSelectorScore {
        target_depth: dot_depth(&rule.target),
        parent_count: parent_lengths.len(),
        parent_lengths,
        source_order: rule.source_order,
    }
}

fn next_theme_generation() -> u64 {
    static NEXT: AtomicU64 = AtomicU64::new(1);
    NEXT.fetch_add(1, AtomicOrdering::Relaxed)
}

fn pack_style(resolved: ResolvedThemeStyle) -> u64 {
    fn color(color: Option<RgbColor>) -> u64 {
        color.map_or(0, |color| {
            (1 << 24)
                | (u64::from(color.red) << 16)
                | (u64::from(color.green) << 8)
                | u64::from(color.blue)
        })
    }
    let style = resolved.style;
    color(style.foreground)
        | (color(style.background) << 25)
        | (u64::from(style.modifiers.0) << 50)
        | (u64::from(resolved.foreground_matched) << 58)
        | (u64::from(resolved.background_matched) << 59)
        | (u64::from(resolved.modifiers_matched) << 60)
}

fn unpack_style(packed: u64) -> ResolvedThemeStyle {
    fn color(packed: u64) -> Option<RgbColor> {
        (packed & (1 << 24) != 0).then_some(RgbColor {
            red: ((packed >> 16) & 0xff) as u8,
            green: ((packed >> 8) & 0xff) as u8,
            blue: (packed & 0xff) as u8,
        })
    }
    ResolvedThemeStyle {
        foreground_matched: packed & (1 << 58) != 0,
        background_matched: packed & (1 << 59) != 0,
        modifiers_matched: packed & (1 << 60) != 0,
        style: ResolvedSyntaxStyle {
            foreground: color(packed & 0x1ff_ffff),
            background: color((packed >> 25) & 0x1ff_ffff),
            modifiers: SyntaxModifiers(((packed >> 50) & 0xff) as u8),
        },
    }
}

impl CompiledThemeRule {
    fn matches(&self, table: &HighlightScopeTable, stack: &[crate::ScopeAtomId]) -> bool {
        let Some((inner, ancestors)) = stack.split_last() else {
            return false;
        };
        let Some(inner) = table.atom(*inner) else {
            return false;
        };
        if !scope_matches(inner, &self.target) {
            return false;
        }
        let mut ancestor_index = ancestors.len();
        let mut parent_index = 0;
        while parent_index < self.parents.len() {
            let mut direct = false;
            let mut pattern = self.parents[parent_index].as_str();
            if pattern == ">" {
                parent_index += 1;
                let Some(next) = self.parents.get(parent_index) else {
                    return false;
                };
                pattern = next;
                direct = true;
            }
            let mut found = false;
            while ancestor_index > 0 {
                ancestor_index -= 1;
                if table
                    .atom(ancestors[ancestor_index])
                    .is_some_and(|scope| scope_matches(scope, pattern))
                {
                    found = true;
                    break;
                }
                if direct {
                    return false;
                }
            }
            if !found {
                return false;
            }
            parent_index += 1;
        }
        true
    }
}

fn update_property<'a, T: Copy>(
    current: &mut Option<(&'a CompiledThemeRule, T)>,
    candidate: &'a CompiledThemeRule,
    value: Option<T>,
) {
    let Some(value) = value else { return };
    if current
        .as_ref()
        .is_none_or(|(rule, _)| compare_specificity(candidate, rule).is_gt())
    {
        *current = Some((candidate, value));
    }
}

fn compare_specificity(a: &CompiledThemeRule, b: &CompiledThemeRule) -> Ordering {
    let target = dot_depth(&a.target).cmp(&dot_depth(&b.target));
    if target != Ordering::Equal {
        return target;
    }
    let a_parents = a.parents.iter().filter(|scope| scope.as_str() != ">");
    let b_parents = b.parents.iter().filter(|scope| scope.as_str() != ">");
    for (a_parent, b_parent) in a_parents.clone().zip(b_parents.clone()) {
        let ordering = a_parent.len().cmp(&b_parent.len());
        if ordering != Ordering::Equal {
            return ordering;
        }
    }
    let parents = a_parents.count().cmp(&b_parents.count());
    if parents != Ordering::Equal {
        return parents;
    }
    a.source_order.cmp(&b.source_order)
}

fn dot_depth(scope: &str) -> usize {
    scope.bytes().filter(|byte| *byte == b'.').count() + 1
}

fn scope_matches(scope: &str, pattern: &str) -> bool {
    scope == pattern
        || scope
            .strip_prefix(pattern)
            .is_some_and(|suffix| suffix.starts_with('.'))
}

fn validate_scope_pattern(pattern: &str, source_order: usize) -> Result<(), String> {
    if pattern.is_empty()
        || pattern.starts_with('.')
        || pattern.ends_with('.')
        || pattern.contains("..")
        || !pattern.bytes().all(|byte| {
            byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b':' | b'.' | b'*')
        })
    {
        return Err(format!(
            "unsupported TextMate selector `{pattern}` at rule {source_order}"
        ));
    }
    Ok(())
}

fn parse_modifiers(value: &str) -> Result<SyntaxModifiers, String> {
    let mut modifiers = SyntaxModifiers::empty();
    for modifier in value.split_whitespace() {
        match modifier {
            // VS Code themes use `normal` to explicitly clear inherited styles.
            "normal" | "regular" => {}
            "bold" => modifiers.insert(SyntaxModifiers::BOLD),
            "italic" => modifiers.insert(SyntaxModifiers::ITALIC),
            "underline" => modifiers.insert(SyntaxModifiers::UNDERLINED),
            "strikethrough" => modifiers.insert(SyntaxModifiers::CROSSED_OUT),
            unsupported => return Err(format!("unsupported TextMate fontStyle `{unsupported}`")),
        }
    }
    Ok(modifiers)
}

fn parse_optional_color(
    value: Option<&str>,
    background: Option<RgbColor>,
) -> Result<Option<RgbColor>, String> {
    value
        .map(|value| parse_color(value, background))
        .transpose()
}

fn parse_optional_syntax_rule_color(value: Option<&str>) -> Result<Option<RgbColor>, String> {
    // User rules have no single background to composite against: their tokens
    // can be rendered over theme, diff, or inline-diff backgrounds. Preserve
    // the historical behavior of accepting alpha forms and using their RGB
    // channels rather than rejecting an existing configuration.
    value
        .map(|value| parse_rgba_color(value).map(|(color, _)| color))
        .transpose()
}

fn parse_editor_background(value: &str) -> Result<RgbColor, String> {
    let (color, alpha) = parse_rgba_color(value)?;
    if alpha != u8::MAX {
        return Err(format!(
            "TextMate editor.background must be opaque, got `{value}`"
        ));
    }
    Ok(color)
}

fn parse_color(value: &str, background: Option<RgbColor>) -> Result<RgbColor, String> {
    let (color, alpha) = parse_rgba_color(value)?;
    if alpha == u8::MAX {
        return Ok(color);
    }
    let background = background.ok_or_else(|| {
        format!("translucent TextMate color `{value}` requires an opaque editor.background")
    })?;
    let composite = |foreground: u8, background: u8| {
        let alpha = u32::from(alpha);
        ((u32::from(foreground) * alpha
            + u32::from(background) * (u32::from(u8::MAX) - alpha)
            + 127)
            / u32::from(u8::MAX)) as u8
    };
    Ok(RgbColor {
        red: composite(color.red, background.red),
        green: composite(color.green, background.green),
        blue: composite(color.blue, background.blue),
    })
}

fn parse_rgba_color(value: &str) -> Result<(RgbColor, u8), String> {
    let invalid = || format!("unsupported TextMate color `{value}`");
    let hex = value.strip_prefix('#').ok_or_else(invalid)?;
    if !hex.is_ascii() {
        return Err(invalid());
    }
    let byte = |start| u8::from_str_radix(&hex[start..start + 2], 16).map_err(|_| invalid());
    let nibble = |index| {
        u8::from_str_radix(&hex[index..index + 1], 16)
            .map(|value| value * 0x11)
            .map_err(|_| invalid())
    };
    let (red, green, blue, alpha) = match hex.len() {
        3 => (nibble(0)?, nibble(1)?, nibble(2)?, u8::MAX),
        4 => (nibble(0)?, nibble(1)?, nibble(2)?, nibble(3)?),
        6 => (byte(0)?, byte(2)?, byte(4)?, u8::MAX),
        8 => (byte(0)?, byte(2)?, byte(4)?, byte(6)?),
        _ => return Err(invalid()),
    };
    Ok((RgbColor { red, green, blue }, alpha))
}

#[cfg(feature = "bundled-themes")]
macro_rules! vendored_theme {
    ($function:ident, $file:literal) => {
        pub fn $function() -> &'static TextMateTheme {
            static THEME: OnceLock<TextMateTheme> = OnceLock::new();
            THEME.get_or_init(|| {
                TextMateTheme::from_json(include_str!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/assets/themes/",
                    $file
                )))
                .unwrap_or_else(|error| panic!("vendored theme {} is invalid: {error}", $file))
            })
        }
    };
}

#[cfg(feature = "bundled-themes")]
vendored_theme!(github_dark, "github-dark.json");
#[cfg(feature = "bundled-themes")]
vendored_theme!(github_dark_high_contrast, "github-dark-high-contrast.json");
#[cfg(feature = "bundled-themes")]
vendored_theme!(github_light, "github-light.json");
#[cfg(feature = "bundled-themes")]
vendored_theme!(
    github_light_high_contrast,
    "github-light-high-contrast.json"
);

#[cfg(feature = "bundled-themes")]
fn builtin_theme(theme: BuiltinTextMateTheme) -> &'static TextMateTheme {
    match theme {
        BuiltinTextMateTheme::Dark => github_dark(),
        BuiltinTextMateTheme::DarkHighContrast => github_dark_high_contrast(),
        BuiltinTextMateTheme::Light => github_light(),
        BuiltinTextMateTheme::LightHighContrast => github_light_high_contrast(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn official_theme_loads() {
        let theme = github_dark_high_contrast();
        assert_eq!(theme.name(), "GitHub Dark High Contrast");
        assert_eq!(theme.rules.len(), 87);
    }

    #[test]
    fn every_named_builtin_theme_loads() {
        for theme in BuiltinTextMateTheme::all() {
            let theme = theme.get();
            assert!(!theme.name().is_empty());
            assert!(
                !theme.rules.is_empty(),
                "{} has no token rules",
                theme.name()
            );
        }
    }

    #[test]
    fn translucent_theme_colors_are_composited_over_editor_background() {
        let theme = TextMateTheme::from_json(
            r##"{
                "colors": {
                    "editor.background": "#102030",
                    "editor.foreground": "#ffffff80",
                    "editor.lineHighlightBackground": "#fff8"
                },
                "tokenColors": [{
                    "scope": "markup.raw",
                    "settings": { "background": "#f000" }
                }]
            }"##,
        )
        .unwrap();

        assert_eq!(
            theme.default_style().foreground,
            Some(RgbColor {
                red: 0x88,
                green: 0x90,
                blue: 0x98,
            })
        );
        assert_eq!(
            theme.color("editor.lineHighlightBackground"),
            Some(RgbColor {
                red: 0x8f,
                green: 0x97,
                blue: 0x9e,
            })
        );
        let (table, stack) = HighlightScopeTable::from_scope_names(&["markup.raw"]);
        assert_eq!(
            theme.resolve(&table, stack).background,
            Some(RgbColor {
                red: 0x10,
                green: 0x20,
                blue: 0x30,
            })
        );
    }

    #[test]
    fn scope_prefix_requires_a_dot_boundary() {
        assert!(scope_matches("support.function", "support"));
        assert!(!scope_matches("supportive.function", "support"));
    }

    #[test]
    fn reset_font_styles_are_supported() {
        assert_eq!(parse_modifiers("").unwrap(), SyntaxModifiers::empty());
        assert_eq!(parse_modifiers("normal").unwrap(), SyntaxModifiers::empty());
        assert_eq!(
            parse_modifiers("regular").unwrap(),
            SyntaxModifiers::empty()
        );
    }

    #[test]
    fn cached_style_encoding_round_trips() {
        let style = ResolvedThemeStyle {
            foreground_matched: true,
            background_matched: false,
            modifiers_matched: true,
            style: ResolvedSyntaxStyle {
                foreground: Some(RgbColor {
                    red: 1,
                    green: 2,
                    blue: 3,
                }),
                background: Some(RgbColor {
                    red: 0xfd,
                    green: 0xfe,
                    blue: 0xff,
                }),
                modifiers: SyntaxModifiers(
                    SyntaxModifiers::BOLD.0 | SyntaxModifiers::CROSSED_OUT.0,
                ),
            },
        };
        assert_eq!(unpack_style(pack_style(style)), style);
    }

    #[test]
    fn render_cache_keeps_base_theme_and_scope_override_matches() {
        let base = TextMateTheme::from_json(
            r##"{"tokenColors":[{"scope":"entity.name","settings":{"foreground":"#123456"}}]}"##,
        )
        .unwrap();
        let overrides = TextMateTheme::from_rules(&[ThemeRule {
            scope: "entity.name".to_owned(),
            font_style: Some(String::new()),
            ..ThemeRule::default()
        }])
        .unwrap();
        let (table, stack) =
            HighlightScopeTable::from_scope_names(&["source.test", "entity.name.test"]);

        let base_match = base.resolve_style(&table, stack);
        let override_match = overrides.resolve_style(&table, stack);
        assert!(base_match.foreground_matched);
        assert!(!base_match.modifiers_matched);
        assert!(!override_match.foreground_matched);
        assert!(override_match.modifiers_matched);

        let (_, cached_base) = table.cached_style(base.generation, stack);
        let (_, cached_override) = table.cached_style(overrides.generation, stack);
        assert_eq!(cached_base.map(unpack_style), Some(base_match));
        assert_eq!(cached_override.map(unpack_style), Some(override_match));
    }

    #[test]
    fn shared_scope_table_cache_is_correct_across_concurrent_themes() {
        let red = Arc::new(
            TextMateTheme::from_json(
                r##"{"tokenColors":[{"scope":"entity.name","settings":{"foreground":"#ff0000"}}]}"##,
            )
            .unwrap(),
        );
        let blue = Arc::new(
            TextMateTheme::from_json(
                r##"{"tokenColors":[{"scope":"entity.name","settings":{"foreground":"#0000ff"}}]}"##,
            )
            .unwrap(),
        );
        let green = Arc::new(
            TextMateTheme::from_json(
                r##"{"tokenColors":[{"scope":"entity.name","settings":{"foreground":"#00ff00"}}]}"##,
            )
            .unwrap(),
        );
        let (table, stack) =
            HighlightScopeTable::from_scope_names(&["source.test", "entity.name.test"]);
        let table = Arc::new(table);

        std::thread::scope(|threads| {
            let start = Arc::new(std::sync::Barrier::new(6));
            for (theme, expected) in [
                (
                    red,
                    RgbColor {
                        red: 0xff,
                        green: 0,
                        blue: 0,
                    },
                ),
                (
                    blue,
                    RgbColor {
                        red: 0,
                        green: 0,
                        blue: 0xff,
                    },
                ),
                (
                    green,
                    RgbColor {
                        red: 0,
                        green: 0xff,
                        blue: 0,
                    },
                ),
            ] {
                for _ in 0..2 {
                    let theme = Arc::clone(&theme);
                    let table = Arc::clone(&table);
                    let start = Arc::clone(&start);
                    threads.spawn(move || {
                        start.wait();
                        for _ in 0..2_000 {
                            assert_eq!(theme.resolve(&table, stack).foreground, Some(expected));
                        }
                    });
                }
            }
        });
    }

    #[test]
    fn official_theme_distinguishes_lossy_coarse_classes() {
        let theme = github_dark_high_contrast();
        let resolve = |scopes: &[&str]| {
            let (table, stack) = HighlightScopeTable::from_scope_names(scopes);
            theme.resolve(&table, stack)
        };
        assert_eq!(
            resolve(&["text.tex.latex", "support.function.general.tex"]).foreground,
            Some(RgbColor {
                red: 0x91,
                green: 0xcb,
                blue: 0xff,
            })
        );
        assert_eq!(
            resolve(&["source.test", "entity.name.function"]).foreground,
            Some(RgbColor {
                red: 0xdb,
                green: 0xb7,
                blue: 0xff,
            })
        );
        assert_eq!(
            resolve(&["text.tex.latex", "constant.character.math.tex"]).foreground,
            Some(RgbColor {
                red: 0xff,
                green: 0x94,
                blue: 0x92,
            })
        );
    }

    #[test]
    fn nested_unmatched_scope_inherits_parent_theme_attributes() {
        let theme = github_dark_high_contrast();
        let (table, stack) = HighlightScopeTable::from_scope_names(&[
            "text.tex.latex",
            "support.function.be.latex",
            "punctuation.definition.function.latex",
        ]);
        assert_eq!(
            theme.resolve(&table, stack).foreground,
            Some(RgbColor {
                red: 0x91,
                green: 0xcb,
                blue: 0xff,
            })
        );
    }

    #[test]
    fn parent_selectors_and_font_modifiers_resolve() {
        let theme = github_dark_high_contrast();
        let (table, stack) =
            HighlightScopeTable::from_scope_names(&["text.html.markdown", "markup.bold.markdown"]);
        assert!(
            theme
                .resolve(&table, stack)
                .modifiers
                .contains(SyntaxModifiers::BOLD)
        );

        let (table, stack) = HighlightScopeTable::from_scope_names(&[
            "source.test",
            "string.quoted",
            "variable.custom",
        ]);
        assert_eq!(
            theme.resolve(&table, stack).foreground,
            Some(RgbColor {
                red: 0x91,
                green: 0xcb,
                blue: 0xff,
            })
        );
    }

    #[test]
    fn nonopaque_user_syntax_rule_colors_preserve_their_rgb_channels() {
        let theme = TextMateTheme::from_rules(&[ThemeRule {
            scope: "constant.numeric".to_owned(),
            foreground: Some("#ff000080".to_owned()),
            background: Some("#0f08".to_owned()),
            ..ThemeRule::default()
        }])
        .unwrap();
        let (table, stack) = HighlightScopeTable::from_scope_names(&["constant.numeric"]);

        assert_eq!(
            theme.resolve(&table, stack),
            ResolvedSyntaxStyle {
                foreground: Some(RgbColor {
                    red: 0xff,
                    green: 0,
                    blue: 0,
                }),
                background: Some(RgbColor {
                    red: 0,
                    green: 0xff,
                    blue: 0,
                }),
                modifiers: SyntaxModifiers::empty(),
            }
        );
    }

    #[test]
    fn user_syntax_rules_use_the_theme_selector_engine() {
        let theme = TextMateTheme::from_rules(&[
            ThemeRule {
                scope: "source.test support.function".to_owned(),
                foreground: Some("#123456".to_owned()),
                font_style: Some("bold italic".to_owned()),
                ..ThemeRule::default()
            },
            ThemeRule {
                scope: "support.function.deep".to_owned(),
                font_style: Some(String::new()),
                ..ThemeRule::default()
            },
        ])
        .unwrap();
        let (table, stack) = HighlightScopeTable::from_scope_names(&[
            "source.test",
            "meta.middle",
            "support.function.deep",
        ]);
        let matched = theme.resolve_with_match(&table, stack);
        assert_eq!(matched.selector, Some("support.function.deep"));
        assert!(matched.foreground_matched);
        assert!(matched.modifiers_matched);
        assert_eq!(matched.style.modifiers, SyntaxModifiers::empty());
        assert_eq!(
            matched.style.foreground,
            Some(RgbColor {
                red: 0x12,
                green: 0x34,
                blue: 0x56
            })
        );
        assert_eq!(
            matched.score.as_ref().map(|score| score.target_depth),
            Some(3)
        );
    }
}
