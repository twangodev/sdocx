use super::{Node, Path, Polyline, decimal};

#[derive(Clone)]
pub struct Data {
    inner: svg::node::element::path::Data,
    valid: bool,
}
impl Default for Data {
    fn default() -> Self {
        Self::new()
    }
}
impl Data {
    pub fn new() -> Self {
        Self {
            inner: Default::default(),
            valid: true,
        }
    }
    fn parameters<const N: usize>(&mut self, values: [f64; N]) -> Vec<f32> {
        let values: Vec<_> = values.into_iter().map(|value| value as f32).collect();
        self.valid &= values.iter().all(|value| value.is_finite());
        values
    }
    pub fn move_to(mut self, point: (impl Into<f64>, impl Into<f64>)) -> Self {
        let values = self.parameters([point.0.into(), point.1.into()]);
        self.inner = self.inner.move_to(values);
        self
    }
    pub fn line_to(mut self, point: (impl Into<f64>, impl Into<f64>)) -> Self {
        let values = self.parameters([point.0.into(), point.1.into()]);
        self.inner = self.inner.line_to(values);
        self
    }
    pub fn horizontal_line_to(mut self, x: impl Into<f64>) -> Self {
        let values = self.parameters([x.into()]);
        self.inner = self.inner.horizontal_line_to(values);
        self
    }
    pub fn horizontal_line_by(mut self, x: impl Into<f64>) -> Self {
        let values = self.parameters([x.into()]);
        self.inner = self.inner.horizontal_line_by(values);
        self
    }
    pub fn vertical_line_by(mut self, y: impl Into<f64>) -> Self {
        let values = self.parameters([y.into()]);
        self.inner = self.inner.vertical_line_by(values);
        self
    }
    pub fn quadratic_curve_to(mut self, p: (f32, f32, f32, f32)) -> Self {
        let values = self.parameters([p.0.into(), p.1.into(), p.2.into(), p.3.into()]);
        self.inner = self.inner.quadratic_curve_to(values);
        self
    }
    pub fn quadratic_curve_by(mut self, p: (f32, f32, f32, f32)) -> Self {
        let values = self.parameters([p.0.into(), p.1.into(), p.2.into(), p.3.into()]);
        self.inner = self.inner.quadratic_curve_by(values);
        self
    }
    pub fn cubic_curve_to(mut self, p: (f32, f32, f32, f32, f32, f32)) -> Self {
        let values = self.parameters([
            p.0.into(),
            p.1.into(),
            p.2.into(),
            p.3.into(),
            p.4.into(),
            p.5.into(),
        ]);
        self.inner = self.inner.cubic_curve_to(values);
        self
    }
    pub fn elliptical_arc_by(mut self, p: (f32, f32, f32, bool, bool, f32, f32)) -> Self {
        self.valid &= p.0 >= 0. && p.1 >= 0.;
        let values = self.parameters([
            p.0.into(),
            p.1.into(),
            p.2.into(),
            u8::from(p.3).into(),
            u8::from(p.4).into(),
            p.5.into(),
            p.6.into(),
        ]);
        self.inner = self.inner.elliptical_arc_by(values);
        self
    }
    pub fn close(mut self) -> Self {
        self.inner = self.inner.close();
        self
    }
    fn text(self) -> Option<String> {
        self.valid
            .then(|| svg::node::Value::from(self.inner).into())
    }
}
impl Path {
    pub fn data(mut self, data: Data) -> Self {
        self.0.optional("d", data.text());
        self
    }
}

pub struct ReplayPath {
    data: String,
    lengths: Vec<usize>,
    valid: bool,
}
impl Default for ReplayPath {
    fn default() -> Self {
        Self {
            data: String::new(),
            lengths: Vec::new(),
            valid: true,
        }
    }
}
impl ReplayPath {
    pub fn push(&mut self, part: Data, replay: bool) {
        if let Some(part) = part.text() {
            self.data.push_str(&part);
        } else {
            self.valid = false;
        }
        if replay {
            self.lengths.push(self.data.len());
        }
    }
    pub fn finish(self) -> Path {
        let mut path = Path::new();
        path.0.valid = self.valid;
        path.0.attr("d", self.data);
        replay_lengths(&mut path.0, &self.lengths);
        path
    }
}

pub fn polyline(points: &[crate::Point], replay: bool) -> Polyline {
    let mut node = Polyline::new();
    let mut value = String::new();
    let mut offsets = Vec::new();
    for point in points {
        if !value.is_empty() {
            value.push(' ');
        }
        let (Some(x), Some(y)) = (decimal(point.x, 2), decimal(point.y, 2)) else {
            node.0.valid = false;
            break;
        };
        value.push_str(&svg::node::Value::from((x.text(), y.text())));
        if replay {
            offsets.push(value.len());
        }
    }
    node.0.attr("points", value);
    replay_lengths(&mut node.0, &offsets);
    node
}
fn replay_lengths(node: &mut Node, lengths: &[usize]) {
    if !lengths.is_empty() {
        node.attr(
            "data-replay-lengths",
            lengths
                .iter()
                .map(usize::to_string)
                .collect::<Vec<_>>()
                .join(","),
        );
    }
}

pub fn coordinate(value: f64, places: usize) -> f32 {
    decimal(value, places)
        .and_then(|value| value.text().parse().ok())
        .unwrap_or(f32::NAN)
}
