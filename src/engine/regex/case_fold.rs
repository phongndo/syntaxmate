//! Case-insensitive membership for class ranges.
//!
//! Oniguruma adds every case variant of a class's members, so under `(?i)` a
//! scalar matches `[a-{]` when it or one of its variants lies in the literal
//! range: `A` matches, `\` does not. Variants are the scalars that
//! `unicode_case_eq` equates with the probe.

/// Case variants not reachable through single-scalar lower- and uppercase
/// mappings (the Kelvin sign for `k`, final sigma for `σ`), sorted by probe.
/// `partner_table_is_complete` regenerates the expectation from the standard
/// library's Unicode tables.
const CASE_PARTNERS: &[(char, char)] = &[
    ('\u{4B}', '\u{212A}'),
    ('\u{53}', '\u{17F}'),
    ('\u{6B}', '\u{212A}'),
    ('\u{73}', '\u{17F}'),
    ('\u{C5}', '\u{212B}'),
    ('\u{DF}', '\u{1E9E}'),
    ('\u{E5}', '\u{212B}'),
    ('\u{1C4}', '\u{1C5}'),
    ('\u{1C6}', '\u{1C5}'),
    ('\u{1C7}', '\u{1C8}'),
    ('\u{1C9}', '\u{1C8}'),
    ('\u{1CA}', '\u{1CB}'),
    ('\u{1CC}', '\u{1CB}'),
    ('\u{1F1}', '\u{1F2}'),
    ('\u{1F3}', '\u{1F2}'),
    ('\u{345}', '\u{1FBE}'),
    ('\u{390}', '\u{1FD3}'),
    ('\u{392}', '\u{3D0}'),
    ('\u{395}', '\u{3F5}'),
    ('\u{398}', '\u{3D1}'),
    ('\u{398}', '\u{3F4}'),
    ('\u{399}', '\u{345}'),
    ('\u{399}', '\u{1FBE}'),
    ('\u{39A}', '\u{3F0}'),
    ('\u{39C}', '\u{B5}'),
    ('\u{3A0}', '\u{3D6}'),
    ('\u{3A1}', '\u{3F1}'),
    ('\u{3A3}', '\u{3C2}'),
    ('\u{3A6}', '\u{3D5}'),
    ('\u{3A9}', '\u{2126}'),
    ('\u{3B0}', '\u{1FE3}'),
    ('\u{3B2}', '\u{3D0}'),
    ('\u{3B5}', '\u{3F5}'),
    ('\u{3B8}', '\u{3D1}'),
    ('\u{3B8}', '\u{3F4}'),
    ('\u{3B9}', '\u{345}'),
    ('\u{3B9}', '\u{1FBE}'),
    ('\u{3BA}', '\u{3F0}'),
    ('\u{3BC}', '\u{B5}'),
    ('\u{3C0}', '\u{3D6}'),
    ('\u{3C1}', '\u{3F1}'),
    ('\u{3C3}', '\u{3C2}'),
    ('\u{3C6}', '\u{3D5}'),
    ('\u{3C9}', '\u{2126}'),
    ('\u{412}', '\u{1C80}'),
    ('\u{414}', '\u{1C81}'),
    ('\u{41E}', '\u{1C82}'),
    ('\u{421}', '\u{1C83}'),
    ('\u{422}', '\u{1C84}'),
    ('\u{422}', '\u{1C85}'),
    ('\u{42A}', '\u{1C86}'),
    ('\u{432}', '\u{1C80}'),
    ('\u{434}', '\u{1C81}'),
    ('\u{43E}', '\u{1C82}'),
    ('\u{441}', '\u{1C83}'),
    ('\u{442}', '\u{1C84}'),
    ('\u{442}', '\u{1C85}'),
    ('\u{44A}', '\u{1C86}'),
    ('\u{462}', '\u{1C87}'),
    ('\u{463}', '\u{1C87}'),
    ('\u{1C84}', '\u{1C85}'),
    ('\u{1C85}', '\u{1C84}'),
    ('\u{1E60}', '\u{1E9B}'),
    ('\u{1E61}', '\u{1E9B}'),
    ('\u{1F80}', '\u{1F88}'),
    ('\u{1F81}', '\u{1F89}'),
    ('\u{1F82}', '\u{1F8A}'),
    ('\u{1F83}', '\u{1F8B}'),
    ('\u{1F84}', '\u{1F8C}'),
    ('\u{1F85}', '\u{1F8D}'),
    ('\u{1F86}', '\u{1F8E}'),
    ('\u{1F87}', '\u{1F8F}'),
    ('\u{1F90}', '\u{1F98}'),
    ('\u{1F91}', '\u{1F99}'),
    ('\u{1F92}', '\u{1F9A}'),
    ('\u{1F93}', '\u{1F9B}'),
    ('\u{1F94}', '\u{1F9C}'),
    ('\u{1F95}', '\u{1F9D}'),
    ('\u{1F96}', '\u{1F9E}'),
    ('\u{1F97}', '\u{1F9F}'),
    ('\u{1FA0}', '\u{1FA8}'),
    ('\u{1FA1}', '\u{1FA9}'),
    ('\u{1FA2}', '\u{1FAA}'),
    ('\u{1FA3}', '\u{1FAB}'),
    ('\u{1FA4}', '\u{1FAC}'),
    ('\u{1FA5}', '\u{1FAD}'),
    ('\u{1FA6}', '\u{1FAE}'),
    ('\u{1FA7}', '\u{1FAF}'),
    ('\u{1FB3}', '\u{1FBC}'),
    ('\u{1FBE}', '\u{345}'),
    ('\u{1FC3}', '\u{1FCC}'),
    ('\u{1FD3}', '\u{390}'),
    ('\u{1FE3}', '\u{3B0}'),
    ('\u{1FF3}', '\u{1FFC}'),
    ('\u{A64A}', '\u{1C88}'),
    ('\u{A64B}', '\u{1C88}'),
    ('\u{FB05}', '\u{FB06}'),
    ('\u{FB06}', '\u{FB05}'),
];

/// Scalars that `unicode_case_eq` equates with `ch`, including `ch` itself.
/// May repeat a scalar.
pub(crate) fn case_variants(ch: char) -> impl Iterator<Item = char> {
    let mut variants = ['\0'; 5];
    let mut len = 1;
    variants[0] = ch;
    // Oniguruma's default fold keeps dotless i out of the I/i class.
    if ch != '\u{131}' {
        let mut push = |variant: Option<char>| {
            if let Some(variant) = variant {
                variants[len] = variant;
                len += 1;
            }
        };
        let lower = single(ch.to_lowercase());
        let upper = single(ch.to_uppercase());
        push(lower);
        push(upper);
        push(lower.and_then(|lower| single(lower.to_uppercase())));
        push(upper.and_then(|upper| single(upper.to_lowercase())));
    }
    let partners = CASE_PARTNERS.partition_point(|(probe, _)| *probe < ch);
    let partners = CASE_PARTNERS[partners..]
        .iter()
        .take_while(move |(probe, _)| *probe == ch)
        .map(|(_, partner)| *partner);
    variants.into_iter().take(len).chain(partners)
}

/// Whether `ch` matches the class range `start..=end` case-insensitively.
#[inline]
pub(crate) fn range_contains_ignore_case(start: char, end: char, ch: char) -> bool {
    let within = |candidate: char| start <= candidate && candidate <= end;
    if within(ch) {
        return true;
    }
    if ch.is_ascii() {
        // ASCII variants are the other case; the remaining ones (Kelvin
        // sign, long s) are non-ASCII and need a range reaching past ASCII.
        return (ch.is_ascii_alphabetic() && within((ch as u8 ^ 0x20) as char))
            || (!end.is_ascii() && case_variants(ch).any(within));
    }
    case_variants(ch).any(within)
}

fn single(mut mapped: impl Iterator<Item = char>) -> Option<char> {
    let first = mapped.next()?;
    mapped.next().is_none().then_some(first)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::regex::backtrack::unicode_case_eq;
    use std::collections::HashMap;

    #[test]
    fn partner_table_is_complete() {
        // Scalars can only be case-equal when they share a full lowercase or
        // uppercase mapping, so grouping by both mappings finds every pair.
        let mut groups: HashMap<(bool, String), Vec<char>> = HashMap::new();
        for ch in (0..=0x10_ffff).filter_map(char::from_u32) {
            groups
                .entry((false, ch.to_lowercase().collect()))
                .or_default()
                .push(ch);
            groups
                .entry((true, ch.to_uppercase().collect()))
                .or_default()
                .push(ch);
        }
        for group in groups.values().filter(|group| group.len() > 1) {
            for &probe in group {
                let variants = case_variants(probe).collect::<Vec<_>>();
                for &other in group {
                    assert_eq!(
                        unicode_case_eq(probe, other),
                        variants.contains(&other),
                        "{probe:?} and {other:?}"
                    );
                }
            }
        }
        for ch in (0..=0x10_ffff).filter_map(char::from_u32) {
            for variant in case_variants(ch) {
                assert!(unicode_case_eq(ch, variant), "{ch:?} and {variant:?}");
            }
        }
    }

    #[test]
    fn ranges_match_case_variants_of_their_members() {
        // Expectations checked against vscode-oniguruma.
        for (start, end, ch, expected) in [
            ('a', '{', '\\', false),
            ('a', '{', '[', false),
            ('a', '{', '`', false),
            ('a', '{', 'A', true),
            ('Z', 'a', '_', true),
            ('Z', 'a', 'z', true),
            ('Z', 'a', 'A', true),
            ('Z', 'a', 'b', false),
            ('@', 'C', 'c', true),
            ('@', 'C', 'd', false),
            ('a', 'z', '\u{212a}', true),
            ('\u{2100}', '\u{2200}', 'k', true),
            ('\u{3c3}', '\u{3c3}', '\u{3c2}', true),
            ('h', 'j', '\u{131}', false),
        ] {
            assert_eq!(
                range_contains_ignore_case(start, end, ch),
                expected,
                "[{start:?}-{end:?}] on {ch:?}"
            );
        }
    }
}
