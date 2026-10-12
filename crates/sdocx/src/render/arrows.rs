use super::vector::{
    Circle, ClipPath, Data, Definitions, Group, LineCap, LineJoin, Paint, Path, Rectangle, Scene,
    Styled, Transform, decimal,
};
use crate::{
    NativeLine,
    shape::{NativePathCommand, visit_path},
};

struct Arrow {
    kind: u8,
    length: f32,
    endpoint: [f32; 2],
    angle: f32,
    cleanup: Option<[f32; 2]>,
}

pub(super) struct Plan {
    arrows: [Option<Arrow>; 2],
    endpoints: [[f32; 2]; 2],
    width: f32,
    cap: LineCap,
    join: LineJoin,
}

impl Plan {
    pub fn for_line(line: &NativeLine) -> Result<Option<Self>, ()> {
        let style = &line.style;
        if style.begin_arrow[0] == 0 && style.end_arrow[0] == 0 {
            return Ok(None);
        }
        if line.line_type != 0 || !style.has_supported_arrow_settings() || style.compound != 0 {
            return Err(());
        }
        let mut endpoints = [line.begin, line.end];
        if !line.path_data.is_empty() {
            let mut count = 0;
            let mut straight = true;
            let (size, supported) = visit_path(&line.path_data, |command| {
                match (count, command) {
                    (0, NativePathCommand::Move(point)) | (1, NativePathCommand::Line(point)) => {
                        endpoints[count] = point
                    }
                    _ => straight = false,
                }
                count += 1;
            })
            .map_err(|_| ())?;
            if !straight || count != 2 || !supported || size != line.path_data.len() {
                return Err(());
            }
        }
        if endpoints
            .iter()
            .flatten()
            .any(|v| !v.is_finite() || f64::from(*v as f32) != *v)
        {
            return Err(());
        }
        let endpoints = endpoints.map(|point| point.map(|v| v as f32));
        let rounding_step = f64::from(style.width) * 0.01;
        if endpoints
            .iter()
            .flatten()
            .any(|value| f64::from(value.abs()) * f64::from(f32::EPSILON) > rounding_step)
        {
            return Err(());
        }
        let dx = endpoints[1][0] - endpoints[0][0];
        let dy = endpoints[1][1] - endpoints[0][1];
        let distance = dx.mul_add(dx, dy * dy).sqrt();
        let width = style.width;
        if !distance.is_finite() || distance == 0.0 || f64::from(distance) < 1.7 * f64::from(width)
        {
            return Err(());
        }
        let lengths = style.arrow_lengths().ok_or(())?;
        let mut arrows = [None, None];
        for (index, [kind, size]) in [style.begin_arrow, style.end_arrow].into_iter().enumerate() {
            if kind == 0 {
                continue;
            }
            let length = lengths[usize::from(size)] as f32;
            let radians =
                (endpoints[0][1] - endpoints[1][1]).atan2(endpoints[0][0] - endpoints[1][0]);
            let angle = (f64::from(radians * 360.0) / std::f64::consts::TAU) as f32
                + if index == 0 { 90.0 } else { -90.0 };
            let cleanup = if kind <= 3 {
                let height = (width + if kind == 2 { width * 1.2 } else { length * 0.5 }).trunc();
                let cut_width = width.trunc() * 2.0;
                if cut_width <= 0.0 || height <= 0.0 {
                    return Err(());
                }
                Some([cut_width, height])
            } else {
                None
            };
            arrows[index] = Some(Arrow {
                kind,
                length,
                endpoint: endpoints[index],
                angle,
                cleanup,
            });
        }
        let cleanup_length: f32 = arrows
            .iter()
            .flatten()
            .filter_map(|arrow| arrow.cleanup.map(|size| size[1]))
            .sum();
        if distance <= cleanup_length + 2.0 * width {
            return Err(());
        }
        Ok(Some(Self {
            arrows,
            endpoints,
            width,
            cap: match style.cap {
                1 => LineCap::Round,
                2 => LineCap::Square,
                _ => LineCap::Butt,
            },
            join: match style.join {
                1 => LineJoin::Round,
                2 => LineJoin::Bevel,
                _ => LineJoin::Miter,
            },
        }))
    }

    pub fn render(&self, svg: &mut Scene, paint: Paint, opacity: f64, body: Group) {
        let holes: Vec<_> = self
            .arrows
            .iter()
            .flatten()
            .filter_map(|arrow| {
                arrow.cleanup.map(|size| {
                    let w = f64::from(self.width);
                    let [width, height] = size.map(f64::from);
                    let (sin, cos) = f64::from(arrow.angle).to_radians().sin_cos();
                    [
                        [-w, -w],
                        [width - w, -w],
                        [width - w, height - w],
                        [-w, height - w],
                    ]
                    .map(|[x, y]| {
                        [
                            f64::from(arrow.endpoint[0]) + x * cos - y * sin,
                            f64::from(arrow.endpoint[1]) + x * sin + y * cos,
                        ]
                    })
                })
            })
            .collect();
        if holes.is_empty() {
            svg.push(body);
        } else {
            let margin = f64::from(self.width) * 2.0;
            let points: Vec<_> = self
                .endpoints
                .iter()
                .map(|point| point.map(f64::from))
                .chain(holes.iter().flatten().copied())
                .collect();
            let bounds: [f64; 4] = [
                points.iter().map(|p| p[0]).fold(f64::INFINITY, f64::min) - margin,
                points.iter().map(|p| p[1]).fold(f64::INFINITY, f64::min) - margin,
                points
                    .iter()
                    .map(|p| p[0])
                    .fold(f64::NEG_INFINITY, f64::max)
                    + margin,
                points
                    .iter()
                    .map(|p| p[1])
                    .fold(f64::NEG_INFINITY, f64::max)
                    + margin,
            ];
            let mut data = Data::new()
                .move_to((bounds[0], bounds[1]))
                .line_to((bounds[2], bounds[1]))
                .line_to((bounds[2], bounds[3]))
                .line_to((bounds[0], bounds[3]))
                .close();
            for hole in holes {
                data = data.move_to((hole[0][0], hole[0][1]));
                for point in &hole[1..] {
                    data = data.line_to((point[0], point[1]));
                }
                data = data.close();
            }
            let clip = svg.definition();
            svg.push(
                Definitions::new()
                    .add(ClipPath::new(&clip).add(Path::new().even_odd_clip().data(data))),
            );
            svg.push(body.clipped(&clip));
        }
        for arrow in self.arrows.iter().flatten() {
            let length = arrow.length;
            let half = length * 0.5;
            let translated = Group::new().transformed(Transform::translate_unrounded(
                f64::from(arrow.endpoint[0]),
                f64::from(arrow.endpoint[1]),
            ));
            svg.scope(translated, |svg| {
                svg.scope(
                    Group::new().transformed(Transform::rotate_origin(f64::from(arrow.angle))),
                    |svg| {
                        let fill = Group::new()
                            .fill(paint)
                            .fill_opacity(decimal(opacity, 4))
                            .stroke(Paint::None);
                        match arrow.kind {
                            1..=3 => {
                                let data = match arrow.kind {
                                    1 => Data::new()
                                        .move_to((-half, length))
                                        .line_to((0.0, 0.0))
                                        .line_to((half, length))
                                        .close(),
                                    2 => Data::new()
                                        .move_to((-half, length))
                                        .line_to((0.0, self.width * f32::from_bits(0x3f8f1aa0)))
                                        .line_to((half, length)),
                                    _ => Data::new()
                                        .move_to((0.0, half))
                                        .line_to((-half, length))
                                        .line_to((0.0, 0.0))
                                        .line_to((half, length))
                                        .line_to((0.0, half)),
                                };
                                if arrow.kind == 2 {
                                    svg.push(
                                        Group::new()
                                            .fill(Paint::None)
                                            .stroke(paint)
                                            .stroke_opacity(decimal(opacity, 4))
                                            .stroke_width(self.width)
                                            .line_cap(self.cap)
                                            .line_join(self.join)
                                            .add(Path::new().data(data)),
                                    );
                                } else {
                                    svg.push(fill.add(Path::new().data(data)));
                                }
                            }
                            4 => svg.push(
                                fill.transformed(Transform::rotate_origin(45.0)).add(
                                    Rectangle::new()
                                        .x(-half)
                                        .y(-half)
                                        .width(length)
                                        .height(length),
                                ),
                            ),
                            5 => svg.push(fill.add(Circle::new().cx(0).cy(0).r(half))),
                            _ => unreachable!(),
                        }
                    },
                );
            });
        }
    }
}
