use crate::{Document, ObjectSpanLayoutConstraint};

use super::{TextSettings, VerticalExclusion};

pub(in crate::render) struct PageExclusions {
    boundaries: Vec<f64>,
    padding: f64,
}

impl PageExclusions {
    pub fn for_page(
        document: &Document,
        source_page_index: usize,
        settings: TextSettings,
    ) -> Option<Self> {
        if document.metadata.page_mode != Some(0) || source_page_index >= document.pages.len() {
            return None;
        }
        let (width, height) = document.metadata.default_page_dimensions?;
        let density_axis = if document.metadata.orientation.unwrap_or(0) == 0 {
            width
        } else {
            height
        };
        if density_axis as i32 <= 0 {
            return None;
        }
        let mut boundaries = Vec::with_capacity(document.pages.len() + 1);
        let mut height = 0.0;
        let mut page_top = 0.0;
        for (index, page) in document.pages.iter().enumerate() {
            if index == source_page_index {
                page_top = height;
            }
            boundaries.push(height);
            height += f64::from(page.height);
        }
        boundaries.push(height);
        for boundary in &mut boundaries {
            *boundary -= page_top;
        }
        Some(Self {
            boundaries,
            padding: settings.pixels(10.0),
        })
    }

    pub fn for_object(
        &self,
        constraint: ObjectSpanLayoutConstraint,
        stored_top: f64,
    ) -> Vec<VerticalExclusion> {
        if !stored_top.is_finite() {
            return Vec::new();
        }
        self.boundaries
            .iter()
            .filter_map(|&boundary| {
                if boundary + self.padding < stored_top {
                    return None;
                }
                let band = match constraint {
                    ObjectSpanLayoutConstraint::OverPagesOverlapPadding => VerticalExclusion {
                        top: boundary,
                        bottom: boundary + 1.0,
                    },
                    ObjectSpanLayoutConstraint::OverPages => VerticalExclusion {
                        top: boundary - self.padding,
                        bottom: boundary + self.padding,
                    },
                    _ => return None,
                };
                (band.bottom > stored_top).then_some(band)
            })
            .collect()
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
