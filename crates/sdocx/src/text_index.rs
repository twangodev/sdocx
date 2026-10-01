use std::ops::Range;

pub(crate) struct TextIndex<'a> {
    text: &'a str,
    byte_offsets: Vec<usize>,
    utf16_offsets: Vec<usize>,
    native_paragraph_starts: Vec<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Paragraph {
    pub physical: Range<usize>,
    pub content: Range<usize>,
}

impl<'a> TextIndex<'a> {
    pub fn new(text: &'a str) -> Self {
        let mut byte_offsets = Vec::new();
        let mut utf16_offsets = Vec::new();
        let mut native_paragraph_starts = vec![0];
        let mut utf16_offset = 0;
        for (character_index, (byte_offset, character)) in text.char_indices().enumerate() {
            byte_offsets.push(byte_offset);
            utf16_offsets.push(utf16_offset);
            utf16_offset += character.len_utf16();
            if matches!(character, '\r' | '\n') {
                native_paragraph_starts.push(character_index + 1);
            }
        }
        byte_offsets.push(text.len());
        utf16_offsets.push(utf16_offset);
        Self {
            text,
            byte_offsets,
            utf16_offsets,
            native_paragraph_starts,
        }
    }

    pub fn len(&self) -> usize {
        self.byte_offsets.len() - 1
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    #[cfg(any(feature = "render", test))]
    pub fn char_to_byte(&self, character_index: usize) -> Option<usize> {
        self.byte_offsets.get(character_index).copied()
    }

    #[cfg(any(feature = "render", test))]
    pub fn byte_to_char(&self, byte_index: usize) -> Option<usize> {
        self.byte_offsets.binary_search(&byte_index).ok()
    }

    pub fn char_to_utf16(&self, character_index: usize) -> Option<u32> {
        u32::try_from(*self.utf16_offsets.get(character_index)?).ok()
    }

    pub fn utf16_to_char(&self, utf16_index: u32) -> Option<usize> {
        self.utf16_offsets
            .binary_search(&usize::try_from(utf16_index).ok()?)
            .ok()
    }

    pub fn slice(&self, range: Range<usize>) -> Option<&'a str> {
        if range.start > range.end {
            return None;
        }
        self.text
            .get(*self.byte_offsets.get(range.start)?..*self.byte_offsets.get(range.end)?)
    }

    #[cfg(any(feature = "render", test))]
    pub fn display_paragraphs(&self) -> impl Iterator<Item = Paragraph> + '_ {
        self.native_paragraph_starts
            .iter()
            .enumerate()
            .filter(|_| !self.is_empty())
            .map(|(ordinal, &content_start)| {
                let end = self
                    .native_paragraph_starts
                    .get(ordinal + 1)
                    .map_or(self.len(), |next| next - 1);
                Paragraph {
                    physical: content_start.saturating_sub(usize::from(ordinal != 0))..end,
                    content: content_start..end,
                }
            })
    }

    pub fn paragraph_index(&self, character_index: usize) -> Option<u32> {
        self.byte_offsets.get(character_index)?;
        let ordinal = self
            .native_paragraph_starts
            .partition_point(|start| *start <= character_index)
            - 1;
        u32::try_from(ordinal).ok()
    }

    pub fn native_paragraphs(&self) -> impl Iterator<Item = Paragraph> + '_ {
        self.native_paragraph_starts
            .iter()
            .enumerate()
            .map(|(ordinal, &start)| {
                let next = self.native_paragraph_starts.get(ordinal + 1);
                Paragraph {
                    physical: start..next.copied().unwrap_or(self.len()),
                    content: start..next.map_or(self.len(), |end| end - 1),
                }
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scalar_and_utf8_boundaries_round_trip_without_accepting_multibyte_interiors() {
        let index = TextIndex::new("A😀e\u{301}中");
        for (character, byte) in [(0, 0), (1, 1), (2, 5), (3, 6), (4, 8), (5, 11)] {
            assert_eq!(index.char_to_byte(character), Some(byte));
            assert_eq!(index.byte_to_char(byte), Some(character));
        }
        for byte in [2, 3, 4, 7, 9, 10, 12, usize::MAX] {
            assert_eq!(index.byte_to_char(byte), None);
        }
        assert_eq!(index.char_to_byte(6), None);
        assert_eq!(index.char_to_byte(usize::MAX), None);
        let empty = TextIndex::new("");
        assert_eq!(empty.char_to_byte(0), Some(0));
        assert_eq!(empty.byte_to_char(0), Some(0));
        assert_eq!(empty.char_to_byte(1), None);
        assert_eq!(empty.byte_to_char(1), None);
    }

    #[test]
    fn unicode_boundaries_preserve_supplementary_and_combining_characters() {
        let index = TextIndex::new("A😀e\u{301}中");
        assert_eq!(index.len(), 5);
        assert!(!index.is_empty());
        for (character, utf16) in [(0, 0), (1, 1), (2, 3), (3, 4), (4, 5), (5, 6)] {
            assert_eq!(index.char_to_utf16(character), Some(utf16));
            assert_eq!(index.utf16_to_char(utf16), Some(character));
        }
        assert_eq!(index.slice(1..4), Some("😀e\u{301}"));
        assert_eq!(index.slice(4..5), Some("中"));
        assert_eq!(index.slice(5..5), Some(""));
    }

    #[test]
    fn invalid_ranges_and_surrogate_interiors_are_rejected() {
        let index = TextIndex::new("A😀B");
        assert_eq!(index.utf16_to_char(2), None);
        assert_eq!(index.utf16_to_char(5), None);
        assert_eq!(index.utf16_to_char(u32::MAX), None);
        assert_eq!(index.char_to_utf16(4), None);
        assert_eq!(index.slice(Range { start: 2, end: 1 }), None);
        assert_eq!(index.slice(0..4), None);
        assert_eq!(index.slice(4..4), None);
    }

    #[test]
    fn display_paragraphs_keep_leading_delimiters_and_unicode_offsets() {
        let index = TextIndex::new("A\r\n😀\n\r\nlast\r");
        let paragraphs = index.display_paragraphs().collect::<Vec<_>>();
        assert_eq!(
            paragraphs,
            [
                Paragraph {
                    physical: 0..1,
                    content: 0..1
                },
                Paragraph {
                    physical: 1..2,
                    content: 2..2
                },
                Paragraph {
                    physical: 2..4,
                    content: 3..4
                },
                Paragraph {
                    physical: 4..5,
                    content: 5..5
                },
                Paragraph {
                    physical: 5..6,
                    content: 6..6
                },
                Paragraph {
                    physical: 6..11,
                    content: 7..11
                },
                Paragraph {
                    physical: 11..12,
                    content: 12..12
                },
            ]
        );
        assert_eq!(index.slice(paragraphs[2].physical.clone()), Some("\n😀"));
        assert_eq!(index.slice(paragraphs[2].content.clone()), Some("😀"));
        assert_eq!(index.slice(paragraphs[4].physical.clone()), Some("\r"));
        assert_eq!(index.slice(paragraphs[4].content.clone()), Some(""));
        assert_eq!(index.char_to_utf16(paragraphs[4].physical.start), Some(6));
    }

    #[test]
    fn empty_text_and_terminal_newlines_have_defined_boundaries() {
        let empty = TextIndex::new("");
        assert!(empty.is_empty());
        assert_eq!(empty.slice(0..0), Some(""));
        assert_eq!(empty.char_to_utf16(0), Some(0));
        assert_eq!(empty.utf16_to_char(0), Some(0));
        assert_eq!(empty.display_paragraphs().count(), 0);
        assert_eq!(
            TextIndex::new("x\n")
                .display_paragraphs()
                .collect::<Vec<_>>(),
            [
                Paragraph {
                    physical: 0..1,
                    content: 0..1
                },
                Paragraph {
                    physical: 1..2,
                    content: 2..2
                }
            ]
        );
        assert_eq!(
            TextIndex::new("\n\n")
                .display_paragraphs()
                .collect::<Vec<_>>(),
            [
                Paragraph {
                    physical: 0..0,
                    content: 0..0
                },
                Paragraph {
                    physical: 0..1,
                    content: 1..1
                },
                Paragraph {
                    physical: 1..2,
                    content: 2..2
                },
            ]
        );
    }

    #[test]
    fn native_paragraph_ordinals_count_cr_and_lf_separately() {
        let index = TextIndex::new("😀\r\nb\n");
        assert_eq!(
            index.native_paragraphs().collect::<Vec<_>>(),
            [
                Paragraph {
                    physical: 0..2,
                    content: 0..1
                },
                Paragraph {
                    physical: 2..3,
                    content: 2..2
                },
                Paragraph {
                    physical: 3..5,
                    content: 3..4
                },
                Paragraph {
                    physical: 5..5,
                    content: 5..5
                },
            ]
        );
        for (character, ordinal) in [(0, 0), (1, 0), (2, 1), (3, 2), (4, 2), (5, 3)] {
            assert_eq!(index.paragraph_index(character), Some(ordinal));
        }
        assert_eq!(index.paragraph_index(6), None);
        assert_eq!(index.char_to_utf16(3), Some(4));
        let visual = index.display_paragraphs().nth(2).unwrap();
        assert_eq!(index.slice(visual.content.clone()), Some("b"));
        assert_eq!(index.paragraph_index(visual.content.start), Some(2));
    }

    #[test]
    fn native_empty_leading_and_trailing_paragraphs_are_retained() {
        let empty = TextIndex::new("");
        assert_eq!(empty.paragraph_index(0), Some(0));
        assert_eq!(
            empty.native_paragraphs().collect::<Vec<_>>(),
            [Paragraph {
                physical: 0..0,
                content: 0..0
            }]
        );
        assert_eq!(
            TextIndex::new("\r\n")
                .native_paragraphs()
                .collect::<Vec<_>>(),
            [
                Paragraph {
                    physical: 0..1,
                    content: 0..0
                },
                Paragraph {
                    physical: 1..2,
                    content: 1..1
                },
                Paragraph {
                    physical: 2..2,
                    content: 2..2
                },
            ]
        );
    }
}
