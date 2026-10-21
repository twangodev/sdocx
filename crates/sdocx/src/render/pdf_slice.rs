#[cfg(feature = "pdf")]
use crate::LayoutPage;

/// A note-coordinate viewport emitted as one PDF overlay page.
///
/// The end may extend past the note page's stored height: native PDF-paper
/// exports retain the final selected segment height even when it overshoots
/// the physical note page. The start must still belong to the note page.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct PageSlice {
    y: u32,
    height: u32,
    include_canvas: bool,
    #[cfg(test)]
    cull_fountain_paint: bool,
}

#[cfg(feature = "pdf")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PageSliceError {
    ZeroHeight,
    StartOutsidePage { y: u32, page_height: u32 },
    EndOverflow { y: u32, height: u32 },
}

impl PageSlice {
    #[cfg(feature = "pdf")]
    pub(crate) fn new_for_page(
        page: &LayoutPage,
        y: u32,
        height: u32,
        include_canvas: bool,
    ) -> Result<Self, PageSliceError> {
        if height == 0 {
            return Err(PageSliceError::ZeroHeight);
        }
        if y >= page.page.height {
            return Err(PageSliceError::StartOutsidePage {
                y,
                page_height: page.page.height,
            });
        }
        y.checked_add(height)
            .ok_or(PageSliceError::EndOverflow { y, height })?;
        Ok(Self {
            y,
            height,
            include_canvas,
            #[cfg(test)]
            cull_fountain_paint: true,
        })
    }

    pub(crate) const fn y(self) -> u32 {
        self.y
    }

    pub(crate) const fn height(self) -> u32 {
        self.height
    }

    pub(crate) const fn include_canvas(self) -> bool {
        self.include_canvas
    }

    #[cfg(test)]
    fn with_fountain_paint_culling(mut self, enabled: bool) -> Self {
        self.cull_fountain_paint = enabled;
        self
    }

    pub(crate) const fn fountain_paint_culling_enabled(self) -> bool {
        #[cfg(test)]
        {
            self.cull_fountain_paint
        }
        #[cfg(not(test))]
        {
            true
        }
    }
}

/// Per-output-page scene request used only by the PDF export pipeline.
#[cfg(feature = "pdf")]
#[derive(Clone, Copy, Debug)]
pub(crate) struct PageSceneRequest<'a> {
    layout_page: &'a LayoutPage,
    slice: Option<PageSlice>,
}

#[cfg(feature = "pdf")]
impl<'a> PageSceneRequest<'a> {
    pub(crate) const fn full_page(layout_page: &'a LayoutPage) -> Self {
        Self {
            layout_page,
            slice: None,
        }
    }

    #[cfg(feature = "pdf")]
    pub(crate) fn sliced(
        layout_page: &'a LayoutPage,
        y: u32,
        height: u32,
        include_canvas: bool,
    ) -> Result<Self, PageSliceError> {
        Ok(Self {
            layout_page,
            slice: Some(PageSlice::new_for_page(
                layout_page,
                y,
                height,
                include_canvas,
            )?),
        })
    }

    #[cfg(test)]
    pub(crate) fn with_fountain_paint_culling(mut self, enabled: bool) -> Self {
        self.slice = self
            .slice
            .map(|slice| slice.with_fountain_paint_culling(enabled));
        self
    }

    pub(crate) const fn layout_page(self) -> &'a LayoutPage {
        self.layout_page
    }

    pub(crate) const fn slice(self) -> Option<PageSlice> {
        self.slice
    }
}

#[cfg(all(test, feature = "pdf"))]
mod tests {
    use super::{PageSceneRequest, PageSlice, PageSliceError};
    use crate::{BoundingBox, LayoutPage, Page};

    fn layout_page(height: u32) -> LayoutPage {
        LayoutPage {
            page: Page {
                uuid: "test".to_owned(),
                width: 100,
                height,
                content_bbox: BoundingBox::default(),
                background_color: None,
                template: None,
                background: Default::default(),
                objects: Vec::new(),
            },
            source_page_index: 0,
            body_text: None,
        }
    }

    #[test]
    fn slices_allow_native_final_height_to_extend_past_note_bottom() {
        let page = layout_page(15_372);
        let request = PageSceneRequest::sliced(&page, 15_228, 1_527, false).unwrap();
        let slice = request.slice().unwrap();
        assert_eq!(slice.y(), 15_228);
        assert_eq!(slice.height(), 1_527);
        assert!(!slice.include_canvas());
    }

    #[test]
    fn slices_reject_empty_out_of_page_starts_and_integer_overflow() {
        let page = layout_page(100);
        assert_eq!(
            PageSlice::new_for_page(&page, 0, 0, false),
            Err(PageSliceError::ZeroHeight)
        );
        assert_eq!(
            PageSlice::new_for_page(&page, 100, 1, false),
            Err(PageSliceError::StartOutsidePage {
                y: 100,
                page_height: 100,
            })
        );
        assert_eq!(
            PageSlice::new_for_page(&page, 99, u32::MAX, false),
            Err(PageSliceError::EndOverflow {
                y: 99,
                height: u32::MAX,
            })
        );
    }
}
