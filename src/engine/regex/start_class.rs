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
//! Each pattern gets two masks packed into one byte. The low nibble is
//! consulted when both neighboring bytes are ASCII, so its analysis only has
//! to be sound for ASCII word characters (`[0-9A-Za-z_]`). The high nibble
//! is consulted at positions with a non-ASCII neighbor, where the scan
//! classifies both neighbors with the Unicode `\w` predicate; its analysis
//! drops every claim that only holds for ASCII (for example that `[^A-Za-z]`
//! excludes word characters, or that `[[:alpha:]]` implies `\w`).

use super::ast::{
    AnchorKind, Ast, CharClass, ClassAtom, LookKind, ParsedRegex, PerlClassKind, RegexFlags,
};
use super::is_unicode_word_char;

/// Bit `1 << (prev_word * 2 + cur_word)` in the ASCII nibble, shifted by
/// [`UNICODE_SHIFT`] in the Unicode nibble. Line edges count as non-word.
pub(crate) const START_CLASS_ALL: u8 = 0xff;
const NIBBLE_ALL: u8 = 0b1111;
/// Offset of the Unicode-sound nibble within a packed mask.
pub(crate) const UNICODE_SHIFT: u8 = 4;

const PREV_WORD: u8 = 0b1100;
const PREV_NONWORD: u8 = 0b0011;
const CUR_WORD: u8 = 0b1010;
const CUR_NONWORD: u8 = 0b0101;
const BOUNDARY: u8 = 0b0110;
const NOT_BOUNDARY: u8 = 0b1001;

/// Which characters a mask must be sound for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    /// Both neighbors are ASCII.
    Ascii,
    /// Any neighbor may be any scalar. Tracks the effective case flag: a
    /// case-insensitive non-ASCII scalar may match partners whose word-ness
    /// the analysis cannot prove equal, so it stays conservative.
    Unicode { case_insensitive: bool },
}

impl Mode {
    fn with_flags(self, flags: RegexFlags) -> Self {
        match self {
            Self::Ascii => Self::Ascii,
            Self::Unicode { .. } => Self::Unicode {
                case_insensitive: flags.case_insensitive,
            },
        }
    }

    fn is_unicode(self) -> bool {
        matches!(self, Self::Unicode { .. })
    }
}

/// Packed start-class masks for a whole pattern. Each nibble is non-zero.
pub(crate) fn start_class_mask(parsed: &ParsedRegex) -> u8 {
    let unicode = Mode::Unicode {
        case_insensitive: parsed.flags.case_insensitive,
    };
    nibble_mask(parsed, Mode::Ascii) | (nibble_mask(parsed, unicode) << UNICODE_SHIFT)
}

/// (may-be-word, may-be-non-word) for a pattern scalar and every scalar it
/// matches. ASCII scalars' case partners share their word-ness (see the
/// exhaustive `ascii_case_partners_share_word_ness` test).
fn scalar_word_sides(ch: char, mode: Mode) -> (bool, bool) {
    if mode
        == (Mode::Unicode {
            case_insensitive: true,
        })
        && !ch.is_ascii()
    {
        return (true, true);
    }
    let word = is_unicode_word_char(ch);
    (word, !word)
}

fn nibble_mask(parsed: &ParsedRegex, mode: Mode) -> u8 {
    let (mask, continuation) = node_mask(&parsed.ast, NIBBLE_ALL, mode);
    let mask = mask | continuation.unwrap_or(0);
    if mask == 0 {
        // A provably unmatchable start set is more likely an analysis gap
        // than a real grammar pattern; never let it silence a candidate.
        NIBBLE_ALL
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
fn node_mask(ast: &Ast, constraint: u8, mode: Mode) -> (u8, Option<u8>) {
    match ast {
        Ast::Empty => (0, Some(constraint)),
        Ast::Literal(literal) => match literal.chars().next() {
            Some(ch) => {
                let (word, nonword) = scalar_word_sides(ch, mode);
                (constraint & sides_cur_bits(word, nonword), None)
            }
            None => (0, Some(constraint)),
        },
        Ast::Class(class) => {
            let (word, nonword) = class_word_sides(class, mode);
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
                    let sides = first_char_sides(child, mode);
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
                    if negated_look_excludes_word(child, false, mode) {
                        constraint & bits_cur(false)
                    } else {
                        constraint
                    }
                }
                LookKind::Behind => match behind_prev_bits(child, mode) {
                    Some(bits) => constraint & bits,
                    None => constraint,
                },
                LookKind::NotBehind => {
                    if negated_look_excludes_word(child, true, mode) {
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
                let (node_bits, continuation) = node_mask(node, carried, mode);
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
                let (branch_bits, branch_continuation) = node_mask(branch, constraint, mode);
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
            let (mask, continuation) = node_mask(node, constraint, mode);
            if *min == 0 {
                (mask, Some(constraint | continuation.unwrap_or(0)))
            } else {
                (mask, continuation)
            }
        }
        Ast::Group { child, .. } => node_mask(child, constraint, mode),
        Ast::Flags { flags, child } => node_mask(child, constraint, mode.with_flags(*flags)),
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

/// Previous-character bits a positive lookbehind with this child forces, or
/// `None` when it does not constrain the previous character. A `^` / `\A`
/// branch holds only at a line start or right after `\n`, so it forces a
/// non-word previous character just like a consuming non-word branch.
fn behind_prev_bits(child: &Ast, mode: Mode) -> Option<u8> {
    match child {
        Ast::Anchor(AnchorKind::LineStart | AnchorKind::TextStart) => Some(bits_prev(false)),
        Ast::Group { child, .. } => behind_prev_bits(child, mode),
        Ast::Flags { flags, child } => behind_prev_bits(child, mode.with_flags(*flags)),
        Ast::Alternation(branches) => branches.iter().try_fold(0, |bits, branch| {
            Some(bits | behind_prev_bits(branch, mode)?)
        }),
        _ => {
            let sides = last_char_sides(child, mode);
            // Line starts are folded into "previous is non-word", so the
            // impossible prev=None case stays conservative.
            (!sides.nullable).then(|| sides_prev_bits(sides.word, sides.nonword))
        }
    }
}

/// Word-ness of the first character `ast` consumes.
fn first_char_sides(ast: &Ast, mode: Mode) -> CharSides {
    char_sides(ast, false, mode)
}

/// Word-ness of the last character `ast` consumes.
fn last_char_sides(ast: &Ast, mode: Mode) -> CharSides {
    char_sides(ast, true, mode)
}

fn char_sides(ast: &Ast, from_end: bool, mode: Mode) -> CharSides {
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
                    let (word, nonword) = scalar_word_sides(ch, mode);
                    CharSides {
                        word,
                        nonword,
                        nullable: false,
                    }
                }
                None => CharSides::ZERO_WIDTH,
            }
        }
        Ast::Class(class) => {
            let (word, nonword) = class_word_sides(class, mode);
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
                let sides = char_sides(node, from_end, mode);
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
                let sides = char_sides(branch, from_end, mode);
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
            let mut sides = char_sides(node, from_end, mode);
            sides.nullable |= *min == 0;
            sides
        }
        Ast::Group { child, .. } => char_sides(child, from_end, mode),
        Ast::Flags { flags, child } => char_sides(child, from_end, mode.with_flags(*flags)),
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
fn negated_look_excludes_word(child: &Ast, from_end: bool, mode: Mode) -> bool {
    match child {
        Ast::Class(class) => class_covers_all_word(class, mode),
        Ast::Group { child, .. } => negated_look_excludes_word(child, from_end, mode),
        Ast::Flags { flags, child } => {
            negated_look_excludes_word(child, from_end, mode.with_flags(*flags))
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
            negated_look_excludes_word(adjacent, from_end, mode)
                && rest.iter().all(matches_empty_unconditionally)
        }
        // Any single word-forced branch is enough: a word char makes that
        // branch (and therefore the alternation) match.
        Ast::Alternation(branches) => branches
            .iter()
            .any(|branch| negated_look_excludes_word(branch, from_end, mode)),
        Ast::Repeat { node, min, max, .. } => {
            // One iteration must suffice and be permitted.
            *min <= 1
                && max.is_none_or(|max| max >= 1)
                && negated_look_excludes_word(node, from_end, mode)
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

/// True when the class is a superset of the word characters the mode can
/// observe: `[0-9A-Za-z_]` at ASCII positions, all of `\w` otherwise.
fn class_covers_all_word(class: &CharClass, mode: Mode) -> bool {
    if class.negated || !class.intersections.is_empty() {
        return false;
    }
    atoms_cover_all_word(&class.atoms, mode)
}

fn atoms_cover_all_word(atoms: &[ClassAtom], mode: Mode) -> bool {
    match mode {
        Mode::Ascii => atoms_cover_all_ascii_word(atoms),
        // `\w` and `[[:word:]]` evaluate the same Unicode predicate as the
        // scan's position classifier; nothing else is claimed.
        Mode::Unicode { .. } => atoms.iter().any(|atom| match atom {
            ClassAtom::Perl(PerlClassKind::Word) => true,
            ClassAtom::Posix {
                name,
                negated: false,
            } => name == "word",
            _ => false,
        }),
    }
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
fn class_word_sides(class: &CharClass, mode: Mode) -> (bool, bool) {
    if class.atoms.is_empty() {
        return (true, true);
    }
    // `&&` intersections only narrow the first union, so its sides remain a
    // conservative superset.
    let mut word = false;
    let mut nonword = false;
    for atom in &class.atoms {
        let (atom_word, atom_nonword) = atom_word_sides(atom, mode);
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
            class.intersections.is_empty() && atoms_cover_all_word(&class.atoms, mode);
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

fn atom_word_sides(atom: &ClassAtom, mode: Mode) -> (bool, bool) {
    match atom {
        ClassAtom::Char(ch) => scalar_word_sides(*ch, mode),
        ClassAtom::Range(start, end) if mode.is_unicode() => unicode_range_word_sides(*start, *end),
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
            if mode.is_unicode() {
                // Alphabetic, uppercase, and lowercase include non-`\w`
                // symbols such as circled letters; claim only exact sets.
                return match name.as_str() {
                    "digit" | "xdigit" | "word" => (true, false),
                    "space" | "blank" | "cntrl" => (false, true),
                    _ => (true, true),
                };
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
        ClassAtom::Nested(class) => class_word_sides(class, mode),
    }
}

/// Word-ness of every scalar a range can match with or without case
/// folding. A case-insensitive probe matches when its first lowercase or
/// uppercase mapping falls inside the folded bounds; with ASCII bounds that
/// mapping is an ASCII scalar sharing the probe's word-ness, so scanning the
/// literal and both folded intervals is complete. Non-ASCII bounds stay
/// conservative.
fn unicode_range_word_sides(start: char, end: char) -> (bool, bool) {
    let lower = |ch: char| ch.to_lowercase().next().unwrap_or(ch);
    let upper = |ch: char| ch.to_uppercase().next().unwrap_or(ch);
    let intervals = [
        (start, end),
        (lower(start), lower(end)),
        (upper(start), upper(end)),
    ];
    if intervals
        .iter()
        .any(|(low, high)| !low.is_ascii() || !high.is_ascii())
    {
        return (true, true);
    }
    let mut word = false;
    let mut nonword = false;
    for (low, high) in intervals {
        for ch in low.min(high)..=low.max(high) {
            if is_unicode_word_char(ch) {
                word = true;
            } else {
                nonword = true;
            }
        }
    }
    (word, nonword)
}

#[cfg(test)]
mod tests {
    use super::super::ast::parse;
    use super::*;

    fn mask(pattern: &str) -> u8 {
        start_class_mask(&parse(pattern)) & NIBBLE_ALL
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
        assert_eq!(mask(r"\1x"), NIBBLE_ALL);
        assert_eq!(mask(r".*"), NIBBLE_ALL);
        assert_eq!(mask(r"x|.|^"), NIBBLE_ALL);
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
        assert_eq!(mask(r"\b\B"), NIBBLE_ALL);
        assert_eq!(unicode_mask(r"\b\B"), NIBBLE_ALL);
    }

    fn unicode_mask(pattern: &str) -> u8 {
        start_class_mask(&parse(pattern)) >> UNICODE_SHIFT
    }

    #[test]
    fn line_start_lookbehind_branch_forces_nonword_previous() {
        let pattern = r"(?i:(?<=[^.а-яё\w]|^)(Если|If)(?=[^.а-яё\w]|$))";
        assert_eq!(mask(pattern), WORD_START);
        // Case-insensitive Cyrillic literals keep both current-character sides.
        assert_eq!(unicode_mask(pattern), GAP | WORD_START);
        // A nullable branch other than a line anchor still leaves the
        // previous character unconstrained.
        assert_eq!(mask(r"(?<=\W|x?)if") & MID_WORD, MID_WORD);
    }

    #[test]
    fn unicode_masks_drop_ascii_only_claims() {
        // `[^A-Za-z0-9_]` excludes every ASCII word char but contains `é`.
        assert_eq!(mask(r"(?<=[^A-Za-z0-9_])x") & (MID_WORD | WORD_END), 0);
        assert_eq!(unicode_mask(r"(?<=[^A-Za-z0-9_])x") & MID_WORD, MID_WORD);
        assert_eq!(unicode_mask(r"(?<![A-Za-z0-9_])x") & MID_WORD, MID_WORD);
        // `[[:alpha:]]` contains non-word symbols such as `Ⓐ`.
        assert_eq!(mask(r"[[:alpha:]]") & (GAP | WORD_END), 0);
        assert_ne!(unicode_mask(r"[[:alpha:]]") & (GAP | WORD_END), 0);
        // Exact Unicode predicates keep their precision.
        assert_eq!(unicode_mask(r"(?<!\w)this(?!\w)"), WORD_START);
        assert_eq!(unicode_mask(r"\bwhile\b"), WORD_START);
        assert_eq!(unicode_mask(r"(?i)[a-z]+") & (GAP | WORD_END), 0);
    }

    #[test]
    fn ascii_case_partners_share_word_ness() {
        // A case-insensitive ASCII literal scalar or ASCII-bounded range
        // matches exactly the scalars whose first lowercase or uppercase
        // mapping is ASCII, so those scalars must share its word-ness.
        // (Non-ASCII pairs such as U+A7D2/U+A7D3 do not always agree across
        // the Unicode tables in use; the analysis keeps them conservative.)
        for ch in (0..=0x10_ffff).filter_map(char::from_u32) {
            let word = is_unicode_word_char(ch);
            for head in [ch.to_lowercase().next(), ch.to_uppercase().next()]
                .into_iter()
                .flatten()
                .filter(char::is_ascii)
            {
                assert_eq!(is_unicode_word_char(head), word, "{ch:?} -> {head:?}");
            }
        }
    }

    #[test]
    fn case_insensitive_non_ascii_scalars_stay_conservative() {
        assert_eq!(unicode_mask(r"(?i)ꟓ"), NIBBLE_ALL);
        assert_eq!(unicode_mask(r"(?<=(?i:ꟓ))x"), WORD_START | MID_WORD);
        assert_eq!(unicode_mask(r"(?<=ꟓ)x"), MID_WORD);
        // ASCII case partners stay exact.
        assert_eq!(unicode_mask(r"(?i)(?<!k)x") & MID_WORD, MID_WORD);
        assert_eq!(unicode_mask(r"(?i)k"), WORD_START | MID_WORD);
    }
}
