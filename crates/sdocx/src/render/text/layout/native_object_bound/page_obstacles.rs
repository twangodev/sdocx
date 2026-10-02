use crate::{BoundingBox, Document};

use super::super::{ExclusionKind, TextFrame};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::render) struct NativeObjectPageObstacles {
    width: i32,
    height: i32,
}

#[derive(Clone, Copy)]
pub(in crate::render) struct NativeCellPageObstacles {
    page: NativeObjectPageObstacles,
}

impl NativeObjectPageObstacles {
    pub fn from_document(document: &Document) -> Option<Self> {
        let [page] = document.pages.as_slice() else {
            return None;
        };
        let source = document.metadata.note_text.as_ref()?;
        if document.metadata.page_mode != Some(0)
            || document.metadata.document_density() != 1.0
            || document.metadata.flow_page_padding != Some((0, 0))
            || !page.objects.is_empty()
            || !source.text_sections.is_empty()
        {
            return None;
        }
        let width = i32::try_from(page.width).ok()?;
        let height = i32::try_from(page.height).ok()?;
        if width <= 0
            || height <= 20
            || source.bbox
                != (BoundingBox {
                    x_min: 0.0,
                    y_min: 0.0,
                    x_max: f64::from(width as f32),
                    y_max: f64::from(height as f32),
                })
            || f64::from(width as f32) != f64::from(width)
            || f64::from(height as f32) != f64::from(height)
        {
            return None;
        }
        Some(Self { width, height })
    }

    pub(super) fn supports_parent_frame(self, frame: &TextFrame<'_>) -> bool {
        let [first, last] = frame.exclusions else {
            return false;
        };
        frame.bbox.x_min == 0.0
            && frame.bbox.y_min == 0.0
            && frame.bbox.x_max == f64::from(self.width)
            && frame.bbox.y_max == 0.0
            && first.kind == ExclusionKind::PagePadding
            && last.kind == ExclusionKind::PagePadding
            && [first.top, first.bottom] == [-10.0, 10.0]
            && [last.top, last.bottom]
                == [
                    f64::from(self.height as f32 - 10.0),
                    f64::from(self.height as f32 + 10.0),
                ]
    }

    pub fn matches_callback_bands(self, top: f32, bands: &[&BoundingBox]) -> bool {
        if !top.is_finite() {
            return false;
        }
        let height = self.height as f32;
        let expected = [[-10.0, 10.0], [height - 10.0, height + 10.0]]
            .into_iter()
            .filter(|band| band[1] > top)
            .map(|[start, end]| [0.0, start - top, self.width as f32, end - top]);
        let actual = bands
            .iter()
            .map(|band| [band.x_min, band.y_min, band.x_max, band.y_max]);
        actual.eq(expected.map(|band| band.map(f64::from)))
    }

    pub fn certify_cell_bands(
        self,
        callback_top: f32,
        bands: &[&BoundingBox],
    ) -> Option<NativeCellPageObstacles> {
        self.matches_callback_bands(callback_top, bands)
            .then_some(NativeCellPageObstacles { page: self })
    }
}

impl NativeCellPageObstacles {
    pub fn supports_frame(self, frame: &TextFrame<'_>) -> bool {
        frame.bbox.x_min == 0.0
            && frame.bbox.y_min == 0.0
            && frame.bbox.x_max > 0.0
            && frame.bbox.x_max <= f64::from(self.page.width)
            && frame.bbox.x_max.fract() == 0.0
            && frame.exclusions.iter().all(|band| {
                band.kind == ExclusionKind::Obstacle
                    && [band.top, band.bottom]
                        .into_iter()
                        .all(|value| value.is_finite() && f64::from(value as f32) == value)
                    && band.top <= band.bottom
            })
    }
}

#[cfg(all(test, feature = "serde"))]
mod tests {
    use super::*;

    #[test]
    fn original_document_controls_bound_page_obstacle_provenance() {
        let document = crate::parse_bytes(include_bytes!(
            "../../../../../tests/fixtures/native_body_table.sdocx"
        ))
        .unwrap();
        assert!(NativeObjectPageObstacles::from_document(&document).is_some());
        let unsupported: [fn(&mut Document); 8] = [
            |document| document.metadata.note_text = None,
            |document| document.metadata.note_text.as_mut().unwrap().bbox.y_max = 0.0,
            |document| document.metadata.note_text.as_mut().unwrap().bbox.y_max = f64::NAN,
            |document| document.metadata.note_text.as_mut().unwrap().bbox.x_min = 1.0,
            |document| document.metadata.flow_page_padding = Some((1, 0)),
            |document| document.metadata.page_mode = None,
            |document| document.pages.push(document.pages[0].clone()),
            |document| {
                document
                    .metadata
                    .note_text
                    .as_mut()
                    .unwrap()
                    .text_sections
                    .push(crate::RichTextSection {
                        start_utf16: 0,
                        length_utf16: 1,
                    });
            },
        ];
        for change in unsupported {
            let mut changed = document.clone();
            change(&mut changed);
            assert!(NativeObjectPageObstacles::from_document(&changed).is_none());
        }
    }
}
