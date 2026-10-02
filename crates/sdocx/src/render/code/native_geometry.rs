use crate::BoundingBox;

#[derive(Clone, Copy)]
struct NativeRect([f32; 4]);

impl NativeRect {
    fn translated(self, x: f32, y: f32) -> Self {
        let [left, top, right, bottom] = self.0;
        Self([left + x, top + y, right + x, bottom + y])
    }

    fn finite(self) -> bool {
        self.0.into_iter().all(f32::is_finite)
    }

    fn bounds(self) -> BoundingBox {
        let [x_min, y_min, x_max, y_max] = self.0.map(f64::from);
        BoundingBox {
            x_min,
            y_min,
            x_max,
            y_max,
        }
    }
}

pub(super) struct NativeCodeGeometry {
    source: NativeRect,
    pub copy: BoundingBox,
    pub title: BoundingBox,
    pub body: BoundingBox,
    top_padding: f32,
    body_gap: f32,
    bottom_padding: f32,
}

impl NativeCodeGeometry {
    pub fn new(
        bounds: BoundingBox,
        density: f32,
        first_split: Option<BoundingBox>,
    ) -> Option<Self> {
        let source = NativeRect([
            bounds.x_min as f32,
            bounds.y_min as f32,
            bounds.x_max as f32,
            bounds.y_max as f32,
        ]);
        let width = source.0[2] - source.0[0];
        let left = 16.0 * density;
        let right = 16.0 * density;
        let top_padding = 12.0 * density;
        let bottom_padding = 12.0 * density;
        let title_gap = 12.0 * density;
        let body_gap = 8.0 * density;
        let copy_size = 24.0 * density;
        let copy = NativeRect([0.0, 0.0, copy_size, copy_size])
            .translated((width - copy_size) - right, top_padding);
        let header_delta = first_split.map_or(0.0, |split| {
            let top = split.y_min as f32;
            if copy.0[3] > top { top } else { 0.0 }
        });
        let copy = copy.translated(0.0, header_delta);
        let title = NativeRect([left, copy.0[1], copy.0[0] - title_gap, copy.0[3]]);
        let mut body = NativeRect([0.0; 4]).translated(left, copy.0[3] + body_gap);
        body.0[2] += (width - right) - left;
        if density <= 0.0
            || ![source, copy, title, body]
                .into_iter()
                .all(NativeRect::finite)
        {
            return None;
        }
        Some(Self {
            source,
            copy: copy.bounds(),
            title: title.bounds(),
            body: body.bounds(),
            top_padding,
            body_gap,
            bottom_padding,
        })
    }

    pub fn absolute_frame(&self, local: BoundingBox) -> Option<BoundingBox> {
        let [x, y, _, _] = self.source.0.map(f64::from);
        let result = BoundingBox {
            x_min: local.x_min + x,
            y_min: local.y_min + y,
            x_max: local.x_max + x,
            y_max: local.y_max + y,
        };
        let unchanged_dimensions = result.x_max - result.x_min == local.x_max - local.x_min
            && result.y_max - result.y_min == local.y_max - local.y_min;
        unchanged_dimensions.then_some(result)
    }

    pub fn body_padding(&self, rectangles: &[BoundingBox]) -> Option<Vec<BoundingBox>> {
        rectangles
            .iter()
            .map(|rectangle| {
                let top = rectangle.y_min as f32 - self.body.y_min as f32;
                let bottom = rectangle.y_max as f32 - self.body.y_min as f32;
                (top.is_finite() && bottom.is_finite()).then_some(BoundingBox {
                    y_min: f64::from(top),
                    y_max: f64::from(bottom),
                    ..*rectangle
                })
            })
            .collect()
    }

    pub fn measured_bounds(&self, body_height: f32) -> Option<BoundingBox> {
        let mut result = self.source;
        let body_bottom = self.body.y_min as f32 + body_height;
        result.0[3] = ((result.0[1] + body_bottom) + self.body_gap) + self.bottom_padding;
        result.finite().then(|| result.bounds())
    }

    pub fn minimum_first_page_height(&self, first_body_height: f32) -> Option<f64> {
        let title_height = self.title.y_max as f32 - self.title.y_min as f32;
        let result = ((self.top_padding + title_height) + self.body_gap) + first_body_height;
        result.is_finite().then(|| f64::from(result))
    }
}
