#[derive(Clone, Copy, Debug)]
pub(super) struct NativeLineMetrics {
    pub max_font: f32,
    pub base_height: f32,
    pub extra_pixels: f32,
    pub multiplier: f32,
    pub height_limit: f32,
    pub object_margin_line: bool,
    pub has_object_metric: bool,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct NativeLineBands {
    pub top: f32,
    pub baseline: f32,
    pub bottom: f32,
    pub overflow: bool,
}

impl NativeLineMetrics {
    pub fn height(self) -> (f32, bool) {
        if self.object_margin_line {
            return (self.base_height, false);
        }
        let height = if self.extra_pixels == 0.0 {
            self.max_font
                .mul_add(self.multiplier - 1.0, self.base_height)
        } else {
            self.base_height + self.extra_pixels
        };
        if height > self.height_limit {
            (self.base_height, true)
        } else {
            (height, false)
        }
    }

    pub fn place(self, cursor: f32, top_margin: f32) -> Option<NativeLineBands> {
        if ![
            self.max_font,
            self.base_height,
            self.extra_pixels,
            self.multiplier,
            cursor,
            top_margin,
        ]
        .into_iter()
        .all(f32::is_finite)
            || self.height_limit.is_nan()
        {
            return None;
        }
        let top = cursor + top_margin;
        let (height, overflow) = self.height();
        let mut bottom = top + height;
        let mut baseline = if self.object_margin_line {
            bottom
        } else {
            self.max_font.mul_add(-0.35, bottom)
        };
        if self.has_object_metric {
            bottom += 0.001;
            baseline += 0.001;
        }
        [top, baseline, bottom]
            .into_iter()
            .all(f32::is_finite)
            .then_some(NativeLineBands {
                top,
                baseline,
                bottom,
                overflow,
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nonfinite_native_bands_are_rejected() {
        let metrics = NativeLineMetrics {
            max_font: 1e38,
            base_height: 3e38,
            extra_pixels: 1e38,
            multiplier: 1.35,
            height_limit: f32::INFINITY,
            object_margin_line: false,
            has_object_metric: false,
        };
        assert!(metrics.place(0.0, 0.0).is_none());
        assert!(
            NativeLineMetrics {
                extra_pixels: 0.0,
                ..metrics
            }
            .place(f32::NAN, 0.0)
            .is_none()
        );
    }

    #[test]
    fn supplied_height_limit_falls_back_before_baseline_and_object_epsilon() {
        let metrics = NativeLineMetrics {
            max_font: 20.0,
            base_height: 100.0,
            extra_pixels: 20.0,
            multiplier: 1.35,
            height_limit: 119.0,
            object_margin_line: false,
            has_object_metric: true,
        };
        let bands = metrics.place(3.25, 4.75).unwrap();
        assert!(bands.overflow);
        assert_eq!(bands.top.to_bits(), 8.0_f32.to_bits());
        assert_eq!(bands.bottom.to_bits(), 108.001_f32.to_bits());
        assert_eq!(bands.baseline.to_bits(), 101.001_f32.to_bits());
        let unlimited = NativeLineMetrics {
            height_limit: f32::INFINITY,
            ..metrics
        }
        .place(3.25, 4.75)
        .unwrap();
        assert!(!unlimited.overflow);
        assert_eq!(unlimited.bottom.to_bits(), 128.001_f32.to_bits());
        assert_eq!(unlimited.baseline.to_bits(), 121.001_f32.to_bits());
    }
}
