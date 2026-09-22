//! Saved Marker2 geometry, traced from Samsung Notes 4.4.45.37.
//!
//! V1 and V2 share this redraw. The V2 thin-stroke coverage ramp is a GPU
//! shader difference and is not applied here. Arithmetic stays float32 where
//! the native implementation uses float32.
use super::path::{P, Quad};
use crate::{Point, Stroke};

const SPACING: f32 = 1.;
const REJECT_BELOW: f32 = 2.;
const ALTERNATE_BELOW: f32 = 20.;
const PEN: &str = "com.samsung.android.sdk.pen.pen.preload.Marker2";

pub(super) struct MarkerInk {
    pub(super) points: Vec<Point>,
    pub(super) radii: Vec<f64>,
    pub(super) sample_ends: Vec<usize>,
    pub(super) opacity: f64,
}

struct Pass {
    previous: P,
    midpoint: P,
    residual: f32,
    alternate: bool,
    skip_short: bool,
    radius: f64,
    points: Vec<Point>,
    radii: Vec<f64>,
}

impl Pass {
    fn stamp(&mut self, p: P) {
        self.points.push(Point {
            x: p.x as f64,
            y: p.y as f64,
        });
        self.radii.push(self.radius);
    }

    fn line(&mut self, p: P) {
        let distance = p.sub(self.previous).len();
        if distance < REJECT_BELOW {
            return;
        }
        if self.skip_short && distance < ALTERNATE_BELOW {
            // The first accepted short move is dropped. The next one is kept.
            self.alternate = !self.alternate;
            if self.alternate {
                return;
            }
        }
        let mid = self.previous.mid(p);
        let q = Quad::new(self.midpoint, self.previous, mid);
        let mut d = self.residual;
        if q.length > 0. {
            while d <= q.length {
                self.stamp(q.at(d));
                d += SPACING;
            }
        }
        self.residual = d - q.length;
        self.previous = p;
        self.midpoint = mid;
    }
}

pub(super) fn prepare(s: &Stroke) -> Option<MarkerInk> {
    let r = s.rendering.as_ref()?;
    if r.pen_name.as_deref() != Some(PEN)
        || !accepted_settings(r.advanced_settings.as_deref())
        || r.properties.eraser
        || r.properties.straighten
        || r.properties.rainbow_effect
        || s.points.is_empty()
        || !s.pen_width.is_finite()
        || s.pen_width <= 0.
        || s.points
            .iter()
            .any(|p| !p.x.is_finite() || !p.y.is_finite() || p.x.abs() > 1e7 || p.y.abs() > 1e7)
    {
        return None;
    }
    let distance: f64 = s
        .points
        .windows(2)
        .map(|p| (p[1].x - p[0].x).hypot(p[1].y - p[0].y))
        .sum();
    if distance > 400_000. || s.points.len() > 100_000 {
        return None;
    }
    let radius = stamp_radius(s.pen_width);
    let first = P::from(s.points[0]);
    let mut pass = Pass {
        previous: first,
        midpoint: first,
        residual: SPACING,
        alternate: false,
        // Tool type 1 enables the short-move skip. Tool type 2 also requires
        // MotionEvent source 0x1002, which the saved tool/input word does not
        // contain, so a stored stylus stroke does not alternate.
        skip_short: r.tool_type_raw == 1,
        radius,
        points: Vec::new(),
        radii: Vec::new(),
    };
    pass.stamp(first);
    let mut sample_ends = vec![0; s.points.len()];
    sample_ends[0] = pass.points.len();
    for (index, point) in s.points.iter().enumerate().skip(1) {
        pass.line((*point).into());
        sample_ends[index] = pass.points.len();
    }
    Some(MarkerInk {
        points: pass.points,
        radii: pass.radii,
        sample_ends,
        opacity: opacity(r.style.color_argb),
    })
}

fn accepted_settings(settings: Option<&str>) -> bool {
    let Some(settings) = settings else {
        return true;
    };
    settings
        .split(';')
        .next()
        .unwrap_or("")
        .parse::<i32>()
        .is_ok_and(|value| value >= 0)
}

fn stamp_radius(pen_width: f32) -> f64 {
    // SetSize clamps, then SetPenData truncates with fcvtzu. The stamp quad's
    // local 0.5 offset is its texture center, not a page-space translation.
    let size = pen_width.clamp(0.4, 800.).trunc();
    f64::from(size) / 2.
}

fn opacity(argb: Option<u32>) -> f64 {
    argb.map_or(1., |argb| f64::from(argb >> 24) / 255.)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{BoundingBox, StrokeProperties, StrokeRendering, StrokeStyle};

    fn stroke(points: &[(f64, f64)], settings: Option<&str>, tool: u16) -> Stroke {
        Stroke {
            rendering: Some(StrokeRendering {
                pen_name: Some(PEN.into()),
                advanced_settings: settings.map(str::to_string),
                tool_type_raw: tool,
                properties: StrokeProperties {
                    compressed: false,
                    replay_only: false,
                    stylus_channels: false,
                    eraser: false,
                    fixed_width: false,
                    millisecond_timestamps: false,
                    top_layer_pen: true,
                    alpha_lock: false,
                    binary_added: true,
                    generated: false,
                    fixed_opacity: false,
                    rainbow_effect: false,
                    straighten: false,
                    reveal_mode: false,
                },
                style: StrokeStyle {
                    color_argb: Some(0x80ff_ee00),
                    ..StrokeStyle::default()
                },
            }),
            bbox: BoundingBox::default(),
            points: points
                .iter()
                .copied()
                .map(|(x, y)| Point { x, y })
                .collect(),
            pressures: vec![0.2; points.len()],
            timestamps: vec![],
            tilts: vec![],
            orientations: vec![],
            color: Some(crate::Color {
                r: 255,
                g: 238,
                b: 0,
            }),
            pen_width: 4.9,
        }
    }

    #[test]
    fn two_point_redraw_matches_the_traced_chord() {
        let ink = prepare(&stroke(&[(0., 0.), (4., 0.)], Some("2;"), 2)).unwrap();
        assert_eq!(ink.sample_ends, vec![1, 3]);
        assert!(ink.points.iter().all(|p| p.y.abs() < 1e-6));
        let xs: Vec<f64> = ink.points.iter().map(|p| p.x).collect();
        assert!((xs[0] - 0.).abs() < 1e-6, "{xs:?}");
        assert!((xs[1] - 0.499_999_642_4).abs() < 1e-7, "{xs:?}");
        assert!((xs[2] - 1.999_998_569_5).abs() < 1e-7, "{xs:?}");
        assert!(xs.iter().all(|x| (*x - 4.).abs() > 0.1));
        assert!(ink.radii.iter().all(|r| (*r - 2.).abs() < 1e-9));
        assert!((ink.opacity - 128. / 255.).abs() < 1e-12);
    }

    #[test]
    fn short_move_skip_drops_the_first_move() {
        let ink = prepare(&stroke(&[(0., 0.), (4., 0.)], None, 1)).unwrap();
        assert_eq!(ink.sample_ends, vec![1, 1]);
        assert_eq!(ink.points.len(), 1);
        assert!(ink.points[0].x.abs() < 1e-6);
    }

    #[test]
    fn pressure_and_v2_settings_do_not_change_the_stamps() {
        let mut light = stroke(&[(0., 0.), (4., 0.)], Some("2;"), 2);
        light.pressures = vec![0.05, 0.95];
        let heavy = stroke(&[(0., 0.), (4., 0.)], Some("18;"), 2);
        let light = prepare(&light).unwrap();
        let heavy = prepare(&heavy).unwrap();
        assert_eq!(light.points, heavy.points);
        assert_eq!(light.radii, heavy.radii);
    }

    #[test]
    fn other_pens_and_unresolved_settings_stay_approximate() {
        let mut other = stroke(&[(0., 0.), (4., 0.)], Some("2;"), 2);
        other.rendering.as_mut().unwrap().pen_name =
            Some("com.samsung.android.sdk.pen.pen.preload.Marker".into());
        assert!(prepare(&other).is_none());
        assert!(prepare(&stroke(&[(0., 0.), (4., 0.)], Some(""), 2)).is_none());
        assert!(prepare(&stroke(&[(0., 0.), (4., 0.)], Some("-1;"), 2)).is_none());
        let mut eraser = stroke(&[(0., 0.), (4., 0.)], Some("2;"), 2);
        eraser.rendering.as_mut().unwrap().properties.eraser = true;
        assert!(prepare(&eraser).is_none());
        assert!(prepare(&stroke(&[(0., 0.), (400_001., 0.)], Some("2;"), 2)).is_none());
    }
}
