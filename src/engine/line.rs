#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct LineChunk<'a> {
    pub(crate) text: &'a str,
    pub(crate) parse_text: &'a str,
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
                    parse_text: "",
                });
            }
            return None;
        }

        let rest = &self.source[self.offset..];
        if let Some(newline) = rest.find('\n') {
            let end = self.offset + newline + 1;
            let parse_text = &self.source[self.offset..end];
            let text = &parse_text[..parse_text.len() - 1];
            self.offset = end;
            Some(LineChunk { text, parse_text })
        } else {
            self.offset = self.source.len();
            Some(LineChunk {
                text: rest,
                parse_text: rest,
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
        assert_eq!(chunks[0].parse_text, "a\n");
        assert_eq!(chunks[1].text, "");
    }
}
