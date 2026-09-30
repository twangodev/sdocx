use unicode_linebreak::{BreakOpportunity, linebreaks};
use unicode_segmentation::UnicodeSegmentation;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::render) enum BreakKind {
    Allowed,
    Mandatory,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::render) struct BreakCandidate {
    /// Scalar offset immediately after the source text retained on the line.
    pub end: usize,
    pub kind: BreakKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::render) struct ParagraphBreaks {
    pub candidates: Vec<BreakCandidate>,
    /// Grapheme ends for overflow fallback, independent of normal line-breaking rules.
    pub emergency: Vec<usize>,
}

pub(in crate::render) fn break_candidates(text: &str) -> ParagraphBreaks {
    let scalar_boundaries = text
        .char_indices()
        .map(|(byte, _)| byte)
        .chain(std::iter::once(text.len()))
        .collect::<Vec<_>>();
    let emergency = text
        .grapheme_indices(true)
        .filter_map(|(byte, grapheme)| {
            let end = byte.checked_add(grapheme.len())?;
            scalar_boundaries.binary_search(&end).ok()
        })
        .collect::<Vec<_>>();
    let candidates = linebreaks(text)
        .filter_map(|(byte, opportunity)| {
            let end = scalar_boundaries.binary_search(&byte).ok()?;
            emergency.binary_search(&end).ok()?;
            Some(BreakCandidate {
                end,
                kind: match opportunity {
                    BreakOpportunity::Allowed => BreakKind::Allowed,
                    BreakOpportunity::Mandatory => BreakKind::Mandatory,
                },
            })
        })
        .collect();
    ParagraphBreaks {
        candidates,
        emergency,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use BreakKind::{Allowed, Mandatory};

    fn candidates(text: &str) -> Vec<(usize, BreakKind)> {
        break_candidates(text)
            .candidates
            .into_iter()
            .map(|candidate| (candidate.end, candidate.kind))
            .collect()
    }

    #[test]
    fn cjk_breaks_use_scalar_offsets_instead_of_utf8_bytes() {
        assert_eq!(
            candidates("中文测试"),
            [(1, Allowed), (2, Allowed), (3, Allowed), (4, Mandatory)]
        );
        assert_eq!(break_candidates("中文测试").emergency, [1, 2, 3, 4]);
    }

    #[test]
    fn nonbreaking_space_and_word_joiner_suppress_normal_breaks() {
        for text in ["ab\u{a0}cd ef", "ab\u{2060}cd ef"] {
            assert_eq!(candidates(text), [(6, Allowed), (8, Mandatory)]);
            assert!(break_candidates(text).emergency.contains(&3));
        }
    }

    #[test]
    fn combining_marks_and_emoji_zwj_sequences_are_indivisible() {
        assert_eq!(break_candidates("e\u{301}x").emergency, [2, 3]);
        assert_eq!(candidates("e\u{301}x"), [(3, Mandatory)]);
        assert_eq!(break_candidates("A👩‍👩‍👧‍👦B").emergency, [1, 8, 9]);
        assert_eq!(
            candidates("A👩‍👩‍👧‍👦B"),
            [(1, Allowed), (8, Allowed), (9, Mandatory)]
        );
    }

    #[test]
    fn crlf_is_one_mandatory_break_and_one_grapheme() {
        assert_eq!(candidates("a\r\nb"), [(3, Mandatory), (4, Mandatory)]);
        assert_eq!(break_candidates("a\r\nb").emergency, [1, 3, 4]);
    }

    #[test]
    fn leading_repeated_and_trailing_spaces_remain_in_source_ranges() {
        let source = " a  b   ";
        let breaks = break_candidates(source);
        assert_eq!(
            candidates(source),
            [(1, Allowed), (4, Allowed), (8, Mandatory)]
        );
        let mut start = 0;
        let chunks = breaks
            .candidates
            .iter()
            .map(|candidate| {
                let chunk = source
                    .chars()
                    .skip(start)
                    .take(candidate.end - start)
                    .collect::<String>();
                start = candidate.end;
                chunk
            })
            .collect::<Vec<_>>();
        assert_eq!(chunks, [" ", "a  ", "b   "]);
        assert_eq!(chunks.concat(), source);
    }

    #[test]
    fn empty_text_has_no_breaks_but_spaces_have_a_terminal_break() {
        let empty = break_candidates("");
        assert!(empty.candidates.is_empty());
        assert!(empty.emergency.is_empty());
        assert_eq!(candidates("   "), [(3, Mandatory)]);
        assert_eq!(break_candidates("   ").emergency, [1, 2, 3]);
    }
}
