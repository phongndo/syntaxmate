//! Word-context start-class analysis.
//!
//! Classifies every scan position by whether the previous and current
//! characters are word characters, and computes — per pattern — the
//! over-approximate set of position classes where a match could begin. The
//! ordered candidate scan uses the mask to skip anchored attempts that
//! provably cannot start at the current position (for example, a
//! `(?<!\w)keyword` rule in the middle of an identifier, or the C-family
//! comment-or-whitespace separator prefix, whose every branch needs
//! whitespace, a comment, a `\W` boundary, or a line edge).
//!
//! Masks must stay conservative: a dropped bit asserts "no match can start
//! at such a position", so every analysis default is "all classes".
//!
//! The scan consults masks only when both neighboring bytes are ASCII (any
//! non-ASCII neighbor keeps every candidate), so the analysis has to be
//! sound for ASCII word characters (`[0-9A-Za-z_]`); Unicode-only word
//! characters such as combining marks never reach a masked decision.

use super::ast::{AnchorKind, Ast, CharClass, ClassAtom, LookKind, ParsedRegex, PerlClassKind};
use super::is_unicode_word_char;

/// Bit `1 << (prev_word * 2 + cur_word)`. Line edges count as non-word.
pub(crate) const START_CLASS_ALL: u8 = 0b1111;

const PREV_WORD: u8 = 0b1100;
const PREV_NONWORD: u8 = 0b0011;
const CUR_WORD: u8 = 0b1010;
const CUR_NONWORD: u8 = 0b0101;
const BOUNDARY: u8 = 0b0110;
const NOT_BOUNDARY: u8 = 0b1001;

/// Start-class mask for a whole pattern. Always non-zero.
pub(crate) fn start_class_mask(parsed: &ParsedRegex) -> u8 {
    let (mask, continuation) = node_mask(&parsed.ast, START_CLASS_ALL);
    let mask = mask | continuation.unwrap_or(0);
    if mask == 0 {
        // A provably unmatchable start set is more likely an analysis gap
        // than a real grammar pattern; never let it silence a candidate.
        START_CLASS_ALL
    } else {
        mask
    }
}

fn bits_prev(word: bool) -> u8 {
    if word { PREV_WORD } else { PREV_NONWORD }
}

fn bits_cur(word: bool) -> u8 {
    if word { CUR_WORD } else { CUR_NONWORD }
}

fn sides_cur_bits(word: bool, nonword: bool) -> u8 {
    let mut bits = 0;
    if word {
        bits |= CUR_WORD;
    }
    if nonword {
        bits |= CUR_NONWORD;
    }
    bits
}

fn sides_prev_bits(word: bool, nonword: bool) -> u8 {
    let mut bits = 0;
    if word {
        bits |= PREV_WORD;
    }
    if nonword {
        bits |= PREV_NONWORD;
    }
    bits
}

/// Returns the classes at which a match can begin inside `ast` under the
/// accumulated zero-width `constraint`, plus the (possibly narrowed)
/// constraint to carry into the next element when `ast` can match empty.
fn node_mask(ast: &Ast, constraint: u8) -> (u8, Option<u8>) {
    match ast {
        Ast::Empty => (0, Some(constraint)),
        Ast::Literal(literal) => match literal.chars().next() {
            Some(ch) => (constraint & bits_cur(is_unicode_word_char(ch)), None),
            None => (0, Some(constraint)),
        },
        Ast::Class(class) => {
            let (word, nonword) = class_word_sides(class);
            (constraint & sides_cur_bits(word, nonword), None)
        }
        // `.` consumes a character of either word-ness.
        Ast::Dot | Ast::Grapheme => (constraint, None),
        Ast::Anchor(kind) => {
            let narrowed = match kind {
                // `^` / `\A` hold at the line start or right after `\n`;
                // either way the previous character is non-word.
                AnchorKind::LineStart | AnchorKind::TextStart => constraint & bits_prev(false),
                AnchorKind::LineEnd | AnchorKind::TextEnd | AnchorKind::TextEndOrFinalNewline => {
                    constraint & bits_cur(false)
                }
                AnchorKind::Continuation => constraint,
                AnchorKind::WordBoundary => constraint & BOUNDARY,
                AnchorKind::NotWordBoundary => constraint & NOT_BOUNDARY,
            };
            (0, Some(narrowed))
        }
        Ast::Look { kind, child } => {
            let narrowed = match kind {
                LookKind::Ahead => {
                    let sides = first_char_sides(child);
                    if sides.nullable {
                        constraint
                    } else {
                        constraint & sides_cur_bits(sides.word, sides.nonword)
                    }
                }
                // `(?!X)` only tells us something when "current char is a
                // word char" would force X to match: then the guard failing
                // means the next character (or line end) is non-word.
                LookKind::NotAhead => {
                    if negated_look_excludes_word(child, false) {
                        constraint & bits_cur(false)
                    } else {
                        constraint
                    }
                }
                LookKind::Behind => {
                    let sides = last_char_sides(child);
                    if sides.nullable {
                        constraint
                    } else {
                        // Line starts are folded into "previous is non-word",
                        // so the impossible prev=None case stays conservative.
                        constraint & sides_prev_bits(sides.word, sides.nonword)
                    }
                }
                LookKind::NotBehind => {
                    if negated_look_excludes_word(child, true) {
                        constraint & bits_prev(false)
                    } else {
                        constraint
                    }
                }
            };
            (0, Some(narrowed))
        }
        Ast::Concat(nodes) => {
            let mut mask = 0;
            let mut carried = constraint;
            for node in nodes {
                let (node_bits, continuation) = node_mask(node, carried);
                mask |= node_bits;
                match continuation {
                    Some(narrowed) => carried = narrowed,
                    None => return (mask, None),
                }
            }
            (mask, Some(carried))
        }
        Ast::Alternation(branches) => {
            let mut mask = 0;
            let mut continuation: Option<u8> = None;
            for branch in branches {
                let (branch_bits, branch_continuation) = node_mask(branch, constraint);
                mask |= branch_bits;
                if let Some(narrowed) = branch_continuation {
                    continuation = Some(continuation.unwrap_or(0) | narrowed);
                }
            }
            (mask, continuation)
        }
        Ast::Repeat { node, min, max, .. } => {
            if *max == Some(0) {
                return (0, Some(constraint));
            }
            let (mask, continuation) = node_mask(node, constraint);
            if *min == 0 {
                (mask, Some(constraint | continuation.unwrap_or(0)))
            } else {
                (mask, continuation)
            }
        }
        Ast::Group { child, .. } | Ast::Flags { child, .. } => node_mask(child, constraint),
        Ast::Backref(_) | Ast::Conditional { .. } | Ast::Subroutine(_) | Ast::Unsupported(_) => {
            (constraint, Some(constraint))
        }
    }
}

#[derive(Clone, Copy)]
struct CharSides {
    word: bool,
    nonword: bool,
    nullable: bool,
}

impl CharSides {
    const UNKNOWN: Self = Self {
        word: true,
        nonword: true,
        nullable: true,
    };

    const ZERO_WIDTH: Self = Self {
        word: false,
        nonword: false,
        nullable: true,
    };
}

/// Word-ness of the first character `ast` consumes.
fn first_char_sides(ast: &Ast) -> CharSides {
    char_sides(ast, false)
}

/// Word-ness of the last character `ast` consumes.
fn last_char_sides(ast: &Ast) -> CharSides {
    char_sides(ast, true)
}

fn char_sides(ast: &Ast, from_end: bool) -> CharSides {
    match ast {
        Ast::Empty => CharSides::ZERO_WIDTH,
        Ast::Literal(literal) => {
            let ch = if from_end {
                literal.chars().next_back()
            } else {
                literal.chars().next()
            };
            match ch {
                Some(ch) => {
                    let word = is_unicode_word_char(ch);
                    CharSides {
                        word,
                        nonword: !word,
                        nullable: false,
                    }
                }
                None => CharSides::ZERO_WIDTH,
            }
        }
        Ast::Class(class) => {
            let (word, nonword) = class_word_sides(class);
            CharSides {
                word,
                nonword,
                nullable: false,
            }
        }
        Ast::Dot | Ast::Grapheme => CharSides {
            word: true,
            nonword: true,
            nullable: false,
        },
        Ast::Anchor(_) | Ast::Look { .. } => CharSides::ZERO_WIDTH,
        Ast::Concat(nodes) => {
            let mut word = false;
            let mut nonword = false;
            let mut iterate = |node: &Ast| -> bool {
                let sides = char_sides(node, from_end);
                word |= sides.word;
                nonword |= sides.nonword;
                sides.nullable
            };
            let nullable = if from_end {
                nodes.iter().rev().all(&mut iterate)
            } else {
                nodes.iter().all(&mut iterate)
            };
            CharSides {
                word,
                nonword,
                nullable,
            }
        }
        Ast::Alternation(branches) => {
            let mut word = false;
            let mut nonword = false;
            let mut nullable = false;
            for branch in branches {
                let sides = char_sides(branch, from_end);
                word |= sides.word;
                nonword |= sides.nonword;
                nullable |= sides.nullable;
            }
            CharSides {
                word,
                nonword,
                nullable,
            }
        }
        Ast::Repeat { node, min, max, .. } => {
            if *max == Some(0) {
                return CharSides::ZERO_WIDTH;
            }
            let mut sides = char_sides(node, from_end);
            sides.nullable |= *min == 0;
            sides
        }
        Ast::Group { child, .. } | Ast::Flags { child, .. } => char_sides(child, from_end),
        Ast::Backref(_) | Ast::Conditional { .. } | Ast::Subroutine(_) | Ast::Unsupported(_) => {
            CharSides::UNKNOWN
        }
    }
}

/// True when a negative look with this child proves the guarded character is
/// non-word: a word character adjacent to the position must force the child
/// to match, so the guard failing implies "not a word char". The adjacent
/// element (first for lookahead, last for lookbehind) must consume one char
/// from a word-covering class, and everything on the far side of it must be
/// able to match empty unconditionally.
fn negated_look_excludes_word(child: &Ast, from_end: bool) -> bool {
    match child {
        Ast::Class(class) => class_covers_all_ascii_word(class),
        Ast::Group { child, .. } | Ast::Flags { child, .. } => {
            negated_look_excludes_word(child, from_end)
        }
        Ast::Concat(nodes) => {
            let (adjacent, rest) = if from_end {
                match nodes.split_last() {
                    Some((last, rest)) => (last, rest),
                    None => return false,
                }
            } else {
                match nodes.split_first() {
                    Some((first, rest)) => (first, rest),
                    None => return false,
                }
            };
            negated_look_excludes_word(adjacent, from_end)
                && rest.iter().all(matches_empty_unconditionally)
        }
        // Any single word-forced branch is enough: a word char makes that
        // branch (and therefore the alternation) match.
        Ast::Alternation(branches) => branches
            .iter()
            .any(|branch| negated_look_excludes_word(branch, from_end)),
        Ast::Repeat { node, min, max, .. } => {
            // One iteration must suffice and be permitted.
            *min <= 1
                && max.is_none_or(|max| max >= 1)
                && negated_look_excludes_word(node, from_end)
        }
        _ => false,
    }
}

/// True when the node can match the empty string at any position regardless
/// of surrounding context (anchors and lookarounds are conditional, so they
/// do not qualify).
fn matches_empty_unconditionally(ast: &Ast) -> bool {
    match ast {
        Ast::Empty => true,
        Ast::Literal(literal) => literal.is_empty(),
        Ast::Repeat { min, .. } => *min == 0,
        Ast::Group { child, .. } | Ast::Flags { child, .. } => matches_empty_unconditionally(child),
        Ast::Concat(nodes) => nodes.iter().all(matches_empty_unconditionally),
        Ast::Alternation(branches) => branches.iter().any(matches_empty_unconditionally),
        _ => false,
    }
}

/// True when the class is a superset of the ASCII word characters
/// (`[0-9A-Za-z_]`). Masks are never consulted at non-ASCII positions, so
/// ASCII coverage is the required bar.
fn class_covers_all_ascii_word(class: &CharClass) -> bool {
    if class.negated || !class.intersections.is_empty() {
        return false;
    }
    atoms_cover_all_ascii_word(&class.atoms)
}

fn atoms_cover_all_ascii_word(atoms: &[ClassAtom]) -> bool {
    let mut covered = [false; 128];
    for atom in atoms {
        match atom {
            ClassAtom::Perl(PerlClassKind::Word) => return true,
            ClassAtom::Char(ch) if ch.is_ascii() => covered[*ch as usize] = true,
            ClassAtom::Range(start, end) if start.is_ascii() && end.is_ascii() => {
                let (start, end) = (*start.min(end) as usize, *start.max(end) as usize);
                for slot in &mut covered[start..=end] {
                    *slot = true;
                }
            }
            ClassAtom::Posix {
                name,
                negated: false,
            } => match name.as_str() {
                "word" => return true,
                "alnum" => {
                    for ch in ('0'..='9').chain('A'..='Z').chain('a'..='z') {
                        covered[ch as usize] = true;
                    }
                }
                "alpha" => {
                    for ch in ('A'..='Z').chain('a'..='z') {
                        covered[ch as usize] = true;
                    }
                }
                "digit" => {
                    for ch in '0'..='9' {
                        covered[ch as usize] = true;
                    }
                }
                _ => {}
            },
            _ => {}
        }
    }
    ('0'..='9')
        .chain('A'..='Z')
        .chain('a'..='z')
        .chain(std::iter::once('_'))
        .all(|ch| covered[ch as usize])
}

/// Conservative (may-contain-word, may-contain-nonword) sides of a class.
fn class_word_sides(class: &CharClass) -> (bool, bool) {
    if class.atoms.is_empty() {
        return (true, true);
    }
    // `&&` intersections only narrow the first union, so its sides remain a
    // conservative superset.
    let mut word = false;
    let mut nonword = false;
    for atom in &class.atoms {
        let (atom_word, atom_nonword) = atom_word_sides(atom);
        word |= atom_word;
        nonword |= atom_nonword;
        if word && nonword {
            break;
        }
    }
    if class.negated {
        // The complement contains an ASCII word char unless the atoms cover
        // all of them; it contains a non-word char unless the atoms cover the
        // whole non-word side, which only `\W` asserts here.
        let covers_word =
            class.intersections.is_empty() && atoms_cover_all_ascii_word(&class.atoms);
        let covers_nonword = class.intersections.is_empty()
            && class
                .atoms
                .iter()
                .any(|atom| matches!(atom, ClassAtom::Perl(PerlClassKind::NotWord)));
        (!covers_word, !covers_nonword)
    } else {
        (word, nonword)
    }
}

fn atom_word_sides(atom: &ClassAtom) -> (bool, bool) {
    match atom {
        ClassAtom::Char(ch) => {
            let word = is_unicode_word_char(*ch);
            (word, !word)
        }
        ClassAtom::Range(start, end) => {
            if start.is_ascii() && end.is_ascii() {
                let (start, end) = (*start.min(end), *start.max(end));
                let mut word = false;
                let mut nonword = false;
                for ch in start..=end {
                    if is_unicode_word_char(ch) {
                        word = true;
                    } else {
                        nonword = true;
                    }
                    if word && nonword {
                        break;
                    }
                }
                (word, nonword)
            } else {
                (true, true)
            }
        }
        ClassAtom::Perl(kind) => match kind {
            PerlClassKind::Digit => (true, false),
            PerlClassKind::Word => (true, false),
            // Oniguruma `\h` is an ASCII hex digit — a word character.
            PerlClassKind::HorizontalSpace => (true, false),
            PerlClassKind::NotWord => (false, true),
            PerlClassKind::Space | PerlClassKind::VerticalSpace => (false, true),
            PerlClassKind::NotDigit
            | PerlClassKind::NotSpace
            | PerlClassKind::NotHorizontalSpace
            | PerlClassKind::NotVerticalSpace
            | PerlClassKind::NotNewline => (true, true),
        },
        ClassAtom::Posix { name, negated } => {
            if *negated {
                return (true, true);
            }
            match name.as_str() {
                "alpha" | "alnum" | "digit" | "xdigit" | "upper" | "lower" | "word" => {
                    (true, false)
                }
                // `[[:punct:]]` is ASCII punctuation, which includes `_` — a
                // word character.
                "space" | "blank" | "cntrl" => (false, true),
                _ => (true, true),
            }
        }
        ClassAtom::Unicode { .. } => (true, true),
        ClassAtom::Nested(class) => class_word_sides(class),
    }
}

#[cfg(test)]
mod tests {
    use super::super::ast::parse;
    use super::*;

    fn mask(pattern: &str) -> u8 {
        start_class_mask(&parse(pattern))
    }

    const MID_WORD: u8 = 0b1000;
    const WORD_START: u8 = 0b0010;
    const WORD_END: u8 = 0b0100;
    const GAP: u8 = 0b0001;

    #[test]
    fn keyword_with_lookbehind_only_starts_at_word_starts() {
        assert_eq!(mask(r"(?<!\w)this(?!\w)"), WORD_START);
        assert_eq!(mask(r"\bwhile\b"), WORD_START);
    }

    #[test]
    fn separator_prefixed_rules_exclude_mid_word() {
        let separator =
            r"((?:\s*+/\*(?:[^*]++|\*+(?!/))*+\*/\s*+)+|\s++|(?<=\W)|(?=\W)|^|\n?$|\A|\Z)";
        assert_eq!(mask(&format!("{separator}(#)\\s*pragma\\b")) & MID_WORD, 0);
        assert_eq!(
            mask(&format!("{separator}((?<!\\w)this(?!\\w))")) & MID_WORD,
            0
        );
    }

    #[test]
    fn identifier_patterns_allow_all_word_positions() {
        assert_eq!(mask(r"[A-Za-z_]\w*"), WORD_START | MID_WORD);
        assert_eq!(mask(r"\w+"), WORD_START | MID_WORD);
    }

    #[test]
    fn punctuation_and_anchor_patterns() {
        assert_eq!(mask(r"\{"), GAP | WORD_END);
        assert_eq!(mask(r"^\s*#"), GAP);
        assert_eq!(mask(r"$"), GAP | WORD_END);
        assert_eq!(mask(r"\G\w"), WORD_START | MID_WORD);
    }

    #[test]
    fn negated_lookbehind_with_extra_atoms_still_excludes_word_prev() {
        // TypeScript-style guard: prev not in [word ∪ $] implies prev non-word.
        assert_eq!(mask(r"(?<![\w$])if\b") & (MID_WORD | WORD_END), 0);
    }

    #[test]
    fn conservative_constructs_keep_all_classes() {
        // The backref itself is opaque, but the leading literal still bounds
        // the start class.
        assert_eq!(mask(r"(a)\1"), WORD_START | MID_WORD);
        assert_eq!(mask(r"\1x"), START_CLASS_ALL);
        assert_eq!(mask(r".*"), START_CLASS_ALL);
        assert_eq!(mask(r"x|.|^"), START_CLASS_ALL);
    }

    #[test]
    fn nullable_first_element_unions_with_following_element() {
        // `\s*` can match empty, so `#` decides: current char is non-word.
        assert_eq!(mask(r"\s*#") & (WORD_START | MID_WORD), 0);
        // But `\s+` consumes whitespace, so current char may be the space.
        assert_ne!(mask(r"\s+#") & CUR_NONWORD, 0);
    }

    #[test]
    fn empty_mask_falls_back_to_all() {
        // `\b\B` can never hold, and the analysis proves it; keep the
        // conservative all-classes mask instead of silencing the pattern.
        assert_eq!(mask(r"\b\B"), START_CLASS_ALL);
    }
}
