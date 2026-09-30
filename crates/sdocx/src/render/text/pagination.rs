use std::ops::RangeInclusive;

use crate::{Document, ObjectSpanLayoutConstraint};

use super::{TextSettings, VerticalExclusion};

#[derive(Clone)]
pub(in crate::render) struct PageExclusions {
    boundaries: Vec<f64>,
    padding: f64,
}

impl PageExclusions {
    pub fn for_document(document: &Document, settings: TextSettings) -> Option<Self> {
        let last_page = document.pages.len().checked_sub(1)?;
        Self::for_range(document, 0..=last_page, settings)
    }

    pub fn for_range(
        document: &Document,
        pages: RangeInclusive<usize>,
        settings: TextSettings,
    ) -> Option<Self> {
        if document.metadata.page_mode != Some(0) || document.pages.is_empty() {
            return None;
        }
        if pages.is_empty() {
            return None;
        }
        let pages = document.pages.get(pages)?;
        let (width, height) = document.metadata.default_page_dimensions?;
        let density_axis = if document.metadata.orientation.unwrap_or(0) == 0 {
            width
        } else {
            height
        };
        if density_axis as i32 <= 0 {
            return None;
        }
        let mut boundaries = Vec::with_capacity(pages.len() + 1);
        let mut height = 0.0;
        for page in pages {
            boundaries.push(height);
            height += f64::from(page.height);
        }
        boundaries.push(height);
        Some(Self {
            boundaries,
            padding: settings.pixels(10.0),
        })
    }

    pub fn for_page(
        document: &Document,
        source_page_index: usize,
        settings: TextSettings,
    ) -> Option<Self> {
        document.pages.get(source_page_index)?;
        let mut exclusions = Self::for_document(document, settings)?;
        let page_top = exclusions.boundaries[source_page_index];
        for boundary in &mut exclusions.boundaries {
            *boundary -= page_top;
        }
        Some(exclusions)
    }

    pub fn line_bands(&self) -> Vec<VerticalExclusion> {
        self.boundaries
            .iter()
            .map(|boundary| {
                VerticalExclusion::page_padding(boundary - self.padding, boundary + self.padding)
            })
            .collect()
    }

    pub fn for_object(
        &self,
        constraint: ObjectSpanLayoutConstraint,
        stored_top: f64,
    ) -> Vec<VerticalExclusion> {
        if !stored_top.is_finite() {
            return Vec::new();
        }
        match constraint {
            ObjectSpanLayoutConstraint::OverPages => self
                .line_bands()
                .into_iter()
                .map(|band| VerticalExclusion::obstacle(band.top, band.bottom))
                .filter(|band| band.bottom > stored_top)
                .collect(),
            ObjectSpanLayoutConstraint::OverPagesOverlapPadding => self
                .boundaries
                .iter()
                .filter_map(|&boundary| {
                    let band = VerticalExclusion::obstacle(boundary, boundary + 1.0);
                    (band.bottom > stored_top).then_some(band)
                })
                .collect(),
            _ => Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{BoundingBox, DocumentMetadata, Page};

    use super::*;

    fn document(heights: &[u32], mode: Option<u16>) -> Document {
        Document {
            pages: heights
                .iter()
                .enumerate()
                .map(|(index, &height)| Page {
                    uuid: format!("page-{index}"),
                    width: 1080,
                    height,
                    content_bbox: BoundingBox::default(),
                    background_color: None,
                    template: None,
                    background: Default::default(),
                    objects: Vec::new(),
                })
                .collect(),
            metadata: DocumentMetadata {
                page_mode: mode,
                default_page_dimensions: Some((1080, 1527)),
                ..Default::default()
            },
        }
    }

    fn exclusions(document: &Document, page: usize) -> PageExclusions {
        PageExclusions::for_page(
            document,
            page,
            TextSettings::from_document(&document.metadata),
        )
        .unwrap()
    }

    fn bounds(bands: Vec<VerticalExclusion>) -> Vec<(f64, f64)> {
        bands
            .into_iter()
            .map(|band| (band.top, band.bottom))
            .collect()
    }

    #[test]
    fn capture_boundaries_restart_at_the_first_selected_physical_page() {
        let document = document(&[100, 200, 300, 400], Some(0));
        let settings = TextSettings::from_document(&document.metadata);
        let capture = PageExclusions::for_range(&document, 1..=2, settings).unwrap();
        assert_eq!(capture.boundaries, [0.0, 200.0, 500.0]);
        assert_eq!(
            bounds(capture.line_bands()),
            [(-30.0, 30.0), (170.0, 230.0), (470.0, 530.0)]
        );
        assert!(PageExclusions::for_range(&document, 1..=4, settings).is_none());
        let reversed = (2_usize, 1_usize);
        assert!(PageExclusions::for_range(&document, reversed.0..=reversed.1, settings).is_none());
    }

    #[test]
    fn global_line_bands_and_page_views_share_physical_boundaries() {
        let document = document(&[100, 200, 300], Some(0));
        let settings = TextSettings::from_document(&document.metadata);
        let global = PageExclusions::for_document(&document, settings).unwrap();
        assert_eq!(global.boundaries, [0.0, 100.0, 300.0, 600.0]);
        assert_eq!(
            bounds(global.line_bands()),
            [(-30.0, 30.0), (70.0, 130.0), (270.0, 330.0), (570.0, 630.0)]
        );
        let page = PageExclusions::for_page(&document, 2, settings).unwrap();
        let translated = bounds(page.line_bands())
            .into_iter()
            .map(|(top, bottom)| (top + 300.0, bottom + 300.0))
            .collect::<Vec<_>>();
        assert_eq!(bounds(global.line_bands()), translated);
        assert_eq!(
            bounds(global.for_object(ObjectSpanLayoutConstraint::OverPagesOverlapPadding, 150.0)),
            [(300.0, 301.0), (600.0, 601.0)]
        );
    }

    #[test]
    fn boundaries_use_all_physical_pages_without_flow_padding() {
        let mut document = document(&[100, 200, 200], Some(0));
        document.metadata.flow_dimensions = Some((1080, 520));
        document.metadata.flow_page_padding = Some((0, 10));
        let exclusions = exclusions(&document, 1);
        assert_eq!(exclusions.boundaries, [-100.0, 0.0, 200.0, 400.0]);
        assert_eq!(
            bounds(exclusions.for_object(ObjectSpanLayoutConstraint::OverPages, -101.0)),
            [
                (-130.0, -70.0),
                (-30.0, 30.0),
                (170.0, 230.0),
                (370.0, 430.0)
            ]
        );
    }

    #[test]
    fn collapsed_padding_reconciles_captured_code_body_skip() {
        let document = document(&[1527, 1527, 1527, 1527], Some(0));
        let bands = exclusions(&document, 2).for_object(
            ObjectSpanLayoutConstraint::OverPagesOverlapPadding,
            1297.751953125,
        );
        assert_eq!(bounds(bands), [(1527.0, 1528.0), (3054.0, 3055.0)]);
        let second_line_top = 1297.751953125 + 132.0 + 60.75;
        let skip = 1528.0 - second_line_top;
        assert_eq!(skip, 37.498046875);
        assert_eq!(374.25 + skip, 411.748046875);
    }

    #[test]
    fn constraint_and_object_origin_filter_bands() {
        let document = document(&[100, 100], Some(0));
        let exclusions = exclusions(&document, 1);
        for constraint in [
            ObjectSpanLayoutConstraint::Normal,
            ObjectSpanLayoutConstraint::Other(3),
        ] {
            assert!(exclusions.for_object(constraint, -100.0).is_empty());
        }
        assert_eq!(
            bounds(exclusions.for_object(ObjectSpanLayoutConstraint::OverPages, 30.0)),
            [(70.0, 130.0)]
        );
        assert_eq!(
            bounds(
                exclusions.for_object(ObjectSpanLayoutConstraint::OverPagesOverlapPadding, 1.0,)
            ),
            [(100.0, 101.0)]
        );
        assert!(
            exclusions
                .for_object(ObjectSpanLayoutConstraint::OverPages, f64::NAN)
                .is_empty()
        );
    }

    #[test]
    fn missing_or_unsupported_page_context_has_no_exclusions() {
        for mode in [None, Some(1), Some(2)] {
            let document = document(&[1527], mode);
            assert!(
                PageExclusions::for_page(
                    &document,
                    0,
                    TextSettings::from_document(&document.metadata),
                )
                .is_none()
            );
        }
        for (heights, page) in [(&[][..], 0), (&[1527][..], 1)] {
            let document = document(heights, Some(0));
            assert!(
                PageExclusions::for_page(
                    &document,
                    page,
                    TextSettings::from_document(&document.metadata),
                )
                .is_none()
            );
        }
    }

    #[test]
    fn unresolved_native_density_has_no_exclusions() {
        let mut document = document(&[1527], Some(0));
        for dimensions in [None, Some((0, 1527)), Some((u32::MAX, 1527))] {
            document.metadata.default_page_dimensions = dimensions;
            assert!(
                PageExclusions::for_page(
                    &document,
                    0,
                    TextSettings::from_document(&document.metadata),
                )
                .is_none()
            );
        }
        document.metadata.default_page_dimensions = Some((1080, 0));
        document.metadata.orientation = Some(1);
        assert!(
            PageExclusions::for_page(
                &document,
                0,
                TextSettings::from_document(&document.metadata),
            )
            .is_none()
        );
    }
}
