#![cfg_attr(not(test), allow(dead_code))]

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
