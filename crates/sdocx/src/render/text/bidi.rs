use std::ops::Range;

use unicode_bidi::{BidiClass, BidiInfo, Level};

use crate::fonts::Direction;

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub(in crate::render) enum BidiError {
    #[error("bidi source range is outside its paragraph or not in logical order")]
    InvalidRange,
    #[error("native visual ordering across an internal paragraph separator is unverified")]
    UnsupportedParagraphSeparator,
    #[error("a shaped cluster crosses directional embedding levels")]
    MixedEmbeddingLevels,
    #[error("a shaped cluster is not contiguous in the paragraph visual map")]
    NonContiguousCluster,
}

pub(in crate::render) struct ParagraphBidi<'text> {
    analysis: BidiInfo<'text>,
    source: Range<usize>,
    levels: Vec<Level>,
    visual_ranks: Option<Vec<usize>>,
}

impl<'text> ParagraphBidi<'text> {
    pub fn new(text: &'text str, source_start: usize) -> Result<Self, BidiError> {
        let analysis = BidiInfo::new(text, None);
        let source_end = source_start
            .checked_add(text.chars().count())
            .ok_or(BidiError::InvalidRange)?;
        let internal_separator = analysis.original_classes.contains(&BidiClass::B);
        let levels = if internal_separator {
            text.char_indices()
                .map(|(byte, _)| analysis.levels[byte])
                .collect()
        } else if let Some(paragraph) = analysis.paragraphs.first() {
            analysis.reordered_levels_per_char(paragraph, paragraph.range.clone())
        } else {
            Vec::new()
        };
        let mut bidi = Self {
            analysis,
            source: source_start..source_end,
            levels,
            visual_ranks: None,
        };
        if !internal_separator {
            bidi.restore_control_levels(text);
            let visual_order = BidiInfo::reorder_visual(bidi.levels());
            let mut ranks = vec![0; bidi.levels.len()];
            for (rank, source) in visual_order.into_iter().enumerate() {
                ranks[source] = rank;
            }
            bidi.visual_ranks = Some(ranks);
        }
        Ok(bidi)
    }

    pub fn base_direction(&self) -> Direction {
        direction(
            self.analysis
                .paragraphs
                .first()
                .map_or(Level::ltr(), |paragraph| paragraph.level),
        )
    }

    pub fn direction_at(&self, scalar: usize) -> Option<Direction> {
        self.levels().get(scalar).copied().map(direction)
    }

    pub fn levels(&self) -> &[Level] {
        &self.levels
    }

    fn restore_control_levels(&mut self, text: &str) {
        let mut following = match self.base_direction() {
            Direction::RightToLeft => Level::rtl(),
            _ => Level::ltr(),
        };
        for ((byte, _), level) in text.char_indices().rev().zip(self.levels.iter_mut().rev()) {
            if matches!(
                self.analysis.original_classes[byte],
                BidiClass::BN
                    | BidiClass::LRE
                    | BidiClass::RLE
                    | BidiClass::LRO
                    | BidiClass::RLO
                    | BidiClass::PDF
            ) {
                *level = following;
            }
            following = *level;
        }
    }

    pub fn visual_order(
        &self,
        line: Range<usize>,
        logical_items: &[Range<usize>],
    ) -> Result<Vec<usize>, BidiError> {
        let line = self.relative_range(line)?;
        let ranks = self
            .visual_ranks
            .as_ref()
            .ok_or(BidiError::UnsupportedParagraphSeparator)?;
        let mut ranked = Vec::with_capacity(logical_items.len());
        let mut previous_end = line.start;
        for (index, source) in logical_items.iter().enumerate() {
            let source = self.relative_range(source.clone())?;
            if source.is_empty()
                || source.start < previous_end
                || source.start < line.start
                || source.end > line.end
            {
                return Err(BidiError::InvalidRange);
            }
            let level = self.levels[source.start];
            if self.levels[source.clone()]
                .iter()
                .any(|&other| other != level)
            {
                return Err(BidiError::MixedEmbeddingLevels);
            }
            let item_ranks = &ranks[source.clone()];
            let first = *item_ranks.iter().min().ok_or(BidiError::InvalidRange)?;
            let last = *item_ranks.iter().max().ok_or(BidiError::InvalidRange)?;
            if last - first + 1 != source.len() {
                return Err(BidiError::NonContiguousCluster);
            }
            ranked.push((first, index));
            previous_end = source.end;
        }
        ranked.sort_unstable_by_key(|&(rank, _)| rank);
        Ok(ranked.into_iter().map(|(_, index)| index).collect())
    }

    fn relative_range(&self, source: Range<usize>) -> Result<Range<usize>, BidiError> {
        if source.start > source.end
            || source.start < self.source.start
            || source.end > self.source.end
        {
            return Err(BidiError::InvalidRange);
        }
        Ok(source.start - self.source.start..source.end - self.source.start)
    }
}

fn direction(level: Level) -> Direction {
    if level.is_rtl() {
        Direction::RightToLeft
    } else {
        Direction::LeftToRight
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scalar_items(source: Range<usize>) -> Vec<Range<usize>> {
        source.map(|index| index..index + 1).collect()
    }

    #[test]
    fn soft_wrapping_filters_paragraph_ranks_without_resetting_trailing_spaces() {
        let bidi = ParagraphBidi::new("Aאב  גZ", 0).unwrap();
        assert_eq!(bidi.base_direction(), Direction::LeftToRight);
        assert_eq!(
            bidi.visual_order(0..5, &scalar_items(0..5)).unwrap(),
            [0, 4, 3, 2, 1]
        );
        let fresh_line_levels = bidi
            .analysis
            .reordered_levels_per_char(&bidi.analysis.paragraphs[0], 0..7);
        assert_eq!(
            BidiInfo::reorder_visual(&fresh_line_levels[..5]),
            [0, 2, 1, 3, 4]
        );
        assert_eq!(bidi.direction_at(3), Some(Direction::RightToLeft));
    }

    #[test]
    fn nested_digit_levels_keep_native_paragraph_visual_ranks() {
        let bidi = ParagraphBidi::new("Aאב12גZ", 10).unwrap();
        assert_eq!(
            bidi.levels.iter().map(Level::number).collect::<Vec<_>>(),
            [0, 1, 1, 2, 2, 1, 0]
        );
        assert_eq!(
            bidi.visual_order(10..17, &scalar_items(10..17)).unwrap(),
            [0, 5, 3, 4, 2, 1, 6]
        );
    }

    #[test]
    fn inline_object_anchors_share_the_paragraph_visual_map() {
        let bidi = ParagraphBidi::new("א\u{fffc}12ב", 0).unwrap();
        assert_eq!(bidi.base_direction(), Direction::RightToLeft);
        assert_eq!(
            bidi.visual_order(0..5, &scalar_items(0..5)).unwrap(),
            [4, 2, 3, 1, 0]
        );
    }

    #[test]
    fn supplementary_scalars_and_combining_clusters_stay_atomic() {
        let bidi = ParagraphBidi::new("א😀12ב", 20).unwrap();
        assert_eq!(
            bidi.visual_order(20..25, &scalar_items(20..25)).unwrap(),
            [4, 2, 3, 1, 0]
        );
        let combined = ParagraphBidi::new("Aא\u{5b0}בZ", 0).unwrap();
        assert_eq!(
            combined
                .visual_order(0..5, &[0..1, 1..3, 3..4, 4..5])
                .unwrap(),
            [0, 2, 1, 3]
        );
    }

    #[test]
    fn paragraph_level_reset_matches_icu_for_an_unclosed_override() {
        let bidi = ParagraphBidi::new("A\u{202e}אב  ", 0).unwrap();
        assert_eq!(
            bidi.levels.iter().map(Level::number).collect::<Vec<_>>(),
            [0, 1, 1, 1, 0, 0]
        );
        assert_eq!(bidi.direction_at(4), Some(Direction::LeftToRight));
        assert_eq!(
            bidi.visual_order(0..6, &scalar_items(0..6)).unwrap(),
            [0, 3, 2, 1, 4, 5]
        );
    }

    #[test]
    fn all_even_nested_embedding_levels_are_preserved() {
        let bidi = ParagraphBidi::new("A\u{202b}ABC\u{202c}Z", 0).unwrap();
        assert_eq!(
            bidi.levels().iter().map(Level::number).collect::<Vec<_>>(),
            [0, 2, 2, 2, 2, 0, 0]
        );
        assert_eq!(
            bidi.visual_order(0..7, &scalar_items(0..7)).unwrap(),
            [0, 1, 2, 3, 4, 5, 6]
        );
        assert!((0..7).all(|index| bidi.direction_at(index) == Some(Direction::LeftToRight)));
    }

    #[test]
    fn uniform_paragraph_directions_and_ranks_match_icu() {
        for (text, direction, expected) in [
            (
                "A\u{202a}BC\u{202c}Z",
                Direction::LeftToRight,
                vec![0, 1, 2, 3, 4, 5],
            ),
            (
                "\u{202b}אב\u{202c}",
                Direction::RightToLeft,
                vec![3, 2, 1, 0],
            ),
        ] {
            let bidi = ParagraphBidi::new(text, 0).unwrap();
            let len = text.chars().count();
            assert_eq!(bidi.base_direction(), direction);
            assert!((0..len).all(|index| bidi.direction_at(index) == Some(direction)));
            assert_eq!(
                bidi.visual_order(0..len, &scalar_items(0..len)).unwrap(),
                expected
            );
        }
    }

    #[test]
    fn removed_controls_copy_following_levels_but_isolates_keep_their_levels() {
        for text in ["A\u{200d}אבZ", "A\u{feff}אבZ"] {
            let bidi = ParagraphBidi::new(text, 0).unwrap();
            assert_eq!(
                bidi.levels().iter().map(Level::number).collect::<Vec<_>>(),
                [0, 1, 1, 1, 0]
            );
            assert_eq!(
                bidi.visual_order(0..5, &scalar_items(0..5)).unwrap(),
                [0, 3, 2, 1, 4]
            );
        }
        let isolate = ParagraphBidi::new("A\u{2067}אב\u{2069}Z", 0).unwrap();
        assert_eq!(
            isolate
                .levels()
                .iter()
                .map(Level::number)
                .collect::<Vec<_>>(),
            [0, 0, 1, 1, 0, 0]
        );
        assert_eq!(
            isolate.visual_order(0..6, &scalar_items(0..6)).unwrap(),
            [0, 1, 3, 2, 4, 5]
        );
    }

    #[test]
    fn unverified_internal_paragraph_separators_reject_only_visual_ordering() {
        let bidi = ParagraphBidi::new("א\u{2029}  Z", 0).unwrap();
        assert_eq!(bidi.base_direction(), Direction::RightToLeft);
        assert_eq!(bidi.direction_at(4), Some(Direction::LeftToRight));
        assert_eq!(
            bidi.visual_order(0..5, &scalar_items(0..5)),
            Err(BidiError::UnsupportedParagraphSeparator)
        );
    }

    #[test]
    fn invalid_and_cross_level_cluster_ranges_fail_without_reordering_source() {
        let bidi = ParagraphBidi::new("Aאב12גZ", 10).unwrap();
        for (line, items) in [
            (9..17, std::iter::once(10..11).collect()),
            (10..18, std::iter::once(10..11).collect()),
            (10..17, vec![11..12, 10..11]),
            (10..17, vec![11..13, 12..13]),
            (10..17, std::iter::once(10..10).collect()),
        ] {
            assert_eq!(
                bidi.visual_order(line, &items),
                Err(BidiError::InvalidRange)
            );
        }
        assert_eq!(
            bidi.visual_order(10..17, std::slice::from_ref(&(10..12))),
            Err(BidiError::MixedEmbeddingLevels)
        );
        assert_eq!(
            bidi.visual_order(10..17, std::slice::from_ref(&(12..14))),
            Err(BidiError::MixedEmbeddingLevels)
        );
        assert!(matches!(
            ParagraphBidi::new("AB", usize::MAX),
            Err(BidiError::InvalidRange)
        ));
    }

    #[test]
    fn ascii_and_empty_paragraphs_keep_source_order() {
        let bidi = ParagraphBidi::new("ABC 123", 30).unwrap();
        assert_eq!(
            bidi.visual_order(31..36, &scalar_items(31..36)).unwrap(),
            [0, 1, 2, 3, 4]
        );
        let empty = ParagraphBidi::new("", 30).unwrap();
        assert_eq!(empty.base_direction(), Direction::LeftToRight);
        assert!(empty.visual_order(30..30, &[]).unwrap().is_empty());
        assert_eq!(empty.direction_at(0), None);
    }
}
