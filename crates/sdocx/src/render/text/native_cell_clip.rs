#![cfg_attr(not(test), allow(dead_code))]

#[derive(Debug, Clone, Copy)]
pub(in crate::render) struct NativeCellTextClipContext {
    model_rect: [f32; 4],
    caller_origin: [f32; 2],
    model_height: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(in crate::render) enum NativeCellRunClip {
    Unclipped,
    Clipped {
        world_rect: [f32; 4],
        intersects_source: bool,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("native cell clipping has invalid or overflowing geometry")]
pub(in crate::render) struct NativeCellClipError;

impl NativeCellTextClipContext {
    pub fn new(model_rect: [f32; 4], caller_origin: [f32; 2]) -> Result<Self, NativeCellClipError> {
        let model_height = model_rect[3] - model_rect[1];
        if !model_rect
            .into_iter()
            .chain(caller_origin)
            .chain([model_height])
            .all(f32::is_finite)
        {
            return Err(NativeCellClipError);
        }
        Ok(Self {
            model_rect,
            caller_origin,
            model_height,
        })
    }

    pub fn decide(
        self,
        local_run_rect: [f32; 4],
    ) -> Result<NativeCellRunClip, NativeCellClipError> {
        if !local_run_rect.into_iter().all(f32::is_finite) {
            return Err(NativeCellClipError);
        }
        if local_run_rect[3] <= self.model_height {
            return Ok(NativeCellRunClip::Unclipped);
        }
        let [left, top, right, bottom] = local_run_rect;
        let [x, y] = self.caller_origin;
        let mut world_rect = [left + x, top + y, right + x, bottom + y];
        if !world_rect.into_iter().all(f32::is_finite) {
            return Err(NativeCellClipError);
        }
        let intersects_source = world_rect[0] < self.model_rect[2]
            && self.model_rect[0] < world_rect[2]
            && world_rect[1] < self.model_rect[3]
            && self.model_rect[1] < world_rect[3];
        if intersects_source {
            for axis in 0..2 {
                if world_rect[axis] < self.model_rect[axis] {
                    world_rect[axis] = self.model_rect[axis];
                }
                if world_rect[axis + 2] > self.model_rect[axis + 2] {
                    world_rect[axis + 2] = self.model_rect[axis + 2];
                }
            }
        }
        Ok(NativeCellRunClip::Clipped {
            world_rect,
            intersects_source,
        })
    }
}

#[cfg(all(test, feature = "serde"))]
#[path = "native_cell_clip/fixture_tests.rs"]
mod fixture_tests;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finite_admission_rejects_overflow_without_normalizing_native_dimensions() {
        assert!(
            NativeCellTextClipContext::new([0.0, -f32::MAX, 10.0, f32::MAX], [0.0; 2]).is_err()
        );
        assert!(NativeCellTextClipContext::new([f32::NAN, 0.0, 10.0, 10.0], [0.0; 2]).is_err());
        assert!(
            NativeCellTextClipContext::new([0.0, 0.0, 10.0, 10.0], [f32::INFINITY, 0.0]).is_err()
        );
        let context =
            NativeCellTextClipContext::new([0.0, 0.0, 10.0, 0.0], [f32::MAX, 0.0]).unwrap();
        assert!(context.decide([f32::MAX, 0.0, f32::MAX, 1.0]).is_err());
        let inverted = NativeCellTextClipContext::new([10.0, 10.0, 0.0, 0.0], [0.0; 2]).unwrap();
        assert_eq!(
            inverted.decide([1.0, 1.0, 5.0, 5.0]).unwrap(),
            NativeCellRunClip::Clipped {
                world_rect: [1.0, 1.0, 5.0, 5.0],
                intersects_source: false,
            }
        );
    }

    #[test]
    fn equal_intersection_edges_keep_the_translated_rectangle_bits() {
        let context = NativeCellTextClipContext::new([0.0, 0.0, 10.0, 10.0], [-0.0, -0.0]).unwrap();
        let NativeCellRunClip::Clipped {
            world_rect,
            intersects_source,
        } = context.decide([-0.0, -0.0, 10.0, 11.0]).unwrap()
        else {
            panic!("run must exceed source height");
        };
        assert!(intersects_source);
        assert_eq!(
            world_rect.map(f32::to_bits),
            [-0.0_f32, -0.0, 10.0, 10.0].map(f32::to_bits)
        );
    }
}
