#![cfg_attr(not(test), allow(dead_code))]

use crate::BoundingBox;
use crate::render::text::NativeObjectEntryBounds;
use crate::render::text::native_cell_clip::{NativeCellClipError, NativeCellTextClipContext};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::render) enum NativeCellTextClipProvenance {
    SavedSource,
    ActualDocumentPlacement,
    Unknown,
}

#[derive(Debug, Clone, Copy)]
pub(in crate::render) enum NativeCellTextPlacement {
    Unknown,
    SavedSource(NativeCellTextClipTransport),
    ActualDocumentPlacement(NativeCellTextClipTransport),
}

impl NativeCellTextPlacement {
    pub fn provenance(self) -> NativeCellTextClipProvenance {
        match self {
            Self::Unknown => NativeCellTextClipProvenance::Unknown,
            Self::SavedSource(_) => NativeCellTextClipProvenance::SavedSource,
            Self::ActualDocumentPlacement(_) => {
                NativeCellTextClipProvenance::ActualDocumentPlacement
            }
        }
    }

    pub fn transport(self) -> Option<NativeCellTextClipTransport> {
        match self {
            Self::Unknown => None,
            Self::SavedSource(transport) | Self::ActualDocumentPlacement(transport) => {
                Some(transport)
            }
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub(in crate::render) struct NativeCellTextClipTransport {
    native: NativeCellTextClipContext,
    output_translation: [f64; 2],
}

impl NativeCellTextClipTransport {
    pub fn native_context(self) -> NativeCellTextClipContext {
        self.native
    }

    pub fn output_translation(self) -> [f64; 2] {
        self.output_translation
    }

    fn new(
        model: NativeCellModelState,
        caller_origin: NativeCellTextWriterOrigin,
        output_translation: [f64; 2],
    ) -> Result<Self, NativeCellClipError> {
        if !output_translation.into_iter().all(f64::is_finite) {
            return Err(NativeCellClipError);
        }
        Ok(Self {
            native: NativeCellTextClipContext::new(model.content_rect(), caller_origin.0)?,
            output_translation,
        })
    }
}

#[derive(Debug, Clone, Copy)]
pub(in crate::render) struct NativeCellTextWriterOrigin([f32; 2]);

impl NativeCellTextWriterOrigin {
    pub fn coordinates(self) -> [f32; 2] {
        self.0
    }
}

#[derive(Debug, Clone, Copy)]
pub(in crate::render) struct NativeDocumentTablePlacement {
    parent_rect: BoundingBox,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(in crate::render) struct NativeTableModelState {
    raw_rect: [f32; 4],
    content_rect: [f32; 4],
}

#[derive(Debug, Clone, Copy)]
pub(in crate::render) struct NativeTableContentFit {
    pub mode: u8,
    pub maximum_height: f32,
    pub maximum_width: f32,
    pub maximum_height_enabled: bool,
}

impl Default for NativeTableContentFit {
    fn default() -> Self {
        Self {
            mode: 3,
            maximum_height: 0.0,
            maximum_width: 0.0,
            maximum_height_enabled: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(in crate::render) struct NativeTableContentResize {
    pub column_scale: f32,
    pub row_scale: f32,
}

impl NativeTableContentResize {
    pub fn preserves_column(self, width: f32) -> bool {
        let resized = self.column_scale * width;
        width.is_finite() && resized.is_finite() && resized.to_bits() == width.to_bits()
    }

    pub fn preserves_row(self, height: f32, maximum_height: f32) -> bool {
        let resized = self.row_scale * height;
        if ![height, maximum_height, resized]
            .into_iter()
            .all(f32::is_finite)
        {
            return false;
        }
        resized > maximum_height || resized.to_bits() == height.to_bits()
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(in crate::render) struct NativeTableRectUpdate {
    pub model: NativeTableModelState,
    pub resize: Option<NativeTableContentResize>,
}

impl NativeTableModelState {
    pub fn new(raw_rect: [f32; 4], content_rect: [f32; 4]) -> Result<Self, NativeCellClipError> {
        if !raw_rect.into_iter().chain(content_rect).all(f32::is_finite) {
            return Err(NativeCellClipError);
        }
        Ok(Self {
            raw_rect,
            content_rect,
        })
    }

    pub fn raw_rect(self) -> [f32; 4] {
        self.raw_rect
    }

    pub fn content_rect(self) -> [f32; 4] {
        self.content_rect
    }

    pub fn apply_document_placement(
        self,
        placement: NativeDocumentTablePlacement,
    ) -> Result<Self, NativeCellClipError> {
        Ok(self
            .set_rect(
                native_coordinates(placement.parent_rect()),
                NativeTableContentFit::default(),
            )?
            .model)
    }

    pub fn set_rect(
        self,
        raw_rect: [f32; 4],
        fit: NativeTableContentFit,
    ) -> Result<NativeTableRectUpdate, NativeCellClipError> {
        if !raw_rect.into_iter().all(f32::is_finite)
            || raw_rect[0] > raw_rect[2]
            || raw_rect[1] > raw_rect[3]
        {
            return Err(NativeCellClipError);
        }
        if rectangles_within(raw_rect, self.raw_rect, 0.01_f32) {
            return Ok(NativeTableRectUpdate {
                model: self,
                resize: None,
            });
        }
        let content_rect = [
            0.0 + raw_rect[0],
            0.0 + raw_rect[1],
            raw_rect[0] + (self.content_rect[2] - self.content_rect[0]),
            raw_rect[1] + (self.content_rect[3] - self.content_rect[1]),
        ];
        let fitted = Self::new(raw_rect, content_rect)?.fit_content(fit)?;
        let content_rect = normalized_rect(content_rect);
        if rectangles_within(content_rect, self.content_rect, 0.01_f32) {
            return Ok(NativeTableRectUpdate {
                model: Self::new(fitted.raw_rect, self.content_rect)?,
                resize: None,
            });
        }
        let scale = |axis: usize| {
            let previous = self.content_rect[axis + 2] - self.content_rect[axis];
            if previous == 0.0 {
                0.0
            } else {
                (content_rect[axis + 2] - content_rect[axis]) / previous
            }
        };
        let resize = NativeTableContentResize {
            column_scale: scale(0),
            row_scale: scale(1),
        };
        if ![resize.column_scale, resize.row_scale]
            .into_iter()
            .all(f32::is_finite)
        {
            return Err(NativeCellClipError);
        }
        Ok(NativeTableRectUpdate {
            model: Self::new(fitted.raw_rect, content_rect)?,
            resize: Some(resize),
        })
    }

    pub fn set_content_size(
        self,
        size: [f32; 2],
        fit: NativeTableContentFit,
    ) -> Result<Self, NativeCellClipError> {
        if !size
            .into_iter()
            .chain([fit.maximum_height, fit.maximum_width])
            .all(f32::is_finite)
            || (size[0] <= 0.0 && size[1] <= 0.0)
        {
            return Err(NativeCellClipError);
        }
        if size
            == [
                self.content_rect[2] - self.content_rect[0],
                self.content_rect[3] - self.content_rect[1],
            ]
        {
            return Ok(self);
        }
        let mut next = self;
        next.content_rect[2] = self.content_rect[0] + size[0];
        next.content_rect[3] = self.content_rect[1] + size[1];
        next.fit_content(fit)
    }

    fn fit_content(self, fit: NativeTableContentFit) -> Result<Self, NativeCellClipError> {
        let mut next = self;
        if ![fit.maximum_height, fit.maximum_width]
            .into_iter()
            .all(f32::is_finite)
        {
            return Err(NativeCellClipError);
        }
        if fit.mode & 0xfe == 2 {
            next.raw_rect[3] = next.raw_rect[1] + (next.content_rect[3] - next.content_rect[1]);
        }
        if fit.mode | 2 == 3 {
            next.raw_rect[2] = next.raw_rect[0] + (next.content_rect[2] - next.content_rect[0]);
        }
        if fit.maximum_height > 0.0
            && fit.maximum_height_enabled
            && next.raw_rect[3] - next.raw_rect[1] > fit.maximum_height
        {
            next.raw_rect[3] = next.raw_rect[1] + fit.maximum_height;
        }
        if fit.maximum_width > 0.0 && next.raw_rect[2] - next.raw_rect[0] > fit.maximum_width {
            next.raw_rect[2] = next.raw_rect[0] + fit.maximum_width;
        }
        next.raw_rect = normalized_rect(next.raw_rect);
        Self::new(next.raw_rect, next.content_rect)
    }
}

fn rectangles_within(left: [f32; 4], right: [f32; 4], epsilon: f32) -> bool {
    left.into_iter()
        .zip(right)
        .all(|(left, right)| (left - right).abs() <= epsilon)
}

fn normalized_rect(mut rect: [f32; 4]) -> [f32; 4] {
    for axis in 0..2 {
        if rect[axis] > rect[axis + 2] {
            rect.swap(axis, axis + 2);
        }
    }
    rect
}

impl NativeDocumentTablePlacement {
    pub fn from_entry(
        entry: NativeObjectEntryBounds,
        page_origin: [f32; 2],
        current_raw_rect: BoundingBox,
        current_drawn_rect: BoundingBox,
    ) -> Result<Self, NativeCellClipError> {
        let bound = entry.text_bound();
        if !page_origin.into_iter().all(f32::is_finite) {
            return Err(NativeCellClipError);
        }
        let target = std::array::from_fn(|axis| bound[axis] - page_origin[axis % 2]);
        let parent_rect = crate::render::embedded::cloned_raw_bounds(
            current_raw_rect,
            current_drawn_rect,
            native_bounds(target),
        )
        .map_err(|_| NativeCellClipError)?;
        Ok(Self { parent_rect })
    }

    pub fn parent_rect(self) -> BoundingBox {
        self.parent_rect
    }

    pub fn cell_model(
        self,
        source: NativeCellModelState,
        physical_local_frame: [f32; 4],
    ) -> Result<NativeCellModelState, NativeCellClipError> {
        source
            .apply_feedback(
                [self.parent_rect.x_min as f32, self.parent_rect.y_min as f32],
                physical_local_frame,
            )
            .map_err(|_| NativeCellClipError)
    }

    pub fn cell_text(
        self,
        source: NativeCellModelState,
        physical_local_frame: [f32; 4],
        writer: NativeTableTextWriterWindow,
        output_translation: [f64; 2],
    ) -> Result<NativeCellTextPlacement, NativeCellClipError> {
        Ok(NativeCellTextPlacement::ActualDocumentPlacement(
            NativeCellTextClipTransport::new(
                self.cell_model(source, physical_local_frame)?,
                writer.cell_origin(physical_local_frame)?,
                output_translation,
            )?,
        ))
    }
}

fn native_bounds([x_min, y_min, x_max, y_max]: [f32; 4]) -> BoundingBox {
    BoundingBox {
        x_min: f64::from(x_min),
        y_min: f64::from(y_min),
        x_max: f64::from(x_max),
        y_max: f64::from(y_max),
    }
}

fn native_coordinates(rect: BoundingBox) -> [f32; 4] {
    [rect.x_min, rect.y_min, rect.x_max, rect.y_max].map(|value| value as f32)
}

#[derive(Debug, Clone, Copy)]
pub(in crate::render) struct NativeTableTextWriterWindow {
    rounded_measured_world: [f32; 4],
}

impl NativeTableTextWriterWindow {
    pub fn from_layout(
        virtual_drawn_rect: [f32; 4],
        measured_local_rect: [f32; 4],
    ) -> Result<Self, NativeCellClipError> {
        if !virtual_drawn_rect.into_iter().all(f32::is_finite) {
            return Err(NativeCellClipError);
        }
        Ok(Self {
            rounded_measured_world: offset_rounded_rect(
                measured_local_rect,
                [virtual_drawn_rect[0], virtual_drawn_rect[1]],
            )?,
        })
    }

    pub fn rounded_measured_world(self) -> [f32; 4] {
        self.rounded_measured_world
    }

    pub fn cell_origin(
        self,
        local_frame: [f32; 4],
    ) -> Result<NativeCellTextWriterOrigin, NativeCellClipError> {
        let rect = offset_rounded_rect(
            local_frame,
            [
                self.rounded_measured_world[0],
                self.rounded_measured_world[1],
            ],
        )?;
        Ok(NativeCellTextWriterOrigin([rect[0], rect[1]]))
    }
}

fn offset_rounded_rect(rect: [f32; 4], origin: [f32; 2]) -> Result<[f32; 4], NativeCellClipError> {
    if !rect.into_iter().chain(origin).all(f32::is_finite) {
        return Err(NativeCellClipError);
    }
    let shifted = std::array::from_fn::<_, 4, _>(|axis| rect[axis] + origin[axis % 2]);
    if !shifted.into_iter().all(f32::is_finite) {
        return Err(NativeCellClipError);
    }
    Ok([
        shifted[0].floor(),
        shifted[1].floor(),
        shifted[2].ceil(),
        shifted[3].ceil(),
    ])
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(in crate::render) struct NativeCellModelState {
    saved_rect: [f32; 4],
    content_rect: [f32; 4],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("native cell model placement has invalid or overflowing coordinates")]
pub(in crate::render) struct NativeCellModelError;

impl NativeCellModelState {
    pub fn new(saved_rect: [f32; 4], content_rect: [f32; 4]) -> Result<Self, NativeCellModelError> {
        if !saved_rect
            .into_iter()
            .chain(content_rect)
            .all(f32::is_finite)
        {
            return Err(NativeCellModelError);
        }
        Ok(Self {
            saved_rect,
            content_rect,
        })
    }

    pub fn saved_rect(self) -> [f32; 4] {
        self.saved_rect
    }

    pub fn content_rect(self) -> [f32; 4] {
        self.content_rect
    }

    pub fn saved_text(
        self,
        caller_origin: NativeCellTextWriterOrigin,
        output_translation: [f64; 2],
    ) -> Result<NativeCellTextPlacement, NativeCellClipError> {
        Ok(NativeCellTextPlacement::SavedSource(
            NativeCellTextClipTransport::new(self, caller_origin, output_translation)?,
        ))
    }

    pub fn apply_feedback(
        self,
        parent_origin: [f32; 2],
        local_frame: [f32; 4],
    ) -> Result<Self, NativeCellModelError> {
        if !parent_origin
            .into_iter()
            .chain(local_frame)
            .all(f32::is_finite)
        {
            return Err(NativeCellModelError);
        }
        let requested = std::array::from_fn(|axis| local_frame[axis] + parent_origin[axis % 2]);
        self.with_saved_rect(requested)
    }

    fn with_saved_rect(self, requested: [f32; 4]) -> Result<Self, NativeCellModelError> {
        if !requested.into_iter().all(f32::is_finite) {
            return Err(NativeCellModelError);
        }
        if requested == self.saved_rect {
            return Ok(self);
        }
        let mut content_rect = requested;
        for axis in 0..2 {
            if content_rect[axis] > content_rect[axis + 2] {
                content_rect.swap(axis, axis + 2);
            }
        }
        Ok(Self {
            saved_rect: requested,
            content_rect,
        })
    }
}

#[cfg(test)]
mod fixture_tests;
