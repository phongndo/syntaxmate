/// One logical line of a document.
///
/// Lines end at `\n` or the end of input; one `\r` right before that end
/// belongs to the terminator, so CRLF input tokenizes exactly like LF input.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct LineChunk<'a> {
    /// Line content without its terminator.
    pub(crate) text: &'a str,
    /// Content plus its `\n` for LF lines; use [`Self::parse_text`] instead.
    raw_parse_text: &'a str,
    /// The original terminator: `"\n"`, `"\r\n"`, or for the last line `"\r"` or empty.
    pub(crate) terminator: &'a str,
}

impl<'a> LineChunk<'a> {
    /// The text the tokenizer parses: the content plus `\n`, as TextMate
    /// parses every line. LF lines borrow the source; CRLF and unterminated
    /// final lines are copied into `buffer` with a synthetic LF.
    pub(crate) fn parse_text<'b>(&self, buffer: &'b mut String) -> &'b str
    where
        'a: 'b,
    {
        if self.terminator == "\n" {
            self.raw_parse_text
        } else {
            buffer.clear();
            buffer.push_str(self.text);
            buffer.push('\n');
            buffer
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FinalEmptyLine {
    Pending,
    Complete,
}

pub(crate) struct LineChunks<'a> {
    source: &'a str,
    offset: usize,
    final_empty_line: FinalEmptyLine,
}

impl<'a> LineChunks<'a> {
    pub(crate) fn new(source: &'a str) -> Self {
        Self {
            source,
            offset: 0,
            final_empty_line: if source.is_empty() || source.ends_with('\n') {
                FinalEmptyLine::Pending
            } else {
                FinalEmptyLine::Complete
            },
        }
    }
}

impl<'a> Iterator for LineChunks<'a> {
    type Item = LineChunk<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.offset >= self.source.len() {
            if self.final_empty_line == FinalEmptyLine::Pending {
                self.final_empty_line = FinalEmptyLine::Complete;
                return Some(LineChunk {
                    text: "",
                    raw_parse_text: "",
                    terminator: "",
                });
            }
            return None;
        }

        let rest = &self.source[self.offset..];
        if let Some(newline) = rest.find('\n') {
            let end = self.offset + newline + 1;
            let raw_parse_text = &self.source[self.offset..end];
            let line = &raw_parse_text[..raw_parse_text.len() - 1];
            let (text, terminator) = match line.strip_suffix('\r') {
                Some(text) => (text, &raw_parse_text[text.len()..]),
                None => (line, &raw_parse_text[line.len()..]),
            };
            self.offset = end;
            Some(LineChunk {
                text,
                raw_parse_text,
                terminator,
            })
        } else {
            self.offset = self.source.len();
            let text = rest.strip_suffix('\r').unwrap_or(rest);
            Some(LineChunk {
                text,
                raw_parse_text: rest,
                terminator: &rest[text.len()..],
            })
        }
    }
}

pub(crate) fn next_char_boundary(text: &str, offset: usize) -> usize {
    if offset >= text.len() {
        return text.len();
    }
    text[offset..]
        .chars()
        .next()
        .map(|ch| offset + ch.len_utf8())
        .unwrap_or(text.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preserves_final_empty_line_behavior() {
        let chunks = LineChunks::new("a\n").collect::<Vec<_>>();
        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0].text, "a");
        assert_eq!(chunks[0].parse_text(&mut String::new()), "a\n");
        assert_eq!(chunks[1].text, "");
    }

    #[test]
    fn crlf_terminators_parse_like_lf() {
        let chunks = LineChunks::new("a\r\nb\n\rc\r").collect::<Vec<_>>();
        let mut buffer = String::new();
        assert_eq!(chunks.len(), 3);
        assert_eq!((chunks[0].text, chunks[0].terminator), ("a", "\r\n"));
        assert_eq!(chunks[0].parse_text(&mut buffer), "a\n");
        assert_eq!((chunks[1].text, chunks[1].terminator), ("b", "\n"));
        assert_eq!(chunks[1].parse_text(&mut buffer), "b\n");
        // A `\r` inside a line is content; one at the end is a terminator.
        assert_eq!((chunks[2].text, chunks[2].terminator), ("\rc", "\r"));
        assert_eq!(chunks[2].parse_text(&mut buffer), "\rc\n");
    }
}
