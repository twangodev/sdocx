use super::ShapeStyle;

#[cfg_attr(not(feature = "render"), allow(dead_code))]
pub(crate) struct DashPattern {
    intervals: [f32; 6],
    count: usize,
    pub phase: f32,
}

#[cfg(feature = "render")]
impl DashPattern {
    pub fn intervals(&self) -> &[f32] {
        &self.intervals[..self.count]
    }
}

impl ShapeStyle {
    pub(crate) fn dash_pattern(&self) -> Result<Option<DashPattern>, ()> {
        if self.dash == 0 {
            return Ok(None);
        }
        let w = self.width;
        let (intervals, count, phase) = match self.dash {
            1 => ([0.01, w + w, 0.0, 0.0, 0.0, 0.0], 2, 0.0),
            2 => ([w + w, w, 0.0, 0.0, 0.0, 0.0], 2, 1.0),
            3 => ([4.0 * w, 3.0 * w, 0.0, 0.0, 0.0, 0.0], 2, 1.0),
            4 => ([4.0 * w, 3.0 * w, w, 3.0 * w, 0.0, 0.0], 4, 1.0),
            5 => ([8.0 * w, 3.0 * w, 0.0, 0.0, 0.0, 0.0], 2, 1.0),
            6 => ([8.0 * w, 3.0 * w, w, 3.0 * w, 0.0, 0.0], 4, 1.0),
            7 => ([8.0 * w, 3.0 * w, w, 3.0 * w, w, 3.0 * w], 6, 1.0),
            _ => return Err(()),
        };
        if !w.is_finite()
            || w <= 0.0
            || intervals[..count]
                .iter()
                .any(|v| !v.is_finite() || *v <= 0.0)
        {
            return Err(());
        }
        Ok(Some(DashPattern {
            intervals,
            count,
            phase,
        }))
    }
}
