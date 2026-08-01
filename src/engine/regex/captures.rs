use std::collections::{HashMap, VecDeque};
use std::ops::Range;

pub fn whole_match(start: usize, end: usize) -> Vec<Option<Range<usize>>> {
    vec![Some(start..end)]
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SubstitutionError {
    PatternTooLong { len: usize, max: usize },
}

#[derive(Debug, Clone)]
pub struct EndPatternCache {
    max_entries: usize,
    max_pattern_len: usize,
    order: VecDeque<String>,
    entries: HashMap<String, String>,
}

impl EndPatternCache {
    pub fn new(max_entries: usize, max_pattern_len: usize) -> Self {
        Self {
            max_entries,
            max_pattern_len,
            order: VecDeque::new(),
            entries: HashMap::new(),
        }
    }

    pub fn substitute(
        &mut self,
        pattern: &str,
        captures: &[Option<&str>],
    ) -> Result<String, SubstitutionError> {
        let key = cache_key(pattern, captures);
        if let Some(value) = self.entries.get(&key) {
            return Ok(value.clone());
        }
        let substituted = substitute_end_pattern(pattern, captures, self.max_pattern_len)?;
        if self.max_entries > 0 {
            if self.entries.len() >= self.max_entries
                && let Some(oldest) = self.order.pop_front()
            {
                self.entries.remove(&oldest);
            }
            self.order.push_back(key.clone());
            self.entries.insert(key, substituted.clone());
        }
        Ok(substituted)
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

pub fn substitute_end_pattern(
    pattern: &str,
    captures: &[Option<&str>],
    max_pattern_len: usize,
) -> Result<String, SubstitutionError> {
    let mut output = String::with_capacity(pattern.len());
    let mut chars = pattern.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch != '\\' {
            output.push(ch);
        } else if let Some(next @ '1'..='9') = chars.peek().copied() {
            let mut digits = String::new();
            digits.push(next);
            chars.next();
            while let Some(digit @ '0'..='9') = chars.peek().copied() {
                digits.push(digit);
                chars.next();
            }
            let index = digits.parse::<usize>().unwrap_or(0);
            if let Some(Some(text)) = captures.get(index) {
                output.push_str(&escape_regex_literal(text));
            }
        } else {
            output.push('\\');
            if let Some(next) = chars.next() {
                output.push(next);
            }
        }
        if output.len() > max_pattern_len {
            return Err(SubstitutionError::PatternTooLong {
                len: output.len(),
                max: max_pattern_len,
            });
        }
    }
    Ok(output)
}

pub fn capture_texts<'a>(line: &'a str, captures: &[Option<Range<usize>>]) -> Vec<Option<&'a str>> {
    captures
        .iter()
        .map(|range| range.as_ref().and_then(|range| line.get(range.clone())))
        .collect()
}

fn escape_regex_literal(text: &str) -> String {
    let mut output = String::new();
    for ch in text.chars() {
        if matches!(
            ch,
            '\\' | '.' | '+' | '*' | '?' | '(' | ')' | '|' | '[' | ']' | '{' | '}' | '^' | '$'
        ) {
            output.push('\\');
        }
        output.push(ch);
    }
    output
}

fn cache_key(pattern: &str, captures: &[Option<&str>]) -> String {
    let mut key = String::from(pattern);
    for capture in captures {
        key.push('\u{0}');
        if let Some(capture) = capture {
            key.push_str(capture);
        }
    }
    key
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn substitutes_numbered_end_captures() {
        let captures = vec![Some("<<"), Some("TAG")];
        let end = substitute_end_pattern(r"^\1$", &captures, 1024).unwrap();
        assert_eq!(end, r"^TAG$");
    }

    #[test]
    fn escapes_capture_text() {
        let captures = vec![Some("all"), Some("a+b")];
        let end = substitute_end_pattern(r"\1", &captures, 1024).unwrap();
        assert_eq!(end, r"a\+b");
    }

    #[test]
    fn unmatched_begin_capture_substitutes_as_empty() {
        let captures = vec![Some("all"), Some("tag"), None];
        let end = substitute_end_pattern(r"\1\2", &captures, 1024).unwrap();
        assert_eq!(end, "tag");
    }

    #[test]
    fn cache_reuses_and_caps_entries() {
        let mut cache = EndPatternCache::new(1, 1024);
        let captures = vec![Some("all"), Some("TAG")];
        assert_eq!(cache.substitute(r"\1", &captures).unwrap(), "TAG");
        assert_eq!(cache.substitute(r"\1", &captures).unwrap(), "TAG");
        assert_eq!(cache.len(), 1);
    }
}
