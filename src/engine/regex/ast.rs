use std::collections::BTreeMap;
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RegexFlags {
    pub case_insensitive: bool,
    pub multi_line: bool,
    pub dot_matches_new_line: bool,
    pub ignore_whitespace: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RegexFeatures {
    pub lookahead: bool,
    pub lookbehind: bool,
    pub backreference: bool,
    pub subroutine: bool,
    pub anchor_a: bool,
    pub anchor_g: bool,
    pub line_anchor: bool,
    pub named_group: bool,
    pub possessive_or_atomic: bool,
    pub inline_flags: bool,
    pub unicode_or_posix_class: bool,
    pub conditional: bool,
    pub unsupported_escape: bool,
}

impl RegexFeatures {
    pub fn requires_fallback(&self) -> bool {
        self.lookahead
            || self.lookbehind
            || self.backreference
            || self.subroutine
            || self.anchor_g
            || self.named_group
            || self.possessive_or_atomic
            || self.conditional
            || self.unsupported_escape
    }

    pub fn reasons(&self) -> Vec<&'static str> {
        let mut reasons = Vec::new();
        if self.lookahead {
            reasons.push("lookahead");
        }
        if self.lookbehind {
            reasons.push("lookbehind");
        }
        if self.backreference {
            reasons.push("backreference");
        }
        if self.subroutine {
            reasons.push("subroutine");
        }
        if self.anchor_g {
            reasons.push("\\G");
        }
        if self.named_group {
            reasons.push("named-group");
        }
        if self.possessive_or_atomic {
            reasons.push("possessive-or-atomic");
        }
        if self.conditional {
            reasons.push("conditional");
        }
        if self.unsupported_escape {
            reasons.push("unsupported-escape");
        }
        reasons
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnchorKind {
    LineStart,
    LineEnd,
    TextStart,
    TextEnd,
    TextEndOrFinalNewline,
    Continuation,
    WordBoundary,
    NotWordBoundary,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LookKind {
    Ahead,
    NotAhead,
    Behind,
    NotBehind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PerlClassKind {
    Digit,
    NotDigit,
    Space,
    NotSpace,
    Word,
    NotWord,
    HorizontalSpace,
    NotHorizontalSpace,
    VerticalSpace,
    NotVerticalSpace,
    NotNewline,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClassAtom {
    Char(char),
    Range(char, char),
    Perl(PerlClassKind),
    Posix { name: String, negated: bool },
    Unicode { name: String, negated: bool },
    Nested(Box<CharClass>),
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CharClass {
    pub negated: bool,
    /// Additional union terms intersected with `atoms` by Oniguruma's `&&`
    /// operator. Each inner vector is a union, so `[ab&&bc&&cd]` is stored as
    /// `ab AND bc AND cd`.
    pub intersections: Vec<Vec<ClassAtom>>,
    pub atoms: Vec<ClassAtom>,
}

impl CharClass {
    fn current_union_mut(&mut self) -> &mut Vec<ClassAtom> {
        self.intersections.last_mut().unwrap_or(&mut self.atoms)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Backref {
    Number(u32),
    Name(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AstPathStep {
    Branch(usize),
    Child,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubroutineCall {
    pub target: Backref,
    pub(crate) target_path: Option<Vec<AstPathStep>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ast {
    Empty,
    Literal(String),
    Dot,
    Grapheme,
    Class(CharClass),
    Anchor(AnchorKind),
    Concat(Vec<Ast>),
    Alternation(Vec<Ast>),
    Repeat {
        node: Box<Ast>,
        min: usize,
        max: Option<usize>,
        greedy: bool,
        possessive: bool,
        atomic: bool,
    },
    Group {
        index: Option<u32>,
        name: Option<String>,
        child: Box<Ast>,
    },
    Look {
        kind: LookKind,
        child: Box<Ast>,
    },
    Backref(Backref),
    Conditional {
        condition: Backref,
        matched: Box<Ast>,
        unmatched: Box<Ast>,
    },
    Subroutine(Box<SubroutineCall>),
    Flags {
        flags: RegexFlags,
        child: Box<Ast>,
    },
    Unsupported(String),
}

fn collect_group_paths(
    ast: &Ast,
    path: &mut Vec<AstPathStep>,
    paths: &mut BTreeMap<u32, Vec<AstPathStep>>,
) {
    if let Ast::Group {
        index: Some(index), ..
    } = ast
    {
        paths.insert(*index, path.clone());
    }
    match ast {
        Ast::Concat(nodes) | Ast::Alternation(nodes) => {
            for (index, node) in nodes.iter().enumerate() {
                path.push(AstPathStep::Branch(index));
                collect_group_paths(node, path, paths);
                path.pop();
            }
        }
        Ast::Conditional {
            matched, unmatched, ..
        } => {
            path.push(AstPathStep::Branch(0));
            collect_group_paths(matched, path, paths);
            path.pop();
            path.push(AstPathStep::Branch(1));
            collect_group_paths(unmatched, path, paths);
            path.pop();
        }
        Ast::Repeat { node, .. }
        | Ast::Group { child: node, .. }
        | Ast::Look { child: node, .. }
        | Ast::Flags { child: node, .. } => {
            path.push(AstPathStep::Child);
            collect_group_paths(node, path, paths);
            path.pop();
        }
        Ast::Empty
        | Ast::Literal(_)
        | Ast::Dot
        | Ast::Grapheme
        | Ast::Class(_)
        | Ast::Anchor(_)
        | Ast::Backref(_)
        | Ast::Subroutine(_)
        | Ast::Unsupported(_) => {}
    }
}

fn resolve_subroutine_paths(
    ast: &mut Ast,
    named_captures: &BTreeMap<String, u32>,
    paths: &BTreeMap<u32, Vec<AstPathStep>>,
) {
    if let Ast::Subroutine(call) = ast {
        let target = match &call.target {
            Backref::Number(index) => Some(*index),
            Backref::Name(name) => named_captures.get(name).copied(),
        };
        call.target_path = target.and_then(|index| paths.get(&index).cloned());
    }
    match ast {
        Ast::Concat(nodes) | Ast::Alternation(nodes) => {
            for node in nodes {
                resolve_subroutine_paths(node, named_captures, paths);
            }
        }
        Ast::Conditional {
            matched, unmatched, ..
        } => {
            resolve_subroutine_paths(matched, named_captures, paths);
            resolve_subroutine_paths(unmatched, named_captures, paths);
        }
        Ast::Repeat { node, .. }
        | Ast::Group { child: node, .. }
        | Ast::Look { child: node, .. }
        | Ast::Flags { child: node, .. } => {
            resolve_subroutine_paths(node, named_captures, paths);
        }
        _ => {}
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedRegex {
    pub source: String,
    pub ast: Ast,
    pub features: RegexFeatures,
    pub flags: RegexFlags,
    pub capture_count: u32,
    pub named_captures: BTreeMap<String, u32>,
    pub diagnostics: Vec<String>,
}

impl ParsedRegex {
    pub fn route_reason(&self) -> &'static str {
        if self.features.requires_fallback() {
            "fallback"
        } else {
            "dfa"
        }
    }
}

pub fn parse(pattern: &str) -> ParsedRegex {
    Parser::new(pattern).parse()
}

pub fn classify_features(pattern: &str) -> RegexFeatures {
    parse(pattern).features
}

pub(crate) fn has_case_insensitive_scope(ast: &Ast) -> bool {
    match ast {
        Ast::Flags { flags, child } => flags.case_insensitive || has_case_insensitive_scope(child),
        Ast::Concat(nodes) | Ast::Alternation(nodes) => {
            nodes.iter().any(has_case_insensitive_scope)
        }
        Ast::Conditional {
            matched, unmatched, ..
        } => has_case_insensitive_scope(matched) || has_case_insensitive_scope(unmatched),
        Ast::Repeat { node, .. }
        | Ast::Group { child: node, .. }
        | Ast::Look { child: node, .. } => has_case_insensitive_scope(node),
        _ => false,
    }
}

pub(crate) fn uniform_effective_flags(ast: &Ast) -> Option<RegexFlags> {
    fn visit(ast: &Ast, inherited: RegexFlags) -> Result<Option<RegexFlags>, ()> {
        match ast {
            Ast::Empty => Ok(None),
            Ast::Flags { flags, child } => visit(child, *flags),
            Ast::Concat(nodes) | Ast::Alternation(nodes) => {
                let mut uniform = None;
                for node in nodes {
                    if let Some(flags) = visit(node, inherited)? {
                        if uniform.is_some_and(|uniform| uniform != flags) {
                            return Err(());
                        }
                        uniform = Some(flags);
                    }
                }
                Ok(uniform)
            }
            Ast::Conditional {
                matched, unmatched, ..
            } => {
                let left = visit(matched, inherited)?;
                let right = visit(unmatched, inherited)?;
                if left.is_some() && right.is_some() && left != right {
                    Err(())
                } else {
                    Ok(left.or(right))
                }
            }
            Ast::Repeat { node, .. }
            | Ast::Group { child: node, .. }
            | Ast::Look { child: node, .. } => visit(node, inherited),
            _ => Ok(Some(inherited)),
        }
    }
    visit(ast, RegexFlags::default()).ok().flatten()
}

struct Parser<'a> {
    source: &'a str,
    chars: Vec<char>,
    pos: usize,
    next_capture: u32,
    named_captures: BTreeMap<String, u32>,
    features: RegexFeatures,
    flags: RegexFlags,
    diagnostics: Vec<String>,
}

impl<'a> Parser<'a> {
    fn new(source: &'a str) -> Self {
        Self {
            source,
            chars: source.chars().collect(),
            pos: 0,
            next_capture: 1,
            named_captures: BTreeMap::new(),
            features: RegexFeatures::default(),
            flags: RegexFlags::default(),
            diagnostics: Vec::new(),
        }
    }

    fn parse(mut self) -> ParsedRegex {
        let mut ast = self.parse_alternation(None);
        if self.pos < self.chars.len() {
            self.diagnostics
                .push(format!("trailing input at char {}", self.pos));
        }
        if self.features.subroutine {
            let mut paths = BTreeMap::new();
            collect_group_paths(&ast, &mut Vec::new(), &mut paths);
            resolve_subroutine_paths(&mut ast, &self.named_captures, &paths);
        }
        ParsedRegex {
            source: self.source.to_owned(),
            ast,
            features: self.features,
            // Option changes are represented at the AST node where they are
            // active.  The VM therefore always starts with Oniguruma's
            // defaults; using the final parser state here incorrectly applies
            // a trailing `(?i)`/`(?-i)` choice to the entire expression.
            flags: RegexFlags::default(),
            capture_count: self.next_capture.saturating_sub(1),
            named_captures: self.named_captures,
            diagnostics: self.diagnostics,
        }
    }

    fn parse_alternation(&mut self, terminator: Option<char>) -> Ast {
        let mut branches = Vec::new();
        loop {
            branches.push(self.parse_concat(terminator));
            if self.peek() == Some('|') {
                self.bump();
                continue;
            }
            break;
        }
        normalize_flag_changes(branches)
    }

    fn parse_concat(&mut self, terminator: Option<char>) -> Ast {
        let mut nodes = Vec::new();
        while let Some(ch) = self.peek() {
            if Some(ch) == terminator || ch == '|' {
                break;
            }
            if self.flags.ignore_whitespace && ch.is_whitespace() {
                self.bump();
                continue;
            }
            if self.flags.ignore_whitespace && ch == '#' {
                while self.peek().is_some_and(|next| next != '\n') {
                    self.bump();
                }
                continue;
            }
            push_concat_node(&mut nodes, self.parse_repeat());
        }
        match nodes.len() {
            0 => Ast::Empty,
            1 => nodes.pop().expect("one node"),
            _ => Ast::Concat(nodes),
        }
    }

    fn parse_repeat(&mut self) -> Ast {
        let active_flags = self.flags;
        let atom_is_group = self.peek() == Some('(');
        let mut node = self.parse_atom();
        // Bare option groups such as `(?i)` affect the remainder of the
        // enclosing subexpression. Snapshot those options on consuming atoms.
        // Groups and lookarounds snapshot their children while parsing, so an
        // outer wrapper here would overwrite an inner `(?-i)` transition.
        if !atom_is_group
            && active_flags != RegexFlags::default()
            && matches!(
                node,
                Ast::Literal(_)
                    | Ast::Dot
                    | Ast::Grapheme
                    | Ast::Class(_)
                    | Ast::Anchor(_)
                    | Ast::Backref(_)
                    | Ast::Subroutine(_)
            )
        {
            node = Ast::Flags {
                flags: active_flags,
                child: Box::new(node),
            };
        }
        while let Some(ch) = self.peek() {
            let quantifier = match ch {
                '*' => {
                    self.bump();
                    Some((0, None, false))
                }
                '+' => {
                    self.bump();
                    Some((1, None, false))
                }
                '?' => {
                    self.bump();
                    Some((0, Some(1), false))
                }
                '{' => self.parse_braced_quantifier(),
                _ => None,
            };
            let Some((min, max, is_braced_exact)) = quantifier else {
                break;
            };
            // Oniguruma keeps the historical Ruby interpretation of `{n}?`:
            // the exact repetition is optional (zero or exactly n copies),
            // rather than a lazily evaluated exact repetition. TextMate
            // grammars rely on this, notably LaTeX hyperlink argument rules.
            if is_braced_exact && self.peek() == Some('?') {
                self.bump();
                let exact = Ast::Repeat {
                    node: Box::new(node),
                    min,
                    max,
                    greedy: true,
                    possessive: false,
                    atomic: false,
                };
                node = Ast::Repeat {
                    node: Box::new(exact),
                    min: 0,
                    max: Some(1),
                    greedy: true,
                    possessive: false,
                    atomic: false,
                };
                continue;
            }
            let mut greedy = true;
            let mut possessive = false;
            if self.peek() == Some('?') {
                self.bump();
                greedy = false;
            } else if self.peek() == Some('+') {
                self.bump();
                possessive = true;
                self.features.possessive_or_atomic = true;
            }
            node = Ast::Repeat {
                node: Box::new(node),
                min,
                max,
                greedy,
                possessive,
                atomic: false,
            };
        }
        node
    }

    fn parse_atom(&mut self) -> Ast {
        let Some(ch) = self.bump() else {
            return Ast::Empty;
        };
        match ch {
            '(' => self.parse_group(),
            '[' => self.parse_class(),
            '.' => Ast::Dot,
            '^' => {
                self.features.line_anchor = true;
                Ast::Anchor(AnchorKind::LineStart)
            }
            '$' => {
                self.features.line_anchor = true;
                Ast::Anchor(AnchorKind::LineEnd)
            }
            '\\' => self.parse_escape(false),
            ')' => {
                self.diagnostics.push(format!(
                    "unmatched ')' at char {}",
                    self.pos.saturating_sub(1)
                ));
                Ast::Unsupported("unmatched ')'".to_owned())
            }
            ch => Ast::Literal(ch.to_string()),
        }
    }

    fn parse_group(&mut self) -> Ast {
        if self.peek() != Some('?') {
            let index = self.alloc_capture(None);
            let outer = self.flags;
            let child = self.parse_alternation(Some(')'));
            self.flags = outer;
            self.expect(')');
            return Ast::Group {
                index: Some(index),
                name: None,
                child: Box::new(child),
            };
        }
        self.bump();
        match self.peek() {
            Some(':') => {
                self.bump();
                let outer = self.flags;
                let child = self.parse_alternation(Some(')'));
                self.flags = outer;
                self.expect(')');
                child
            }
            Some('=') => {
                self.bump();
                self.features.lookahead = true;
                let outer = self.flags;
                let child = self.parse_alternation(Some(')'));
                self.flags = outer;
                self.expect(')');
                Ast::Look {
                    kind: LookKind::Ahead,
                    child: Box::new(child),
                }
            }
            Some('!') => {
                self.bump();
                self.features.lookahead = true;
                let outer = self.flags;
                let child = self.parse_alternation(Some(')'));
                self.flags = outer;
                self.expect(')');
                Ast::Look {
                    kind: LookKind::NotAhead,
                    child: Box::new(child),
                }
            }
            Some('<') => {
                self.bump();
                match self.peek() {
                    Some('=') => {
                        self.bump();
                        self.features.lookbehind = true;
                        let outer = self.flags;
                        let child = self.parse_alternation(Some(')'));
                        self.flags = outer;
                        self.expect(')');
                        Ast::Look {
                            kind: LookKind::Behind,
                            child: Box::new(child),
                        }
                    }
                    Some('!') => {
                        self.bump();
                        self.features.lookbehind = true;
                        let outer = self.flags;
                        let child = self.parse_alternation(Some(')'));
                        self.flags = outer;
                        self.expect(')');
                        Ast::Look {
                            kind: LookKind::NotBehind,
                            child: Box::new(child),
                        }
                    }
                    _ => {
                        let name = self.take_until('>');
                        self.expect('>');
                        self.features.named_group = true;
                        let index = self.alloc_capture(Some(name.clone()));
                        let outer = self.flags;
                        let child = self.parse_alternation(Some(')'));
                        self.flags = outer;
                        self.expect(')');
                        Ast::Group {
                            index: Some(index),
                            name: Some(name),
                            child: Box::new(child),
                        }
                    }
                }
            }
            Some('P') if self.peek_n(1) == Some('<') => {
                self.bump();
                self.bump();
                let name = self.take_until('>');
                self.expect('>');
                self.features.named_group = true;
                let index = self.alloc_capture(Some(name.clone()));
                let outer = self.flags;
                let child = self.parse_alternation(Some(')'));
                self.flags = outer;
                self.expect(')');
                Ast::Group {
                    index: Some(index),
                    name: Some(name),
                    child: Box::new(child),
                }
            }
            Some('>') => {
                self.bump();
                self.features.possessive_or_atomic = true;
                let outer = self.flags;
                let child = self.parse_alternation(Some(')'));
                self.flags = outer;
                self.expect(')');
                Ast::Repeat {
                    node: Box::new(child),
                    min: 1,
                    max: Some(1),
                    greedy: true,
                    possessive: true,
                    atomic: true,
                }
            }
            Some('#') => {
                self.bump();
                self.take_until(')');
                self.expect(')');
                Ast::Empty
            }
            Some('(') => {
                self.features.conditional = true;
                let outer = self.flags;
                let conditional = self.parse_conditional();
                self.flags = outer;
                conditional
            }
            Some(ch) if is_flag_char(ch) || ch == '-' => self.parse_flag_group(),
            _ => {
                self.features.unsupported_escape = true;
                let rest = self.take_until(')');
                self.expect(')');
                Ast::Unsupported(format!("unsupported group (?{rest})"))
            }
        }
    }

    fn parse_conditional(&mut self) -> Ast {
        self.bump(); // condition's opening `(`
        let raw = self.take_until(')');
        self.expect(')');
        let condition = if let Ok(index) = raw.parse::<u32>() {
            Some(Backref::Number(index))
        } else if let Some(name) = raw.strip_prefix('<').and_then(|raw| raw.strip_suffix('>')) {
            Some(Backref::Name(name.to_owned()))
        } else {
            raw.strip_prefix('\'')
                .and_then(|raw| raw.strip_suffix('\''))
                .map(|name| Backref::Name(name.to_owned()))
        };

        let matched = self.parse_concat(Some(')'));
        let unmatched = if self.peek() == Some('|') {
            self.bump();
            self.parse_alternation(Some(')'))
        } else {
            Ast::Empty
        };
        self.expect(')');
        condition.map_or_else(
            || {
                self.diagnostics
                    .push(format!("unsupported conditional test ({raw})"));
                Ast::Unsupported("conditional-test".to_owned())
            },
            |condition| Ast::Conditional {
                condition,
                matched: Box::new(matched),
                unmatched: Box::new(unmatched),
            },
        )
    }

    fn parse_flag_group(&mut self) -> Ast {
        let mut local = self.flags;
        let mut negating = false;
        while let Some(ch) = self.peek() {
            match ch {
                'i' | 'm' | 's' | 'x' => {
                    self.features.inline_flags = true;
                    self.bump();
                    apply_flag(&mut local, ch, !negating);
                }
                '-' => {
                    self.features.inline_flags = true;
                    self.bump();
                    negating = true;
                }
                ':' => {
                    self.bump();
                    let outer = self.flags;
                    self.flags = local;
                    let child = self.parse_alternation(Some(')'));
                    self.flags = outer;
                    self.expect(')');
                    // Keep an explicit snapshot even when `local` is the
                    // default. An enclosing bare option change may otherwise
                    // wrap this child and erase a scoped `(?-i:...)` reset.
                    return Ast::Flags {
                        flags: local,
                        child: Box::new(child),
                    };
                }
                ')' => {
                    self.bump();
                    self.flags = local;
                    return flag_change_marker(local);
                }
                _ => break,
            }
        }
        self.features.unsupported_escape = true;
        Ast::Unsupported("malformed inline flags".to_owned())
    }

    fn parse_class(&mut self) -> Ast {
        Ast::Class(self.parse_class_body())
    }

    fn parse_class_body(&mut self) -> CharClass {
        let mut class = CharClass::default();
        if self.peek() == Some('^') {
            self.bump();
            class.negated = true;
        }
        // Oniguruma treats `]` as a literal when it is the first class atom,
        // e.g. `[]),;}]`. Several VS Code TypeScript rules rely on this form.
        if self.peek() == Some(']') {
            self.bump();
            class.atoms.push(ClassAtom::Char(']'));
        }
        while let Some(ch) = self.peek() {
            if ch == ']' {
                self.bump();
                break;
            }
            if ch == '&' && self.peek_n(1) == Some('&') {
                self.bump();
                self.bump();
                class.intersections.push(Vec::new());
                continue;
            }
            let atom = self.read_class_atom();
            if let ClassAtom::Char(start) = atom {
                if self.peek() == Some('-') && self.peek_n(1).is_some_and(|next| next != ']') {
                    self.bump();
                    let end_atom = self.read_class_atom();
                    if let ClassAtom::Char(end) = end_atom {
                        class.current_union_mut().push(ClassAtom::Range(start, end));
                    } else {
                        let union = class.current_union_mut();
                        union.push(ClassAtom::Char(start));
                        union.push(ClassAtom::Char('-'));
                        union.push(end_atom);
                    }
                    continue;
                }
                class.current_union_mut().push(ClassAtom::Char(start));
            } else {
                class.current_union_mut().push(atom);
            }
        }
        class
    }

    fn read_class_atom(&mut self) -> ClassAtom {
        let Some(ch) = self.peek() else {
            return ClassAtom::Char('\0');
        };
        if ch == '[' && self.peek_n(1) == Some(':') {
            self.bump();
            self.bump();
            let mut name = String::new();
            let mut negated = false;
            if self.peek() == Some('^') {
                self.bump();
                negated = true;
            }
            while let Some(next) = self.peek() {
                if next == ':' && self.peek_n(1) == Some(']') {
                    self.bump();
                    self.bump();
                    break;
                }
                name.push(next);
                self.bump();
            }
            self.features.unicode_or_posix_class = true;
            return ClassAtom::Posix { name, negated };
        }
        if ch == '[' {
            self.bump();
            return ClassAtom::Nested(Box::new(self.parse_class_body()));
        }
        if ch == '\\' {
            self.bump();
            return self.class_escape();
        }
        self.bump();
        ClassAtom::Char(ch)
    }

    fn class_escape(&mut self) -> ClassAtom {
        let Some(ch) = self.bump() else {
            return ClassAtom::Char('\\');
        };
        match ch {
            'd' => ClassAtom::Perl(PerlClassKind::Digit),
            'D' => ClassAtom::Perl(PerlClassKind::NotDigit),
            's' => ClassAtom::Perl(PerlClassKind::Space),
            'S' => ClassAtom::Perl(PerlClassKind::NotSpace),
            'w' => ClassAtom::Perl(PerlClassKind::Word),
            'W' => ClassAtom::Perl(PerlClassKind::NotWord),
            'h' => ClassAtom::Perl(PerlClassKind::HorizontalSpace),
            'H' => ClassAtom::Perl(PerlClassKind::NotHorizontalSpace),
            'v' => ClassAtom::Perl(PerlClassKind::VerticalSpace),
            'V' => ClassAtom::Perl(PerlClassKind::NotVerticalSpace),
            'N' => ClassAtom::Perl(PerlClassKind::NotNewline),
            'p' | 'P' if self.peek() == Some('{') => {
                self.bump();
                let name = self.take_until('}');
                self.expect('}');
                self.features.unicode_or_posix_class = true;
                ClassAtom::Unicode {
                    name,
                    negated: ch == 'P',
                }
            }
            'x' => {
                let digits = if self.peek() == Some('{') {
                    self.bump();
                    let digits = self.take_until('}');
                    self.expect('}');
                    digits
                } else {
                    self.take_hex_digits(2)
                };
                let chars = digits
                    .split_ascii_whitespace()
                    .map(hex_char)
                    .collect::<Option<Vec<_>>>();
                match chars.as_deref() {
                    Some([ch]) => ClassAtom::Char(*ch),
                    Some(chars) if !chars.is_empty() => ClassAtom::Nested(Box::new(CharClass {
                        negated: false,
                        intersections: Vec::new(),
                        atoms: chars.iter().copied().map(ClassAtom::Char).collect(),
                    })),
                    _ => ClassAtom::Char('x'),
                }
            }
            'u' => {
                let digits = self.take_hex_digits(4);
                ClassAtom::Char(hex_char(&digits).unwrap_or('u'))
            }
            _ => ClassAtom::Char(unescape_char(ch)),
        }
    }

    fn parse_escape(&mut self, in_class: bool) -> Ast {
        let Some(ch) = self.bump() else {
            return Ast::Literal("\\".to_owned());
        };
        match ch {
            'A' => {
                self.features.anchor_a = true;
                Ast::Anchor(AnchorKind::TextStart)
            }
            'G' => {
                self.features.anchor_g = true;
                Ast::Anchor(AnchorKind::Continuation)
            }
            'z' => Ast::Anchor(AnchorKind::TextEnd),
            'Z' => Ast::Anchor(AnchorKind::TextEndOrFinalNewline),
            'b' if !in_class => Ast::Anchor(AnchorKind::WordBoundary),
            'B' if !in_class => Ast::Anchor(AnchorKind::NotWordBoundary),
            'd' => Ast::Class(CharClass {
                negated: false,
                intersections: Vec::new(),
                atoms: vec![ClassAtom::Perl(PerlClassKind::Digit)],
            }),
            'D' => Ast::Class(CharClass {
                negated: false,
                intersections: Vec::new(),
                atoms: vec![ClassAtom::Perl(PerlClassKind::NotDigit)],
            }),
            's' => Ast::Class(CharClass {
                negated: false,
                intersections: Vec::new(),
                atoms: vec![ClassAtom::Perl(PerlClassKind::Space)],
            }),
            'S' => Ast::Class(CharClass {
                negated: false,
                intersections: Vec::new(),
                atoms: vec![ClassAtom::Perl(PerlClassKind::NotSpace)],
            }),
            'w' => Ast::Class(CharClass {
                negated: false,
                intersections: Vec::new(),
                atoms: vec![ClassAtom::Perl(PerlClassKind::Word)],
            }),
            'W' => Ast::Class(CharClass {
                negated: false,
                intersections: Vec::new(),
                atoms: vec![ClassAtom::Perl(PerlClassKind::NotWord)],
            }),
            'h' => Ast::Class(CharClass {
                negated: false,
                intersections: Vec::new(),
                atoms: vec![ClassAtom::Perl(PerlClassKind::HorizontalSpace)],
            }),
            'H' => Ast::Class(CharClass {
                negated: false,
                intersections: Vec::new(),
                atoms: vec![ClassAtom::Perl(PerlClassKind::NotHorizontalSpace)],
            }),
            'v' => Ast::Class(CharClass {
                negated: false,
                intersections: Vec::new(),
                atoms: vec![ClassAtom::Perl(PerlClassKind::VerticalSpace)],
            }),
            'V' => Ast::Class(CharClass {
                negated: false,
                intersections: Vec::new(),
                atoms: vec![ClassAtom::Perl(PerlClassKind::NotVerticalSpace)],
            }),
            'N' => Ast::Class(CharClass {
                negated: false,
                intersections: Vec::new(),
                atoms: vec![ClassAtom::Perl(PerlClassKind::NotNewline)],
            }),
            'X' => Ast::Grapheme,
            'p' | 'P' if self.peek() == Some('{') => {
                self.bump();
                let name = self.take_until('}');
                self.expect('}');
                self.features.unicode_or_posix_class = true;
                Ast::Class(CharClass {
                    negated: false,
                    intersections: Vec::new(),
                    atoms: vec![ClassAtom::Unicode {
                        name,
                        negated: ch == 'P',
                    }],
                })
            }
            'k' if self.peek() == Some('<') => {
                self.bump();
                let name = self.take_until('>');
                self.expect('>');
                self.features.backreference = true;
                Ast::Backref(Backref::Name(name))
            }
            'g' if self.peek() == Some('<') => {
                self.bump();
                let name = self.take_until('>');
                self.expect('>');
                self.features.subroutine = true;
                if let Ok(index) = name.parse::<u32>() {
                    Ast::Subroutine(Box::new(SubroutineCall {
                        target: Backref::Number(index),
                        target_path: None,
                    }))
                } else {
                    Ast::Subroutine(Box::new(SubroutineCall {
                        target: Backref::Name(name),
                        target_path: None,
                    }))
                }
            }
            '1'..='9' => {
                let mut number = ch.to_digit(10).unwrap_or(0);
                while let Some(next @ '0'..='9') = self.peek() {
                    self.bump();
                    number = number * 10 + next.to_digit(10).unwrap_or(0);
                }
                self.features.backreference = true;
                Ast::Backref(Backref::Number(number))
            }
            'x' if self.peek() == Some('{') => {
                self.bump();
                let digits = self.take_until('}');
                self.expect('}');
                Ast::Literal(hex_char(&digits).unwrap_or('\u{FFFD}').to_string())
            }
            'x' => {
                let digits = self.take_hex_digits(2);
                Ast::Literal(hex_char(&digits).unwrap_or('x').to_string())
            }
            'u' => {
                let digits = self.take_hex_digits(4);
                Ast::Literal(hex_char(&digits).unwrap_or('u').to_string())
            }
            'R' => Ast::Alternation(vec![
                Ast::Literal("\r\n".to_owned()),
                Ast::Class(CharClass {
                    negated: false,
                    intersections: Vec::new(),
                    atoms: vec![
                        ClassAtom::Char('\n'),
                        ClassAtom::Char('\r'),
                        ClassAtom::Char('\u{000B}'),
                        ClassAtom::Char('\u{000C}'),
                        ClassAtom::Char('\u{0085}'),
                        ClassAtom::Char('\u{2028}'),
                        ClassAtom::Char('\u{2029}'),
                    ],
                }),
            ]),
            _ => Ast::Literal(unescape_char(ch).to_string()),
        }
    }

    fn parse_braced_quantifier(&mut self) -> Option<(usize, Option<usize>, bool)> {
        let saved = self.pos;
        self.bump(); // {
        let min_digits = self.take_digits();
        if min_digits.is_empty() {
            if self.peek() != Some(',') {
                self.pos = saved;
                return None;
            }
            self.bump();
            let max_digits = self.take_digits();
            let Some(max) = max_digits.parse::<usize>().ok() else {
                self.pos = saved;
                return None;
            };
            if self.peek() != Some('}') {
                self.pos = saved;
                return None;
            }
            self.bump();
            return Some((0, Some(max), false));
        }
        let min = min_digits.parse::<usize>().ok()?;
        let is_braced_exact = self.peek() != Some(',');
        let max = if !is_braced_exact {
            self.bump();
            let max_digits = self.take_digits();
            if max_digits.is_empty() {
                None
            } else {
                Some(max_digits.parse::<usize>().ok()?)
            }
        } else {
            Some(min)
        };
        if self.peek() != Some('}') {
            self.pos = saved;
            return None;
        }
        self.bump();
        Some((min, max, is_braced_exact))
    }

    fn alloc_capture(&mut self, name: Option<String>) -> u32 {
        let index = self.next_capture;
        self.next_capture += 1;
        if let Some(name) = name {
            self.named_captures.insert(name, index);
        }
        index
    }

    fn take_digits(&mut self) -> String {
        let mut out = String::new();
        while let Some(ch) = self.peek().filter(|ch| ch.is_ascii_digit()) {
            out.push(ch);
            self.bump();
        }
        out
    }

    fn take_hex_digits(&mut self, limit: usize) -> String {
        let mut out = String::new();
        for _ in 0..limit {
            let Some(ch) = self.peek().filter(|ch| ch.is_ascii_hexdigit()) else {
                break;
            };
            out.push(ch);
            self.bump();
        }
        out
    }

    fn take_until(&mut self, terminator: char) -> String {
        let mut out = String::new();
        while let Some(ch) = self.peek() {
            if ch == terminator {
                break;
            }
            out.push(ch);
            self.bump();
        }
        out
    }

    fn expect(&mut self, expected: char) {
        if self.peek() == Some(expected) {
            self.bump();
        } else {
            self.diagnostics
                .push(format!("expected '{expected}' at char {}", self.pos));
        }
    }

    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    fn peek_n(&self, n: usize) -> Option<char> {
        self.chars.get(self.pos + n).copied()
    }

    fn bump(&mut self) -> Option<char> {
        let ch = self.peek()?;
        self.pos += 1;
        Some(ch)
    }
}

fn normalize_flag_changes(mut branches: Vec<Ast>) -> Ast {
    for branch_index in 0..branches.len() {
        let branch = std::mem::replace(&mut branches[branch_index], Ast::Empty);
        let mut nodes = match branch {
            Ast::Concat(nodes) => nodes,
            node => vec![node],
        };
        let Some(change_index) = nodes
            .iter()
            .position(|node| flag_change_flags(node).is_some())
        else {
            branches[branch_index] = concat_ast(nodes);
            continue;
        };
        let flags = flag_change_flags(&nodes.remove(change_index)).expect("flag marker");
        let prefix = nodes.drain(..change_index).collect::<Vec<_>>();
        let mut remainder = vec![concat_ast(nodes)];
        remainder.extend(branches.drain(branch_index + 1..));
        let scoped_remainder = Ast::Flags {
            flags,
            child: Box::new(normalize_flag_changes(remainder)),
        };
        let mut transformed = prefix;
        transformed.push(scoped_remainder);
        branches[branch_index] = concat_ast(transformed);
        break;
    }
    alternation_ast(branches)
}

fn flag_change_marker(flags: RegexFlags) -> Ast {
    let bits = u8::from(flags.case_insensitive)
        | (u8::from(flags.multi_line) << 1)
        | (u8::from(flags.dot_matches_new_line) << 2)
        | (u8::from(flags.ignore_whitespace) << 3);
    Ast::Unsupported(format!("\0option-change:{bits}"))
}

fn flag_change_flags(ast: &Ast) -> Option<RegexFlags> {
    let Ast::Unsupported(marker) = ast else {
        return None;
    };
    let bits = marker
        .strip_prefix("\0option-change:")?
        .parse::<u8>()
        .ok()?;
    Some(RegexFlags {
        case_insensitive: bits & 1 != 0,
        multi_line: bits & 2 != 0,
        dot_matches_new_line: bits & 4 != 0,
        ignore_whitespace: bits & 8 != 0,
    })
}

fn concat_ast(mut nodes: Vec<Ast>) -> Ast {
    match nodes.len() {
        0 => Ast::Empty,
        1 => nodes.pop().expect("one concat node"),
        _ => Ast::Concat(nodes),
    }
}

fn alternation_ast(mut branches: Vec<Ast>) -> Ast {
    match branches.len() {
        0 => Ast::Empty,
        1 => branches.pop().expect("one alternation branch"),
        _ => Ast::Alternation(branches),
    }
}

fn push_concat_node(nodes: &mut Vec<Ast>, node: Ast) {
    if let Ast::Literal(literal) = node {
        if let Some(Ast::Literal(previous)) = nodes.last_mut() {
            previous.push_str(&literal);
        } else {
            nodes.push(Ast::Literal(literal));
        }
    } else if let Ast::Flags { flags, child } = node {
        // Keep option snapshots compact. Without this, `(?i:keyword)` becomes
        // one flag node per scalar and defeats literal/alternation fast paths.
        match *child {
            Ast::Literal(literal) => {
                if let Some(Ast::Flags {
                    flags: previous_flags,
                    child: previous_child,
                }) = nodes.last_mut()
                    && *previous_flags == flags
                    && let Ast::Literal(previous) = previous_child.as_mut()
                {
                    previous.push_str(&literal);
                } else {
                    nodes.push(Ast::Flags {
                        flags,
                        child: Box::new(Ast::Literal(literal)),
                    });
                }
            }
            child => nodes.push(Ast::Flags {
                flags,
                child: Box::new(child),
            }),
        }
    } else {
        nodes.push(node);
    }
}

fn is_flag_char(ch: char) -> bool {
    matches!(ch, 'i' | 'm' | 's' | 'x')
}

fn apply_flag(flags: &mut RegexFlags, flag: char, value: bool) {
    match flag {
        'i' => flags.case_insensitive = value,
        'm' => flags.multi_line = value,
        's' => flags.dot_matches_new_line = value,
        'x' => flags.ignore_whitespace = value,
        _ => {}
    }
}

fn unescape_char(ch: char) -> char {
    match ch {
        'n' => '\n',
        'r' => '\r',
        't' => '\t',
        'f' => '\u{000C}',
        'a' => '\u{0007}',
        'e' => '\u{001B}',
        other => other,
    }
}

fn hex_char(digits: &str) -> Option<char> {
    u32::from_str_radix(digits, 16)
        .ok()
        .and_then(char::from_u32)
}

impl fmt::Display for ParsedRegex {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "source: {}", self.source)?;
        writeln!(f, "route: {}", self.route_reason())?;
        writeln!(f, "captures: {}", self.capture_count)?;
        writeln!(f, "features: {:?}", self.features)?;
        if !self.diagnostics.is_empty() {
            writeln!(f, "diagnostics: {:?}", self.diagnostics)?;
        }
        writeln!(f, "ast: {:#?}", self.ast)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_fallback_features() {
        let parsed = parse(r"(?<=foo)(bar)\1\G");
        assert!(parsed.features.lookbehind);
        assert!(parsed.features.backreference);
        assert!(parsed.features.anchor_g);
        assert!(parsed.features.requires_fallback());
        assert_eq!(parsed.capture_count, 1);
    }

    #[test]
    fn parses_named_captures_and_flags() {
        let parsed = parse(r"(?i:(?<name>foo|bar)+)");
        assert!(parsed.features.named_group);
        assert!(parsed.features.inline_flags);
        assert_eq!(parsed.named_captures.get("name"), Some(&1));
    }

    #[test]
    fn parses_posix_class() {
        let parsed = parse(r"[[:alpha:]_][[:alnum:]_]*");
        assert!(parsed.features.unicode_or_posix_class);
        assert!(!parsed.features.requires_fallback());
    }

    #[test]
    fn parses_leading_closing_bracket_as_a_class_literal() {
        let parsed = parse(r"[]),;}]");
        assert_eq!(
            parsed.ast,
            Ast::Class(CharClass {
                negated: false,
                intersections: Vec::new(),
                atoms: vec![
                    ClassAtom::Char(']'),
                    ClassAtom::Char(')'),
                    ClassAtom::Char(','),
                    ClassAtom::Char(';'),
                    ClassAtom::Char('}'),
                ],
            })
        );
    }

    #[test]
    fn parses_oniguruma_nested_non_ascii_class_and_hex_escapes() {
        let parsed = parse(r"[-A-Z[^\x00-\x7F]]");
        let Ast::Class(class) = parsed.ast else {
            panic!("expected class");
        };
        assert_eq!(class.atoms[0], ClassAtom::Char('-'));
        assert_eq!(class.atoms[1], ClassAtom::Range('A', 'Z'));
        assert_eq!(
            class.atoms[2],
            ClassAtom::Nested(Box::new(CharClass {
                negated: true,
                intersections: Vec::new(),
                atoms: vec![ClassAtom::Range('\0', '\u{7f}')],
            }))
        );
    }

    #[test]
    fn parses_oniguruma_multi_codepoint_hex_class_escape() {
        let parsed = parse(r"[^\x{FEFF FFFE FFFF}]");
        let Ast::Class(class) = parsed.ast else {
            panic!("expected class");
        };
        assert!(class.negated);
        assert!(matches!(
            class.atoms.as_slice(),
            [ClassAtom::Nested(nested)]
                if nested.atoms == [
                    ClassAtom::Char('\u{feff}'),
                    ClassAtom::Char('\u{fffe}'),
                    ClassAtom::Char('\u{ffff}'),
                ]
        ));
    }

    #[test]
    fn parses_nested_class_intersection_as_intersected_unions() {
        let parsed = parse(r#"[[\p{S}\p{P}]&&[^]"'(),;\[_`{}]]+"#);
        let Ast::Repeat { node, .. } = parsed.ast else {
            panic!("expected repeated class");
        };
        let Ast::Class(class) = *node else {
            panic!("expected class");
        };
        assert_eq!(class.atoms.len(), 1);
        assert_eq!(class.intersections.len(), 1);
        assert!(matches!(
            &class.atoms[0],
            ClassAtom::Nested(nested)
                if matches!(
                    nested.atoms.as_slice(),
                    [
                        ClassAtom::Unicode { name: symbol, negated: false },
                        ClassAtom::Unicode { name: punctuation, negated: false },
                    ] if symbol == "S" && punctuation == "P"
                )
        ));
        assert!(matches!(
            class.intersections[0].as_slice(),
            [ClassAtom::Nested(nested)]
                if nested.negated
                    && nested.atoms.first() == Some(&ClassAtom::Char(']'))
                    && nested.atoms.contains(&ClassAtom::Char('['))
        ));
    }

    #[test]
    fn intersection_rhs_is_a_union_and_chained_intersections_are_preserved() {
        let parsed = parse(r"[a-w&&[^c-g]z&&[^x]]");
        let Ast::Class(class) = parsed.ast else {
            panic!("expected class");
        };
        assert_eq!(class.atoms, vec![ClassAtom::Range('a', 'w')]);
        assert_eq!(class.intersections.len(), 2);
        assert!(matches!(
            class.intersections[0].as_slice(),
            [ClassAtom::Nested(nested), ClassAtom::Char('z')]
                if nested.negated && nested.atoms == [ClassAtom::Range('c', 'g')]
        ));
        assert!(matches!(
            class.intersections[1].as_slice(),
            [ClassAtom::Nested(nested)]
                if nested.negated && nested.atoms == [ClassAtom::Char('x')]
        ));
    }

    #[test]
    fn coalesces_adjacent_literals() {
        let parsed = parse("return");
        assert_eq!(parsed.ast, Ast::Literal("return".to_owned()));
    }

    #[test]
    fn does_not_coalesce_across_repeats_or_captures() {
        let parsed = parse("ab+c(d)e");
        let Ast::Concat(nodes) = parsed.ast else {
            panic!("expected concat");
        };
        assert_eq!(nodes.first(), Some(&Ast::Literal("a".to_owned())));
        assert!(matches!(nodes.get(1), Some(Ast::Repeat { .. })));
        assert_eq!(nodes.get(2), Some(&Ast::Literal("c".to_owned())));
        assert!(matches!(nodes.get(3), Some(Ast::Group { .. })));
        assert_eq!(nodes.get(4), Some(&Ast::Literal("e".to_owned())));
    }

    #[test]
    fn parses_oniguruma_omitted_lower_repeat_bound() {
        let parsed = parse("`_{,2}");
        let Ast::Concat(nodes) = parsed.ast else {
            panic!("expected concatenation");
        };
        assert!(matches!(
            &nodes[1],
            Ast::Repeat {
                min: 0,
                max: Some(2),
                ..
            }
        ));
    }

    #[test]
    fn only_lexical_braced_exact_repeat_uses_oniguruma_optional_semantics() {
        let parsed = parse("a{2}?");
        assert!(matches!(
            parsed.ast,
            Ast::Repeat {
                min: 0,
                max: Some(1),
                node,
                ..
            } if matches!(
                *node,
                Ast::Repeat {
                    min: 2,
                    max: Some(2),
                    greedy: true,
                    ..
                }
            )
        ));

        let parsed = parse("a{2,2}?");
        assert!(matches!(
            parsed.ast,
            Ast::Repeat {
                min: 2,
                max: Some(2),
                greedy: false,
                ..
            }
        ));
    }

    #[test]
    fn resolves_named_subroutine_targets_to_ast_paths() {
        let parsed = parse(r"(?<pair>a)\g<pair>");
        let Ast::Concat(nodes) = &parsed.ast else {
            panic!("expected concat");
        };
        let Ast::Subroutine(call) = &nodes[1] else {
            panic!("expected subroutine call");
        };
        assert_eq!(call.target, Backref::Name("pair".to_owned()));
        assert_eq!(call.target_path, Some(vec![AstPathStep::Branch(0)]));
    }

    #[test]
    fn resolves_forward_subroutine_targets_after_parsing() {
        let parsed = parse(r"\g<word>(?<word>a)");
        let Ast::Concat(nodes) = &parsed.ast else {
            panic!("expected concat");
        };
        let Ast::Subroutine(call) = &nodes[0] else {
            panic!("expected subroutine call");
        };
        assert_eq!(call.target_path, Some(vec![AstPathStep::Branch(1)]));
    }
}
