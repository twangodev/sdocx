//! Fountain V14/V16 coverage using only SVG vector primitives.
use super::vector::{
    Blend, Circle, ColorMatrix, ColorValue, Definitions, Filter, Gradient, Group, LinearGradient,
    Mask, Paint, RadialGradient, ReplayPart, Scene, Stop, Styled, SvgFilter, SvgMask, Transform,
};
use crate::PreparedStroke;
use std::collections::HashMap;

pub(super) fn render(
    svg: &mut Scene,
    paint: &PreparedStroke<'_>,
    replay: bool,
    slice: Option<crate::render::PageSlice>,
) -> bool {
    let culling_slice = slice.filter(|slice| slice.fountain_paint_culling_enabled());
    let Some(plan) = PaintPlan::prepare(paint, culling_slice.is_some()) else {
        return false;
    };
    if culling_slice.is_some_and(|slice| plan.outside(slice)) {
        return true;
    }
    plan.emit(svg, paint, replay);
    true
}

enum PaintPlan {
    V14(V14Plan),
    V16(V16Plan),
}

struct V14Plan {
    color: crate::Color,
    region: (f64, f64, f64, f64),
    stamps: Vec<V14Stamp>,
    y_bounds: Option<(f64, f64)>,
    output_scale: Option<f32>,
}

#[derive(Clone, Copy)]
struct V14Stamp {
    point: crate::Point,
    dx: f64,
    dy: f64,
    width_alpha: f32,
    radial_inner: Option<f32>,
}

struct V16Plan {
    color: crate::Color,
    stamps: Vec<V16Stamp>,
    y_bounds: Option<(f64, f64)>,
    #[cfg(feature = "pdf")]
    inverse_scale: f32,
    #[cfg(feature = "pdf")]
    bounds: Option<crate::BoundingBox>,
}

impl PaintPlan {
    fn prepare(paint: &PreparedStroke<'_>, collect_bounds: bool) -> Option<Self> {
        if paint.profile == Some("FountainPen")
            && paint.dot_radii.is_some()
            && paint.dot_directions.is_none()
        {
            Some(Self::V16(V16Plan::prepare(paint)?))
        } else {
            Some(Self::V14(V14Plan::prepare(paint, collect_bounds)?))
        }
    }

    fn outside(&self, slice: crate::render::PageSlice) -> bool {
        let bounds = match self {
            Self::V14(plan) => plan.y_bounds,
            Self::V16(plan) => plan.y_bounds,
        };
        let Some((min_y, max_y)) = bounds else {
            return false;
        };
        let slice_min = f64::from(slice.y());
        let slice_max = slice_min + f64::from(slice.height());
        max_y < slice_min || min_y > slice_max
    }

    fn emit(self, svg: &mut Scene, paint: &PreparedStroke<'_>, replay: bool) {
        match self {
            Self::V14(plan) => plan.emit(svg, paint, replay),
            Self::V16(plan) => plan.emit(svg, paint, replay),
        }
    }
}

impl V14Plan {
    fn prepare(paint: &PreparedStroke<'_>, collect_bounds: bool) -> Option<Self> {
        if let Some(scale) = paint.fountain_output_scale
            && valid_v14_scale(scale)
            && let Some(plan) = Self::prepare_scaled(paint, collect_bounds, scale)
        {
            return Some(plan);
        }
        Self::prepare_canonical(paint, collect_bounds)
    }

    fn prepare_canonical(paint: &PreparedStroke<'_>, collect_bounds: bool) -> Option<Self> {
        let (Some(directions), Some(radii), Some(bounds)) =
            (&paint.dot_directions, &paint.dot_radii, paint.bounds)
        else {
            return None;
        };
        let color = parse_rgb(&paint.color)?;
        let region = (
            bounds.x_min - 1.,
            bounds.y_min - 1.,
            bounds.x_max - bounds.x_min + 2.,
            bounds.y_max - bounds.y_min + 2.,
        );
        let mut stamps =
            Vec::with_capacity(paint.points.len().min(radii.len()).min(directions.len()));
        let mut y_bounds: Option<(f64, f64)> = None;
        let mut bounds_known = region.0.is_finite()
            && region.1.is_finite()
            && region.2.is_finite()
            && region.3.is_finite()
            && region.2 > 0.0
            && region.3 > 0.0;
        for ((point, radius), direction) in paint.points.iter().zip(radii).zip(directions) {
            let dx = direction.x * radius;
            let dy = direction.y * radius;
            stamps.push(V14Stamp {
                point: *point,
                dx,
                dy,
                width_alpha: 1.0,
                radial_inner: None,
            });
            // Full-page output does not need serialized-transform culling bounds.
            if !collect_bounds {
                continue;
            }
            let (Some(_tx), Some(ty)) = (
                svg_number(point.x, 4).and_then(f32_number),
                svg_number(point.y, 4).and_then(f32_number),
            ) else {
                bounds_known = false;
                continue;
            };
            let (Some(a), Some(b), Some(c), Some(d)) = (
                svg_number(dx, 7).and_then(f32_number),
                svg_number(dy, 7).and_then(f32_number),
                svg_number(-dy, 7).and_then(f32_number),
                svg_number(dx, 7).and_then(f32_number),
            ) else {
                bounds_known = false;
                continue;
            };
            let extent = next_up(a.hypot(c).max(b.hypot(d)));
            let low = next_down(next_down(ty) - extent);
            let high = next_up(next_up(ty) + extent);
            y_bounds = Some(match y_bounds {
                Some((min_y, max_y)) => (min_y.min(f64::from(low)), max_y.max(f64::from(high))),
                None => (f64::from(low), f64::from(high)),
            });
        }
        if !bounds_known {
            y_bounds = None;
        }
        Some(Self {
            color,
            region,
            stamps,
            y_bounds,
            output_scale: None,
        })
    }

    fn prepare_scaled(
        paint: &PreparedStroke<'_>,
        collect_bounds: bool,
        output_scale: f32,
    ) -> Option<Self> {
        const MAX_STAMPS: usize = 8_192;
        let (Some(directions), Some(radii), Some(bounds)) =
            (&paint.dot_directions, &paint.dot_radii, paint.bounds)
        else {
            return None;
        };
        let count = paint.points.len();
        if count == 0
            || count > MAX_STAMPS
            || radii.len() != count
            || directions.len() != count
            || !paint.opacity.is_finite()
            || !(0.0..=1.0).contains(&paint.opacity)
        {
            return None;
        }
        let color = parse_rgb(&paint.color)?;
        let inverse_scale = 1.0_f32 / output_scale;
        if !inverse_scale.is_finite() || inverse_scale <= 0.0 {
            return None;
        }
        let padding = 1.0_f64.max(f64::from(inverse_scale));
        let region = (
            bounds.x_min - padding,
            bounds.y_min - padding,
            bounds.x_max - bounds.x_min + padding * 2.0,
            bounds.y_max - bounds.y_min + padding * 2.0,
        );
        let mut stamps = Vec::with_capacity(count);
        let mut y_bounds: Option<(f64, f64)> = None;
        let mut bounds_known = region.0.is_finite()
            && region.1.is_finite()
            && region.2.is_finite()
            && region.3.is_finite()
            && region.2 > 0.0
            && region.3 > 0.0;
        for ((point, radius), direction) in paint.points.iter().zip(radii).zip(directions) {
            let [
                x,
                y,
                extent_x,
                extent_y,
                direction_x,
                direction_y,
                _transparency,
                radial_inner,
                width_alpha,
            ] = v14_native_addpoint_record(*point, *radius, *direction, output_scale)?;
            let dx = direction_x * extent_x;
            let dy = direction_y * extent_y;
            if !dx.is_finite() || !dy.is_finite() {
                return None;
            }
            let point = crate::Point {
                x: f64::from(x),
                y: f64::from(y),
            };
            stamps.push(V14Stamp {
                point,
                dx: f64::from(dx),
                dy: f64::from(dy),
                width_alpha,
                radial_inner: Some(radial_inner),
            });

            if !collect_bounds {
                continue;
            }
            let (Some(ty), Some(a), Some(b)) = (
                f32_number(f64::from(y)),
                f32_number(f64::from(dx)),
                f32_number(f64::from(dy)),
            ) else {
                bounds_known = false;
                continue;
            };
            let extent = next_up(a.hypot(b));
            let low = next_down(next_down(ty) - extent);
            let high = next_up(next_up(ty) + extent);
            y_bounds = Some(match y_bounds {
                Some((min_y, max_y)) => (min_y.min(f64::from(low)), max_y.max(f64::from(high))),
                None => (f64::from(low), f64::from(high)),
            });
        }
        if !bounds_known {
            y_bounds = None;
        }
        Some(Self {
            color,
            region,
            stamps,
            y_bounds,
            output_scale: Some(output_scale),
        })
    }

    fn emit(self, svg: &mut Scene, paint: &PreparedStroke<'_>, replay: bool) {
        let (x, y, width, height) = self.region;
        let gradient = svg.definition::<Gradient>();
        let filter = svg.definition::<Filter>();
        let ramp = LinearGradient::new(&gradient)
            .x1(-1)
            .x2(1)
            .y1(0)
            .y2(0)
            .add(Stop::new(0, ColorValue::gray(0.07)))
            .add(Stop::new(0.25, ColorValue::WHITE))
            .add(Stop::new(0.75, ColorValue::WHITE))
            .add(Stop::new(1, ColorValue::gray(0.07)));
        let matrix = ColorMatrix::matrix([
            0.,
            0.,
            0.,
            0.,
            f64::from(self.color.r) / 255.,
            0.,
            0.,
            0.,
            0.,
            f64::from(self.color.g) / 255.,
            0.,
            0.,
            0.,
            0.,
            f64::from(self.color.b) / 255.,
            1.,
            0.,
            0.,
            0.,
            0.,
        ]);
        let mut definitions = Definitions::new().add(ramp).add(
            SvgFilter::new(&filter)
                .region(x, y, width, height)
                .add(matrix),
        );
        let mut radial_masks = HashMap::<u32, super::vector::Definition<Mask>>::new();
        if self.output_scale.is_some() {
            for stamp in &self.stamps {
                let Some(inner) = stamp.radial_inner else {
                    continue;
                };
                let key = inner.to_bits();
                if radial_masks.contains_key(&key) {
                    continue;
                }
                let radial = svg.definition::<Gradient>();
                let mask = svg.definition::<Mask>();
                let stop = (f64::from(inner) * 2.0).clamp(0.0, 1.0);
                let mut radial_gradient = RadialGradient::object_bounding_box(&radial)
                    .cx(0.5)
                    .cy(0.5)
                    .r(0.5)
                    .add(Stop::new(0.0, ColorValue::WHITE));
                if stop > 0.0 {
                    radial_gradient = radial_gradient.add(Stop::new(stop, ColorValue::WHITE));
                }
                radial_gradient = radial_gradient.add(Stop::new(1.0, ColorValue::BLACK));
                let mask_definition = SvgMask::luminance_object_bounding_box(&mask)
                    .x(-0.1)
                    .y(-0.1)
                    .width(1.2)
                    .height(1.2)
                    .add(
                        super::vector::Rectangle::new()
                            .x(0.0)
                            .y(0.0)
                            .width(1.0)
                            .height(1.0)
                            .fill(Paint::Gradient(radial)),
                    );
                definitions = definitions.add(radial_gradient).add(mask_definition);
                radial_masks.insert(key, mask);
            }
        }
        svg.scope(definitions, |_| {});
        svg.scope(
            Group::new().filtered(&filter).opacity(paint.opacity),
            |svg| {
                svg.scope(Group::new().isolated(), |svg| {
                    svg.push(
                        super::vector::Rectangle::new()
                            .x(x)
                            .y(y)
                            .width(width)
                            .height(height)
                            .fill(Paint::BLACK),
                    );
                    for (index, stamp) in self.stamps.iter().enumerate() {
                        let transform = Transform::matrix(
                            [
                                stamp.dx,
                                stamp.dy,
                                -stamp.dy,
                                stamp.dx,
                                stamp.point.x,
                                stamp.point.y,
                            ],
                            7,
                            4,
                        );
                        let mut circle = Circle::new().r(1).fill(Paint::Gradient(gradient));
                        if let Some(inner) = stamp.radial_inner {
                            let mask = radial_masks
                                .get(&inner.to_bits())
                                .expect("scaled V14 stamp has a radial mask");
                            circle = circle.masked(mask);
                        }
                        if self.output_scale.is_some() {
                            // Preserve GL_MAX after applying the per-stamp
                            // radial and width coverage. A transparent
                            // semitransparent source under SVG Lighten still
                            // source-over accumulates alpha at overlaps. Put
                            // each stamp on a bounded opaque-black local layer
                            // first, then lighten-composite that scalar field
                            // into the stroke's opaque-black accumulation.
                            let stamp_field = Group::new()
                                .isolated()
                                .transformed(transform)
                                .add(
                                    super::vector::Rectangle::new()
                                        .x(-1.1)
                                        .y(-1.1)
                                        .width(2.2)
                                        .height(2.2)
                                        .fill(Paint::BLACK),
                                )
                                .add(
                                    Group::new()
                                        .opacity(f64::from(stamp.width_alpha))
                                        .add(circle),
                                );
                            svg.push(
                                Group::new()
                                    .blend(Blend::Lighten)
                                    .replay_part(replay.then_some(ReplayPart(index + 1)))
                                    .add(stamp_field),
                            );
                        } else {
                            svg.push(
                                Group::new()
                                    .blend(Blend::Lighten)
                                    .replay_part(replay.then_some(ReplayPart(index + 1)))
                                    .add(circle.transformed(transform)),
                            );
                        }
                    }
                });
            },
        );
    }
}

/// The nine-f32 RTV4::AddPoint record, in native field order. Keep arithmetic
/// in f32 and in the same sequence as the captured native constructor.
fn v14_native_addpoint_record(
    point: crate::Point,
    radius: f64,
    direction: crate::Point,
    output_scale: f32,
) -> Option<[f32; 9]> {
    let (x, y, radius, direction_x, direction_y) = (
        point.x as f32,
        point.y as f32,
        radius as f32,
        direction.x as f32,
        direction.y as f32,
    );
    if !output_scale.is_finite()
        || output_scale <= 0.0
        || !x.is_finite()
        || !y.is_finite()
        || !radius.is_finite()
        || radius <= 0.0
        || !direction_x.is_finite()
        || !direction_y.is_finite()
    {
        return None;
    }
    let inverse_scale = 1.0_f32 / output_scale;
    if !inverse_scale.is_finite() || inverse_scale <= 0.0 {
        return None;
    }
    let rx = radius / inverse_scale;
    let ry = radius / inverse_scale;
    let minimum = rx.min(ry);
    if !minimum.is_finite() || minimum <= 0.0 {
        return None;
    }
    let factor = if minimum < 0.5 { 0.5 / minimum } else { 1.0 };
    let width_alpha = 1.0 / factor;
    let extent_device_x = rx * factor.max(1.0) + 0.5;
    let extent_device_y = ry * factor.max(1.0) + 0.5;
    let maximum = extent_device_x.max(extent_device_y);
    let radial_inner = 0.5 * (maximum - 1.0) / maximum;
    let extent_x = extent_device_x * inverse_scale;
    let extent_y = extent_device_y * inverse_scale;
    if !width_alpha.is_finite()
        || !(0.0..=1.0).contains(&width_alpha)
        || !extent_x.is_finite()
        || !extent_y.is_finite()
        || extent_x <= 0.0
        || extent_y <= 0.0
        || !radial_inner.is_finite()
        || !(0.0..0.5).contains(&radial_inner)
    {
        return None;
    }
    Some([
        x,
        y,
        extent_x,
        extent_y,
        direction_x,
        direction_y,
        1.0,
        radial_inner,
        width_alpha,
    ])
}

fn valid_v14_scale(scale: f32) -> bool {
    if !scale.is_finite() || scale <= 0.0 || scale > 1_000_000.0 {
        return false;
    }
    let inverse = 1.0_f32 / scale;
    inverse.is_finite() && inverse > 0.0 && inverse <= 1_000_000.0
}

fn svg_number(value: f64, places: usize) -> Option<f64> {
    format!("{value:.places$}").parse().ok()
}

fn f32_number(value: f64) -> Option<f32> {
    let value = value as f32;
    value.is_finite().then_some(value)
}

fn next_up(value: f32) -> f32 {
    if value.is_nan() || value == f32::INFINITY {
        value
    } else if value == 0.0 {
        f32::from_bits(1)
    } else if value > 0.0 {
        f32::from_bits(value.to_bits() + 1)
    } else {
        f32::from_bits(value.to_bits() - 1)
    }
}

fn next_down(value: f32) -> f32 {
    -next_up(-value)
}

#[derive(Clone, Copy)]
struct V16Stamp {
    x: f32,
    y: f32,
    extent_x: f32,
    extent_y: f32,
    gradient_key: V16GradientKey,
}

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
struct V16GradientKey {
    alpha: u32,
    inner: u32,
    rgb: Option<[u32; 3]>,
}

/// Prepare the V16 RTV5 source geometry used by both canonical SVG gradients
/// and the bounded retained-PDF full-coverage circle route.
impl V16Plan {
    fn prepare(paint: &PreparedStroke<'_>) -> Option<Self> {
        let (Some(radii), Some(_bounds)) = (&paint.dot_radii, paint.bounds) else {
            return None;
        };
        if radii.len() != paint.points.len()
            || radii.is_empty()
            || !paint.opacity.is_finite()
            || !(0.0..=1.0).contains(&paint.opacity)
        {
            return None;
        }
        let color = parse_rgb(&paint.color)?;
        let inverse_scale = if let Some(scale) = paint.fountain_output_scale {
            if !scale.is_finite() || scale <= 0.0 {
                return None;
            }
            let inverse = 1.0_f32 / scale;
            if !inverse.is_finite() || inverse <= 0.0 {
                return None;
            }
            inverse
        } else {
            1.0_f32
        };
        let stamp_rgb = paint.fountain_v17_rgb.as_deref();
        if stamp_rgb.is_some_and(|colors| {
            colors.len() != paint.points.len()
                || colors.iter().any(|rgb| {
                    rgb.iter()
                        .any(|value| !value.is_finite() || !(0.0..=1.0).contains(value))
                })
        }) {
            return None;
        }
        if stamp_rgb.is_some()
            && (radii.len() > 32_000
                || radii
                    .len()
                    .checked_mul(1024)
                    .and_then(|bytes| bytes.checked_add(4096))
                    .is_none_or(|bytes| bytes > 32 * 1024 * 1024))
        {
            return None;
        }

        // RTV5::AddPoint float32 arithmetic at sx=sy=1, in native instruction
        // order: rx/ry, min and factor, width compensation, extents, then inner.
        // A gradient is shared only when both shader coverage parameters have
        // identical f32 bits.
        let mut stamps = Vec::with_capacity(radii.len());
        let mut y_bounds: Option<(f64, f64)> = None;
        #[cfg(feature = "pdf")]
        let mut field_bounds: Option<crate::BoundingBox> = None;
        for (index, (point, radius)) in paint.points.iter().zip(radii).enumerate() {
            let radius = *radius as f32;
            if !radius.is_finite() || radius <= 0. {
                return None;
            }
            let rx = radius / inverse_scale;
            let ry = radius / inverse_scale;
            let minimum = rx.min(ry);
            let factor = if minimum < 0.5 { 0.5 / minimum } else { 1.0 };
            let alpha = 1.0 / factor;
            let extent_device_x = rx * factor.max(1.0) + 0.5;
            let extent_device_y = ry * factor.max(1.0) + 0.5;
            let maximum = extent_device_x.max(extent_device_y);
            let inner = 0.5 * (maximum - 1.0) / maximum;
            let extent_x = extent_device_x * inverse_scale;
            let extent_y = extent_device_y * inverse_scale;
            let x = point.x as f32;
            let y = point.y as f32;
            if !x.is_finite()
                || !y.is_finite()
                || !extent_x.is_finite()
                || !extent_y.is_finite()
                || !inner.is_finite()
                || !alpha.is_finite()
                || !(0.0..=1.0).contains(&alpha)
                || extent_x <= 0.0
                || extent_y <= 0.0
                || !maximum.is_finite()
                || maximum < 1.0
            {
                return None;
            }
            let key = V16GradientKey {
                alpha: alpha.to_bits(),
                inner: inner.to_bits(),
                rgb: stamp_rgb.map(|colors| colors[index].map(f32::to_bits)),
            };
            y_bounds = Some(match y_bounds {
                Some((min_y, max_y)) => (
                    min_y.min(f64::from(next_down(y - extent_y))),
                    max_y.max(f64::from(next_up(y + extent_y))),
                ),
                None => (
                    f64::from(next_down(y - extent_y)),
                    f64::from(next_up(y + extent_y)),
                ),
            });
            #[cfg(feature = "pdf")]
            {
                let x_min = f64::from(x) - f64::from(extent_x);
                let x_max = f64::from(x) + f64::from(extent_x);
                let y_min = f64::from(y) - f64::from(extent_y);
                let y_max = f64::from(y) + f64::from(extent_y);
                field_bounds = Some(match field_bounds {
                    Some(bounds) => crate::BoundingBox {
                        x_min: bounds.x_min.min(x_min),
                        y_min: bounds.y_min.min(y_min),
                        x_max: bounds.x_max.max(x_max),
                        y_max: bounds.y_max.max(y_max),
                    },
                    None => crate::BoundingBox {
                        x_min,
                        y_min,
                        x_max,
                        y_max,
                    },
                });
            }
            stamps.push(V16Stamp {
                x,
                y,
                extent_x,
                extent_y,
                gradient_key: key,
            });
        }

        Some(Self {
            color,
            stamps,
            y_bounds,
            #[cfg(feature = "pdf")]
            inverse_scale,
            #[cfg(feature = "pdf")]
            bounds: field_bounds,
        })
    }

    #[cfg(feature = "pdf")]
    fn radial_circle_records(&self) -> Option<Vec<[f32; 9]>> {
        if self.stamps.is_empty() || self.stamps.len() > 4096 {
            return None;
        }
        self.stamps
            .iter()
            .map(|stamp| {
                if stamp.extent_x.to_bits() != stamp.extent_y.to_bits() {
                    return None;
                }
                let coverage = f32::from_bits(stamp.gradient_key.alpha);
                if coverage != 1.0 || stamp.extent_x < self.inverse_scale {
                    return None;
                }
                let [red, green, blue] = stamp.gradient_key.rgb?;
                let radius = stamp.extent_x - 0.5 * self.inverse_scale;
                if !radius.is_finite() || radius < 0.0 {
                    return None;
                }
                Some([
                    stamp.x,
                    stamp.y,
                    radius,
                    radius,
                    0.0,
                    1.0,
                    f32::from_bits(red),
                    f32::from_bits(green),
                    f32::from_bits(blue),
                ])
            })
            .collect()
    }

    fn emit(self, svg: &mut Scene, paint: &PreparedStroke<'_>, replay: bool) {
        #[cfg(feature = "pdf")]
        let native_ink_id = if svg.retains_text() {
            self.radial_circle_records()
                .zip(self.bounds)
                .and_then(|(records, bounds)| {
                    svg.native_ink()
                        .register_radial_circles(&records, bounds, paint.opacity as f32)
                        .ok()
                })
        } else {
            None
        };
        let mut unique = Vec::<(V16GradientKey, super::vector::Definition<Gradient>)>::new();
        let mut indices = HashMap::<V16GradientKey, usize>::new();
        for stamp in &self.stamps {
            let key = stamp.gradient_key;
            indices.entry(key).or_insert_with(|| {
                let index = unique.len();
                unique.push((key, svg.definition::<Gradient>()));
                index
            });
        }
        let mut definitions = Definitions::new();
        for (key, id) in &unique {
            let alpha = f32::from_bits(key.alpha);
            let inner = f32::from_bits(key.inner);
            let source_alpha = alpha * paint.opacity as f32;
            let color = if let Some(rgb) = key.rgb {
                ColorValue::rgb_components(
                    f64::from(f32::from_bits(rgb[0])),
                    f64::from(f32::from_bits(rgb[1])),
                    f64::from(f32::from_bits(rgb[2])),
                )
                .expect("prepared Fountain rainbow RGB is finite and normalized")
            } else {
                ColorValue::Rgb(self.color)
            };
            let mut gradient = RadialGradient::new(id)
                .cx(0)
                .cy(0)
                .r(1)
                .add(Stop::new(0, color).opacity(source_alpha));
            if inner > 0. {
                gradient = gradient.add(Stop::new(inner + inner, color).opacity(source_alpha));
            }
            definitions = definitions.add(gradient.add(Stop::new(1, color).opacity(0)));
        }
        svg.scope(definitions, |_| {});
        let outer = Group::new();
        #[cfg(feature = "pdf")]
        let outer = match native_ink_id {
            Some(id) => outer.native_ink_id(id),
            None => outer,
        };
        svg.scope(outer, |svg| {
            // RTV5 submits each instance with source-over blending. Keep the
            // per-instance ARGB alpha in its radial field so overlaps accumulate
            // in draw order; applying it to this parent group would apply it once
            // after the instances have already combined.
            for (index, stamp) in self.stamps.iter().enumerate() {
                let Some(&gradient_index) = indices.get(&stamp.gradient_key) else {
                    continue;
                };
                let Some((_, gradient)) = unique.get(gradient_index) else {
                    continue;
                };
                let transform = Transform::matrix_unrounded([
                    stamp.extent_x as f64,
                    0.,
                    0.,
                    stamp.extent_y as f64,
                    stamp.x as f64,
                    stamp.y as f64,
                ]);
                svg.push(
                    Group::new()
                        .replay_part(replay.then_some(ReplayPart(index + 1)))
                        .transformed(transform)
                        .add(Circle::new().r(1).fill(Paint::Gradient(*gradient))),
                );
            }
        });
    }
}

fn parse_rgb(value: &str) -> Option<crate::Color> {
    let hex = value.strip_prefix('#')?;
    if hex.len() != 6 {
        return None;
    }
    Some(crate::Color {
        r: u8::from_str_radix(&hex[0..2], 16).ok()?,
        g: u8::from_str_radix(&hex[2..4], 16).ok()?,
        b: u8::from_str_radix(&hex[4..6], 16).ok()?,
    })
}

#[cfg(all(test, feature = "serde"))]
mod tests {
    use super::super::vector::Svg;
    use super::*;
    use crate::{BoundingBox, Point, Stroke};
    use std::borrow::Cow;

    fn shaded_stamp(direction: Point, count: usize) -> String {
        shaded_stamp_with_color(direction, count, "#000000", false)
    }

    fn shaded_stamp_with_color(
        direction: Point,
        count: usize,
        color: &str,
        replay: bool,
    ) -> String {
        let reference: serde_json::Value =
            serde_json::from_str(include_str!("../../../../conformance/fountain-v14.json"))
                .unwrap();
        let stroke: Stroke = serde_json::from_value(reference["stroke"].clone()).unwrap();
        let mut paint = crate::prepare_stroke(&stroke, false);
        paint.points = Cow::Owned(vec![Point { x: 32., y: 32. }; count]);
        paint.dot_radii = Some(vec![20.; count]);
        paint.dot_directions = Some(vec![direction; count]);
        paint.bounds = Some(BoundingBox {
            x_min: 12.,
            y_min: 12.,
            x_max: 52.,
            y_max: 52.,
        });
        paint.color = color.into();
        paint.opacity = 0.6;
        let mut svg = Scene::new(Svg::new().width(64).height(64));
        assert!(render(&mut svg, &paint, replay, None));
        svg.finish()
    }

    fn preview(svg: &str) -> resvg::tiny_skia::Pixmap {
        let tree = resvg::usvg::Tree::from_str(svg, &resvg::usvg::Options::default()).unwrap();
        let mut pixmap = resvg::tiny_skia::Pixmap::new(64, 64).unwrap();
        resvg::render(
            &tree,
            resvg::tiny_skia::Transform::identity(),
            &mut pixmap.as_mut(),
        );
        pixmap
    }

    #[derive(Default, Debug)]
    struct V14AlphaComparison {
        pixels: usize,
        union_pixels: usize,
        native_support: usize,
        rendered_support: usize,
        support_xor: usize,
        absolute_error_sum: u64,
        max_absolute_error: u8,
    }

    impl V14AlphaComparison {
        fn add_tile(&mut self, native: &[u8], rendered: &[u8]) {
            assert_eq!(native.len(), 64 * 64);
            assert_eq!(rendered.len(), native.len());
            self.pixels += native.len();
            for (&native, &rendered) in native.iter().zip(rendered) {
                let native_covered = native != 0;
                let rendered_covered = rendered != 0;
                self.native_support += native_covered as usize;
                self.rendered_support += rendered_covered as usize;
                self.support_xor += (native_covered != rendered_covered) as usize;
                self.union_pixels += (native_covered || rendered_covered) as usize;
                let error = native.abs_diff(rendered);
                self.absolute_error_sum += u64::from(error);
                self.max_absolute_error = self.max_absolute_error.max(error);
            }
        }

        fn mae_over_union(&self) -> f64 {
            self.absolute_error_sum as f64 / self.union_pixels.max(1) as f64
        }
    }

    fn v14_raster_tile(
        fixture_stroke: &Stroke,
        point: Point,
        radius: f64,
        direction: Point,
        output_scale: Option<f32>,
        raster_scale: f32,
    ) -> Vec<u8> {
        v14_raster_stamps(
            fixture_stroke,
            &[(point, radius, direction)],
            output_scale,
            raster_scale,
        )
    }

    fn v14_raster_stamps(
        fixture_stroke: &Stroke,
        stamps: &[(Point, f64, Point)],
        output_scale: Option<f32>,
        raster_scale: f32,
    ) -> Vec<u8> {
        let mut paint = crate::prepare_stroke(fixture_stroke, false);
        paint.profile = Some("FountainPen");
        paint.points = Cow::Owned(stamps.iter().map(|stamp| stamp.0).collect());
        paint.dot_radii = Some(stamps.iter().map(|stamp| stamp.1).collect());
        paint.dot_directions = Some(stamps.iter().map(|stamp| stamp.2).collect());
        paint.bounds = Some(BoundingBox {
            x_min: 0.0,
            y_min: 0.0,
            x_max: 64.0,
            y_max: 64.0,
        });
        paint.color = "#000000".into();
        paint.opacity = 1.0;
        paint.fountain_output_scale = output_scale;
        let mut svg = Scene::new(Svg::new().width(64).height(64));
        assert!(render(&mut svg, &paint, false, None));
        let tree = resvg::usvg::Tree::from_str(&svg.finish(), &resvg::usvg::Options::default())
            .expect("V14 scaled SVG parses for raster comparison");
        let mut pixmap = resvg::tiny_skia::Pixmap::new(64, 64).expect("64x64 target");
        // The capture projects around (32, 32), then flips glReadPixels rows
        // into top-to-bottom PNG order. Those two Y reversals cancel, so both
        // SVG axes scale positively around the same anchor in atlas space.
        let transform = resvg::tiny_skia::Transform::from_row(
            raster_scale,
            0.0,
            0.0,
            raster_scale,
            32.0 - 32.0 * raster_scale,
            32.0 - 32.0 * raster_scale,
        );
        resvg::render(&tree, transform, &mut pixmap.as_mut());
        pixmap
            .data()
            .as_chunks::<4>()
            .0
            .iter()
            .map(|pixel| pixel[3])
            .collect()
    }

    fn v16_scene(stamps: &[(f64, f64, f64)], opacity: f64, replay: bool) -> String {
        v16_scene_with_output_scale(stamps, opacity, replay, None)
    }

    fn v16_scene_with_output_scale(
        stamps: &[(f64, f64, f64)],
        opacity: f64,
        replay: bool,
        output_scale: Option<f32>,
    ) -> String {
        let reference: serde_json::Value =
            serde_json::from_str(include_str!("../../../../conformance/fountain-v14.json"))
                .unwrap();
        let stroke: Stroke = serde_json::from_value(reference["stroke"].clone()).unwrap();
        let mut paint = crate::prepare_stroke(&stroke, false);
        paint.profile = Some("FountainPen");
        paint.points = Cow::Owned(
            stamps
                .iter()
                .map(|(x, y, _)| Point { x: *x, y: *y })
                .collect(),
        );
        paint.dot_radii = Some(stamps.iter().map(|(_, _, r)| *r).collect());
        paint.dot_directions = None;
        paint.bounds = Some(BoundingBox {
            x_min: 0.,
            y_min: 0.,
            x_max: 64.,
            y_max: 64.,
        });
        paint.color = "#1a1a1a".into();
        paint.opacity = opacity;
        paint.fountain_output_scale = output_scale;
        let mut svg = Scene::new(Svg::new().width(64).height(64));
        assert!(render(&mut svg, &paint, replay, None));
        svg.finish()
    }

    #[test]
    fn explicit_output_scale_uses_native_f32_device_geometry_and_keeps_pdf_radius() {
        let make_plan = |scale, radius| {
            let reference: serde_json::Value =
                serde_json::from_str(include_str!("../../../../conformance/fountain-v14.json"))
                    .unwrap();
            let stroke: Stroke = serde_json::from_value(reference["stroke"].clone()).unwrap();
            let mut paint = crate::prepare_stroke(&stroke, false);
            paint.profile = Some("FountainPen");
            paint.points = Cow::Owned(vec![Point { x: 32.25, y: 31.75 }]);
            paint.dot_radii = Some(vec![radius]);
            paint.dot_directions = None;
            paint.bounds = Some(BoundingBox {
                x_min: 0.0,
                y_min: 0.0,
                x_max: 64.0,
                y_max: 64.0,
            });
            paint.fountain_v17_rgb = Some(vec![[0.2, 0.4, 0.6]]);
            paint.fountain_output_scale = scale;
            V16Plan::prepare(&paint).unwrap()
        };
        for (scale, inverse_expected, extent_expected, inner_expected) in [
            (1.0_f32, 1.0_f32, 2.5_f32, 0.3_f32),
            (2.0_f32, 0.5_f32, 2.25_f32, 0.388_888_9_f32),
            (4.0_f32, 0.25_f32, 2.125_f32, 0.441_176_47_f32),
        ] {
            let plan = make_plan(Some(scale), 2.0);
            assert_eq!((1.0_f32 / scale).to_bits(), inverse_expected.to_bits());
            let stamp = plan.stamps[0];
            assert_eq!(stamp.extent_x.to_bits(), extent_expected.to_bits());
            assert_eq!(stamp.extent_y.to_bits(), extent_expected.to_bits());
            assert_eq!(
                stamp.gradient_key.inner,
                inner_expected.to_bits(),
                "device-space inner stop at {scale} px/page-unit"
            );
            #[cfg(feature = "pdf")]
            {
                let record = plan.radial_circle_records().unwrap()[0];
                assert_eq!(record[2].to_bits(), 2.0_f32.to_bits());
                assert_eq!(record[3].to_bits(), 2.0_f32.to_bits());
            }
        }
        let canonical = make_plan(None, 2.0);
        let explicit_one = make_plan(Some(1.0), 2.0);
        assert_eq!(
            canonical.stamps[0].x.to_bits(),
            explicit_one.stamps[0].x.to_bits()
        );
        assert_eq!(
            canonical.stamps[0].extent_x.to_bits(),
            explicit_one.stamps[0].extent_x.to_bits()
        );
        assert_eq!(
            canonical.stamps[0].gradient_key,
            explicit_one.stamps[0].gradient_key
        );
        assert_eq!(
            v16_scene_with_output_scale(&[(32.25, 31.75, 2.0)], 1.0, false, None),
            v16_scene_with_output_scale(&[(32.25, 31.75, 2.0)], 1.0, false, Some(1.0))
        );
        let thin_high_zoom = make_plan(Some(4.0), 0.25);
        let thin_record = thin_high_zoom.radial_circle_records().unwrap()[0];
        assert_eq!(thin_record[2].to_bits(), 0.25_f32.to_bits());
        assert_eq!(thin_record[3].to_bits(), 0.25_f32.to_bits());
    }

    #[test]
    fn invalid_output_scale_or_derived_geometry_rejects_the_whole_plan() {
        let reference: serde_json::Value =
            serde_json::from_str(include_str!("../../../../conformance/fountain-v14.json"))
                .unwrap();
        let stroke: Stroke = serde_json::from_value(reference["stroke"].clone()).unwrap();
        let mut paint = crate::prepare_stroke(&stroke, false);
        paint.profile = Some("FountainPen");
        paint.points = Cow::Owned(vec![Point { x: 32.0, y: 32.0 }]);
        paint.dot_radii = Some(vec![2.0]);
        paint.dot_directions = None;
        paint.bounds = Some(BoundingBox {
            x_min: 0.0,
            y_min: 0.0,
            x_max: 64.0,
            y_max: 64.0,
        });
        paint.fountain_v17_rgb = Some(vec![[0.2, 0.4, 0.6]]);
        for scale in [f32::NAN, f32::INFINITY, 0.0, -1.0, f32::from_bits(1)] {
            paint.fountain_output_scale = Some(scale);
            assert!(V16Plan::prepare(&paint).is_none(), "accepted scale {scale}");
        }
        paint.fountain_output_scale = Some(f32::MAX);
        paint.dot_radii = Some(vec![f64::from(f32::MAX)]);
        assert!(
            V16Plan::prepare(&paint).is_none(),
            "accepted overflowed extents"
        );
    }

    #[test]
    fn v14_scaled_addpoint_records_match_the_archived_native_atlas() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../conformance/fountain-v14-fringe-atlas.json"
        ))
        .unwrap();
        let radii: Vec<f64> = fixture["input"]["radii_document_units"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_f64().unwrap())
            .collect();
        let directions: Vec<Point> = fixture["input"]["directions"]
            .as_array()
            .unwrap()
            .iter()
            .map(|pair| Point {
                x: pair[0].as_f64().unwrap(),
                y: pair[1].as_f64().unwrap(),
            })
            .collect();
        let positions: Vec<Point> = fixture["input"]["subpixel_positions_document_units"]
            .as_array()
            .unwrap()
            .iter()
            .map(|pair| Point {
                x: pair[0].as_f64().unwrap(),
                y: pair[1].as_f64().unwrap(),
            })
            .collect();
        let runs = fixture["runs"].as_array().unwrap();
        assert_eq!(runs.len(), 180);
        let decoder = png::Decoder::new(std::io::Cursor::new(include_bytes!(
            "../../../../conformance/fountain-v14-fringe-atlas.png"
        )));
        let mut reader = decoder.read_info().unwrap();
        assert_eq!((reader.info().width, reader.info().height), (11_520, 64));
        assert_eq!(reader.info().color_type, png::ColorType::Grayscale);
        assert_eq!(reader.info().bit_depth, png::BitDepth::Eight);
        let mut atlas = vec![0; reader.output_buffer_size().unwrap()];
        let info = reader.next_frame(&mut atlas).unwrap();
        atlas.truncate(info.buffer_size());
        use sha2::Digest;
        for run in runs {
            let case_index = run[0].as_u64().unwrap() as usize;
            let scale = run[1].as_f64().unwrap() as f32;
            let radius = radii[case_index / 12];
            let direction = directions[(case_index % 12) / 4];
            let point = positions[case_index % 4];
            let record = v14_native_addpoint_record(point, radius, direction, scale).unwrap();
            let expected: Vec<u32> = run[2]
                .as_array()
                .unwrap()
                .iter()
                .map(|word| {
                    let bytes = word.as_str().unwrap().as_bytes();
                    assert_eq!(bytes.len(), 8);
                    let mut little_endian = [0; 4];
                    for (byte, pair) in little_endian.iter_mut().zip(bytes.as_chunks::<2>().0) {
                        *byte = u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap();
                    }
                    u32::from_le_bytes(little_endian)
                })
                .collect();
            assert_eq!(expected.len(), record.len());
            for (slot, (actual, expected)) in record.iter().zip(expected).enumerate() {
                assert_eq!(actual.to_bits(), expected, "case {case_index}, slot {slot}");
            }
            let atlas_x = run[5].as_u64().unwrap() as usize;
            let mut native_alpha = Vec::with_capacity(64 * 64);
            for row in 0..64 {
                let start = row * 11_520 + atlas_x;
                native_alpha.extend_from_slice(&atlas[start..start + 64]);
            }
            assert_eq!(
                format!("{:x}", sha2::Sha256::digest(&native_alpha)),
                run[4].as_str().unwrap(),
                "alpha tile hash for native case {case_index}"
            );
            assert!(
                run[6].as_u64().unwrap() <= 1,
                "shader error for case {case_index}"
            );
        }
    }

    #[test]
    fn v14_scaled_svg_raster_metrics_against_archived_native_atlas() {
        use std::io::Cursor;

        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../conformance/fountain-v14-fringe-atlas.json"
        ))
        .unwrap();
        let source: serde_json::Value =
            serde_json::from_str(include_str!("../../../../conformance/fountain-v14.json"))
                .unwrap();
        let stroke: Stroke = serde_json::from_value(source["stroke"].clone()).unwrap();
        let radii: Vec<f64> = fixture["input"]["radii_document_units"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_f64().unwrap())
            .collect();
        let directions: Vec<Point> = fixture["input"]["directions"]
            .as_array()
            .unwrap()
            .iter()
            .map(|pair| Point {
                x: pair[0].as_f64().unwrap(),
                y: pair[1].as_f64().unwrap(),
            })
            .collect();
        let positions: Vec<Point> = fixture["input"]["subpixel_positions_document_units"]
            .as_array()
            .unwrap()
            .iter()
            .map(|pair| Point {
                x: pair[0].as_f64().unwrap(),
                y: pair[1].as_f64().unwrap(),
            })
            .collect();
        let decoder = png::Decoder::new(Cursor::new(include_bytes!(
            "../../../../conformance/fountain-v14-fringe-atlas.png"
        )));
        let mut reader = decoder.read_info().unwrap();
        assert_eq!((reader.info().width, reader.info().height), (11_520, 64));
        assert_eq!(reader.info().color_type, png::ColorType::Grayscale);
        let mut atlas = vec![0; reader.output_buffer_size().unwrap()];
        let info = reader.next_frame(&mut atlas).unwrap();
        atlas.truncate(info.buffer_size());

        let scales = [1.0_f32, 2.0, 4.0];
        let mut metrics: [V14AlphaComparison; 3] =
            std::array::from_fn(|_| V14AlphaComparison::default());
        let mut canonical_metrics: [V14AlphaComparison; 3] =
            std::array::from_fn(|_| V14AlphaComparison::default());
        let detailed_diagnostics = std::env::var_os("SDOCX_V14_ALPHA_DIAGNOSTICS").is_some();
        let runs = fixture["runs"].as_array().unwrap();
        assert_eq!(runs.len(), 180);
        for run in runs {
            let case_index = run[0].as_u64().unwrap() as usize;
            let scale = run[1].as_f64().unwrap() as f32;
            let scale_index = match scale as u32 {
                1 => 0,
                2 => 1,
                4 => 2,
                other => panic!("unexpected archived scale {other}"),
            };
            let point = positions[case_index % positions.len()];
            let radius = radii[case_index / 12];
            let direction = directions[(case_index % 12) / 4];
            let rendered = v14_raster_tile(&stroke, point, radius, direction, Some(scale), scale);
            let canonical = v14_raster_tile(&stroke, point, radius, direction, None, scale);
            let atlas_x = run[5].as_u64().unwrap() as usize;
            let mut native = Vec::with_capacity(64 * 64);
            for row in 0..64 {
                let start = row * 11_520 + atlas_x;
                native.extend_from_slice(&atlas[start..start + 64]);
            }
            if detailed_diagnostics {
                let mut native_bounds = [64_usize, 64, 0, 0];
                let mut rendered_bounds = [64_usize, 64, 0, 0];
                let mut native_support = 0;
                let mut rendered_support = 0;
                let mut support_xor = 0;
                let mut union = 0;
                let mut error_sum = 0_u64;
                let mut maximum = (0_u8, 0_usize, 0_usize, 0_u8, 0_u8);
                for y in 0..64 {
                    for x in 0..64 {
                        let offset = y * 64 + x;
                        let expected = native[offset];
                        let actual = rendered[offset];
                        if expected != 0 {
                            native_support += 1;
                            native_bounds[0] = native_bounds[0].min(x);
                            native_bounds[1] = native_bounds[1].min(y);
                            native_bounds[2] = native_bounds[2].max(x);
                            native_bounds[3] = native_bounds[3].max(y);
                        }
                        if actual != 0 {
                            rendered_support += 1;
                            rendered_bounds[0] = rendered_bounds[0].min(x);
                            rendered_bounds[1] = rendered_bounds[1].min(y);
                            rendered_bounds[2] = rendered_bounds[2].max(x);
                            rendered_bounds[3] = rendered_bounds[3].max(y);
                        }
                        support_xor += ((expected != 0) != (actual != 0)) as usize;
                        union += (expected != 0 || actual != 0) as usize;
                        let error = expected.abs_diff(actual);
                        error_sum += u64::from(error);
                        if error > maximum.0 {
                            maximum = (error, x, y, expected, actual);
                        }
                    }
                }
                eprintln!(
                    "V14 tile case={case_index} scale={scale} radius={radius} direction=({:.4},{:.4}) point=({:.3},{:.3}) native_support={native_support} rendered_support={rendered_support} support_xor={support_xor} mae_union={:.4} native_bbox={native_bounds:?} rendered_bbox={rendered_bounds:?} max_error=(x={},y={},native={},rendered={},delta={}) center=({}, {})",
                    direction.x,
                    direction.y,
                    point.x,
                    point.y,
                    error_sum as f64 / union.max(1) as f64,
                    maximum.1,
                    maximum.2,
                    maximum.3,
                    maximum.4,
                    maximum.0,
                    native[32 * 64 + 32],
                    rendered[32 * 64 + 32],
                );
            }
            metrics[scale_index].add_tile(&native, &rendered);
            canonical_metrics[scale_index].add_tile(&native, &canonical);
        }

        // Record measurable rendered coverage against the captured native
        // support. The atlas's native scalar-vs-shader error is at most one
        // byte, but this test intentionally does not turn that into an
        // unmeasured resvg-vs-GLES acceptance threshold.
        for ((scale, result), canonical) in scales.into_iter().zip(&metrics).zip(&canonical_metrics)
        {
            assert_eq!(result.pixels, 60 * 64 * 64);
            assert!(result.native_support > 0);
            assert!(result.rendered_support > 0);
            assert!(
                result.mae_over_union() < canonical.mae_over_union(),
                "scale-aware V14 did not improve over canonical at scale {scale}: scaled={result:?}, canonical={canonical:?}"
            );
            eprintln!(
                "V14 atlas scale={scale}: scaled(native_support={}, rendered_support={}, support_xor={}, union={}, alpha_mae_over_union={:.4}, max_abs_error={}); canonical(native_support={}, rendered_support={}, support_xor={}, union={}, alpha_mae_over_union={:.4}, max_abs_error={})",
                result.native_support,
                result.rendered_support,
                result.support_xor,
                result.union_pixels,
                result.mae_over_union(),
                result.max_absolute_error,
                canonical.native_support,
                canonical.rendered_support,
                canonical.support_xor,
                canonical.union_pixels,
                canonical.mae_over_union(),
                canonical.max_absolute_error,
            );
        }
    }

    #[test]
    fn v14_scaled_overlaps_preserve_duplicate_alpha_and_match_max_composition_model() {
        use std::io::Cursor;

        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../conformance/fountain-v14-fringe-atlas.json"
        ))
        .unwrap();
        let source: serde_json::Value =
            serde_json::from_str(include_str!("../../../../conformance/fountain-v14.json"))
                .unwrap();
        let stroke: Stroke = serde_json::from_value(source["stroke"].clone()).unwrap();
        let radii: Vec<f64> = fixture["input"]["radii_document_units"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_f64().unwrap())
            .collect();
        let direction = Point { x: 1.0, y: 0.0 };
        let center = Point { x: 32.0, y: 32.0 };
        let scale = 2.0_f32;
        let decoder = png::Decoder::new(Cursor::new(include_bytes!(
            "../../../../conformance/fountain-v14-fringe-atlas.png"
        )));
        let mut reader = decoder.read_info().unwrap();
        let mut atlas = vec![0; reader.output_buffer_size().unwrap()];
        let info = reader.next_frame(&mut atlas).unwrap();
        atlas.truncate(info.buffer_size());
        let native_tile = |case_index: usize| {
            let run = fixture["runs"]
                .as_array()
                .unwrap()
                .iter()
                .find(|run| {
                    run[0].as_u64() == Some(case_index as u64)
                        && run[1].as_f64() == Some(f64::from(scale))
                })
                .expect("pinned atlas has requested case and scale");
            let atlas_x = run[5].as_u64().unwrap() as usize;
            let mut tile = Vec::with_capacity(64 * 64);
            for row in 0..64 {
                let start = row * 11_520 + atlas_x;
                tile.extend_from_slice(&atlas[start..start + 64]);
            }
            tile
        };

        // Repeated V14 records use the renderer's production Lighten/filter
        // path. They must not accumulate alpha as source-over would.
        let thin = (center, radii[0], direction);
        let once = v14_raster_stamps(&stroke, &[thin], Some(scale), scale);
        let twice = v14_raster_stamps(&stroke, &[thin, thin], Some(scale), scale);
        assert_eq!(twice, once, "identical native stamps must not build alpha");

        // These two archived native tiles are centered and direction-aligned,
        // but use different radii. Their per-pixel native GL_MAX result is the
        // expected coverage for their overlapping native stamps.
        let native_thin = native_tile(0);
        let native_wide = native_tile(48);
        let native_max = native_thin
            .iter()
            .zip(&native_wide)
            .map(|(&a, &b)| a.max(b))
            .collect::<Vec<_>>();
        let wide = (center, radii[4], direction);
        let rendered_thin = v14_raster_stamps(&stroke, &[thin], Some(scale), scale);
        let rendered_wide = v14_raster_stamps(&stroke, &[wide], Some(scale), scale);
        let rendered_pair = v14_raster_stamps(&stroke, &[thin, wide], Some(scale), scale);
        let rendered_max = rendered_thin
            .iter()
            .zip(&rendered_wide)
            .map(|(&a, &b)| a.max(b))
            .collect::<Vec<_>>();
        assert_eq!(
            rendered_pair, rendered_max,
            "overlapping SVG stamps should retain per-pixel Lighten/MAX alpha"
        );
        let mut native_comparison = V14AlphaComparison::default();
        native_comparison.add_tile(&native_max, &rendered_pair);
        eprintln!(
            "V14 overlap scale={scale}: native_max_support={} rendered_support={} support_xor={} union={} alpha_mae_over_union={:.4} max_abs_error={}",
            native_comparison.native_support,
            native_comparison.rendered_support,
            native_comparison.support_xor,
            native_comparison.union_pixels,
            native_comparison.mae_over_union(),
            native_comparison.max_absolute_error,
        );
    }

    #[test]
    fn v14_scaled_route_is_vector_only_and_invalid_scale_keeps_canonical_svg() {
        let reference: serde_json::Value =
            serde_json::from_str(include_str!("../../../../conformance/fountain-v14.json"))
                .unwrap();
        let stroke: Stroke = serde_json::from_value(reference["stroke"].clone()).unwrap();
        let mut paint = crate::prepare_stroke(&stroke, false);
        paint.points = Cow::Owned(vec![Point { x: 32.25, y: 31.75 }]);
        paint.dot_radii = Some(vec![0.1]);
        paint.dot_directions = Some(vec![Point { x: 1., y: 0. }]);
        paint.bounds = Some(BoundingBox {
            x_min: 0.,
            y_min: 0.,
            x_max: 64.,
            y_max: 64.,
        });
        let render_svg = |paint: &PreparedStroke<'_>| {
            let mut svg = Scene::new(Svg::new().width(64).height(64));
            assert!(render(&mut svg, paint, false, None));
            svg.finish()
        };
        let canonical = render_svg(&paint);
        for invalid in [f32::NAN, 0.0, -1.0, 1_000_001.0] {
            paint.fountain_output_scale = Some(invalid);
            assert_eq!(render_svg(&paint), canonical);
        }
        paint.fountain_output_scale = Some(4.0);
        let scaled = render_svg(&paint);
        assert!(scaled.contains("<mask"));
        assert!(scaled.contains("radialGradient"));
        let width_alpha = v14_native_addpoint_record(
            Point { x: 32.25, y: 31.75 },
            0.1,
            Point { x: 1.0, y: 0.0 },
            4.0,
        )
        .unwrap()[8];
        let serialized_opacities = scaled
            .match_indices("opacity=\"")
            .filter_map(|(start, _)| {
                scaled[start + 9..]
                    .split('\"')
                    .next()
                    .and_then(|value| value.parse::<f32>().ok())
            })
            .collect::<Vec<_>>();
        assert!(
            serialized_opacities
                .iter()
                .any(|value| (*value - width_alpha).abs() < 1e-6)
        );
        assert!(!scaled.contains("<image"));
        assert!(resvg::usvg::Tree::from_str(&scaled, &resvg::usvg::Options::default()).is_ok());
    }

    #[cfg(feature = "pdf")]
    #[test]
    fn v16_slice_bounds_include_shader_fringe_and_touching_edges() {
        use crate::{LayoutPage, Page};

        let reference: serde_json::Value =
            serde_json::from_str(include_str!("../../../../conformance/fountain-v14.json"))
                .unwrap();
        let stroke: Stroke = serde_json::from_value(reference["stroke"].clone()).unwrap();
        let mut paint = crate::prepare_stroke(&stroke, false);
        paint.profile = Some("FountainPen");
        paint.points = Cow::Owned(vec![Point { x: 10.0, y: 10.0 }]);
        paint.dot_radii = Some(vec![0.25]);
        paint.dot_directions = None;
        paint.bounds = Some(BoundingBox {
            x_min: 0.0,
            y_min: 0.0,
            x_max: 20.0,
            y_max: 20.0,
        });
        let page = LayoutPage {
            page: Page {
                uuid: "slice-test".to_owned(),
                width: 100,
                height: 100,
                content_bbox: BoundingBox::default(),
                background_color: None,
                template: None,
                background: Default::default(),
                objects: Vec::new(),
            },
            source_page_index: 0,
            body_text: None,
        };
        let touching = crate::render::PageSlice::new_for_page(&page, 11, 10, false).unwrap();
        let separated = crate::render::PageSlice::new_for_page(&page, 12, 10, false).unwrap();
        let plan = PaintPlan::prepare(&paint, true).unwrap();
        assert!(!plan.outside(touching), "the V16 +0.5 extent touches y=11");
        assert!(plan.outside(separated), "the V16 fringe ends before y=12");

        // Compare identical viewports with culling on/off, including a visible
        // fringe and the adjacent empty slice. This is transport equivalence,
        // not a native pixel oracle.
        for y in [10, 11, 12] {
            let request = crate::render::PageSceneRequest::sliced(&page, y, 10, false).unwrap();
            let images = [request, request.with_fountain_paint_culling(false)].map(|request| {
                let mut scene = Scene::new(Svg::new().width(64).height(10).view_box(
                    super::super::vector::ViewBox::unrounded(0., f64::from(y), 64., 10.),
                ));
                assert!(render(&mut scene, &paint, false, request.slice()));
                preview(&scene.finish())
            });
            assert!(images[0].data() == images[1].data(), "slice y={y}");
            if y == 10 {
                assert!(images[0].pixels().iter().any(|pixel| pixel.alpha() > 0));
            }
        }
    }

    #[test]
    fn fountain_gradient_follows_tangent_and_applies_opacity_once() {
        let horizontal = shaded_stamp(Point { x: 1., y: 0. }, 1);
        let image = preview(&horizontal);
        // At pixel center x=48.5, normalized tangent distance is 0.825.
        let expected = 255. * 0.6 * (1. - 1.86 * (0.825 - 0.5));
        assert!((image.pixel(48, 32).unwrap().alpha() as f64 - expected).abs() < 2.);
        assert!((image.pixel(32, 48).unwrap().alpha() as i32 - 153).abs() <= 1);
        let repeated = preview(&shaded_stamp(Point { x: 1., y: 0. }, 2));
        // Check the filled interior; edge antialiasing belongs to the Svg
        // consumer and can differ when coincident boundaries are drawn twice.
        for y in 14..50 {
            for x in 14..50 {
                if (x as f64 + 0.5 - 32.).hypot(y as f64 + 0.5 - 32.) < 18. {
                    assert_eq!(
                        image.pixel(x, y),
                        repeated.pixel(x, y),
                        "overlap at {x},{y}"
                    );
                }
            }
        }
        let vertical = preview(&shaded_stamp(Point { x: 0., y: 1. }, 1));
        assert_eq!(image.pixel(48, 32), vertical.pixel(32, 48));
        assert!(!horizontal.contains("<image"));
        assert_eq!(horizontal.matches("<filter ").count(), 1);
        assert_eq!(horizontal.matches("<feColorMatrix").count(), 1);
    }

    #[test]
    fn v14_colorizer_keeps_tint_opacity_and_replay_parts() {
        let svg = shaded_stamp_with_color(Point { x: 1., y: 0. }, 2, "#336699", false);
        let xml = roxmltree::Document::parse(&svg).unwrap();
        let filter = xml
            .descendants()
            .find(|node| node.has_tag_name("filter"))
            .unwrap();
        assert_eq!(filter.attribute("filterUnits"), Some("userSpaceOnUse"));
        assert_eq!(
            filter.attribute("color-interpolation-filters"),
            Some("sRGB")
        );
        let matrix = xml
            .descendants()
            .find(|node| node.has_tag_name("feColorMatrix"))
            .unwrap();
        let values = svgtypes::NumberListParser::from(matrix.attribute("values").unwrap())
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(values.len(), 20);
        assert_eq!(values[4], 0x33 as f64 / 255.);
        assert_eq!(values[9], 0x66 as f64 / 255.);
        assert_eq!(values[14], 0x99 as f64 / 255.);
        assert_eq!(&values[15..], &[1., 0., 0., 0., 0.]);
        assert_eq!(
            xml.descendants()
                .filter(|node| node.attribute("filter").is_some())
                .count(),
            1
        );
        assert_eq!(
            xml.descendants()
                .filter(|node| node.attribute("opacity").is_some())
                .count(),
            1,
            "saved opacity is applied once on the filtered source"
        );
        let tinted = preview(&svg);
        let pixel = tinted.pixel(48, 32).unwrap();
        let expected_alpha = 255. * 0.6 * (1. - 1.86 * (0.825 - 0.5));
        assert!((pixel.alpha() as f64 - expected_alpha).abs() <= 2.);
        let expected_channel = |channel: u8| (channel as f64 * expected_alpha / 255.).round() as u8;
        for (actual, expected) in [
            (pixel.red(), expected_channel(0x33)),
            (pixel.green(), expected_channel(0x66)),
            (pixel.blue(), expected_channel(0x99)),
        ] {
            assert!((actual as i16 - expected as i16).abs() <= 1);
        }
        assert!(!svg.contains("<image"));
        assert!(!svg.contains("<feImage"));

        let replay = shaded_stamp_with_color(Point { x: 1., y: 0. }, 2, "#336699", true);
        let replay_xml = roxmltree::Document::parse(&replay).unwrap();
        let parts = replay_xml
            .descendants()
            .filter_map(|node| node.attribute("data-replay-part"))
            .collect::<Vec<_>>();
        assert_eq!(parts, ["1", "2"]);
    }

    #[test]
    fn v16_svg_fields_match_native_unit_scale_records() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../conformance/fountain-v16-addpoint-native.json"
        ))
        .unwrap();
        let mut checked = 0;
        for case in fixture["cases"].as_array().unwrap() {
            if case["inverse_xy"] != serde_json::json!([1.0, 1.0])
                || case["radius"].as_f64().unwrap() <= 0.0
            {
                continue;
            }
            let native: Vec<f32> = case["record_f32_le_hex_words"]
                .as_array()
                .unwrap()
                .iter()
                .map(|word| {
                    f32::from_bits(
                        u32::from_str_radix(word.as_str().unwrap(), 16)
                            .unwrap()
                            .swap_bytes(),
                    )
                })
                .collect();
            let svg = v16_scene(
                &[(32.25, 32.25, case["radius"].as_f64().unwrap())],
                1.,
                false,
            );
            let xml = roxmltree::Document::parse(&svg).unwrap();
            let transform = xml
                .descendants()
                .filter_map(|node| node.attribute("transform"))
                .find(|value| value.starts_with("matrix("))
                .unwrap();
            let matrix: Vec<f32> = transform
                .strip_prefix("matrix(")
                .unwrap()
                .strip_suffix(')')
                .unwrap()
                .split(|c: char| c == ',' || c.is_ascii_whitespace())
                .filter(|part| !part.is_empty())
                .map(|part| part.parse().unwrap())
                .collect();
            assert_eq!(matrix, [native[2], 0., 0., native[3], native[0], native[1]]);
            let stops: Vec<(f32, f32)> = xml
                .descendants()
                .filter(|node| node.has_tag_name("stop"))
                .map(|node| {
                    (
                        node.attribute("offset").unwrap().parse().unwrap(),
                        node.attribute("stop-opacity").unwrap().parse().unwrap(),
                    )
                })
                .collect();
            let mut expected = vec![(0., native[6])];
            if native[5] > 0. {
                expected.push((native[5] + native[5], native[6]));
            }
            expected.push((1., 0.));
            assert_eq!(stops, expected, "{}", case["case_id"]);
            checked += 1;
        }
        assert_eq!(checked, 6);
    }

    #[test]
    fn v16_unit_scale_radial_stops_match_rtv5_numeric_witnesses() {
        // Independent RTV5 AddPoint + fragment-shader witnesses at sx=sy=1:
        // r=.1 => extent 1, A=.2, inner=0; center=.2, u=.5 => .1.
        // r=.5 => extent 1, A=1, inner=0.
        // r=2 => extent 2.5, A=1, inner=.3 (u plateau ends at .6).
        let svg = v16_scene(
            &[
                (12.5, 12.5, 0.1),
                (24.5, 24.5, 0.499),
                (36.5, 36.5, 0.5),
                (48.5, 48.5, 0.501),
                (55.5, 55.5, 2.0),
            ],
            1.,
            false,
        );
        let xml = roxmltree::Document::parse(&svg).unwrap();
        let gradients = xml
            .descendants()
            .filter(|node| node.has_tag_name("radialGradient"))
            .collect::<Vec<_>>();
        // .1 and .499 have the same inner stop but distinct width alpha.
        // .5 and .501 are distinct at float32, while repeat parameters share.
        assert_eq!(gradients.len(), 5);
        let stops = |gradient: roxmltree::Node<'_, '_>| {
            gradient
                .children()
                .filter(|node| node.has_tag_name("stop"))
                .map(|node| {
                    (
                        node.attribute("offset").unwrap().parse::<f32>().unwrap(),
                        node.attribute("stop-opacity")
                            .unwrap()
                            .parse::<f32>()
                            .unwrap(),
                    )
                })
                .collect::<Vec<_>>()
        };
        let tiny = gradients
            .iter()
            .find(|gradient| (stops(**gradient)[0].1 - 0.2).abs() < 1e-6)
            .unwrap();
        let tiny_stops = stops(*tiny);
        assert_eq!(tiny_stops, [(0., 0.2), (1., 0.)]);
        assert!((0.2_f32 * (1.0_f32 - 0.5_f32) - 0.1_f32).abs() < f32::EPSILON);
        let large = gradients
            .iter()
            .find(|gradient| {
                let values = stops(**gradient);
                values.len() == 3 && values[1].0 > 0.5
            })
            .unwrap();
        let large_stops = stops(*large);
        assert_eq!(large_stops[0], (0., 1.));
        assert!((large_stops[1].0 - 0.6).abs() < 1e-6);
        assert_eq!(large_stops[1].1, 1.);
        assert_eq!(large_stops[2], (1., 0.));
        assert!(!svg.contains("<image"));
        assert!(!svg.contains("<filter "));
        assert!(!svg.contains("<feColorMatrix"));
    }

    #[test]
    fn v16_instances_source_over_with_argb_alpha_per_stamp() {
        let one_svg = v16_scene(&[(32.5, 32.5, 0.1)], 1., false);
        let xml = roxmltree::Document::parse(&one_svg).unwrap();
        assert_eq!(
            xml.descendants()
                .filter(|node| node.has_tag_name("filter"))
                .count(),
            0
        );
        assert_eq!(
            xml.descendants()
                .filter(|node| node.attribute("opacity").is_some())
                .count(),
            0,
            "native ARGB alpha belongs to each V16 instance"
        );
        assert!(
            !xml.descendants()
                .any(|node| node.has_tag_name("rect") && node.attribute("fill") == Some("#000000"))
        );
        let gradient = xml
            .descendants()
            .find(|node| node.has_tag_name("radialGradient"))
            .unwrap();
        assert_eq!(
            gradient
                .children()
                .find(|node| node.has_tag_name("stop"))
                .unwrap()
                .attribute("stop-color"),
            Some("#1a1a1a")
        );
        assert!(!one_svg.contains("<image"));
        let one = preview(&one_svg);
        // Pixel center coincides with the stamp center: native shader A=.2.
        assert!(
            (one.pixel(32, 32).unwrap().alpha() as i32 - 51).abs() <= 1,
            "one center alpha {}",
            one.pixel(32, 32).unwrap().alpha()
        );
        let alpha = preview(&v16_scene(&[(32.5, 32.5, 0.1)], 0.5, false));
        // One instance has shader coverage .2 and source ARGB alpha .5.
        assert!((alpha.pixel(32, 32).unwrap().alpha() as i32 - 26).abs() <= 1);

        let overlap = preview(&v16_scene(
            &[(32.5, 32.5, 0.1), (32.75, 32.5, 0.15)],
            1.,
            false,
        ));
        // At the first center, the instance alphas are .2 and .225. V16 draws
        // with source-over, so .2 + .225 * (1-.2) = .38.
        assert!(
            (overlap.pixel(32, 32).unwrap().alpha() as i32 - 97).abs() <= 1,
            "overlap alpha was {}",
            overlap.pixel(32, 32).unwrap().alpha()
        );
        let partial_overlap = preview(&v16_scene(
            &[(32.5, 32.5, 0.1), (32.75, 32.5, 0.15)],
            0.5,
            false,
        ));
        // The source alpha multiplies each instance before blending: .1 +
        // .1125 * (1-.1) = .20125 (51/255 after rasterization).
        assert!(
            (partial_overlap.pixel(32, 32).unwrap().alpha() as i32 - 51).abs() <= 1,
            "partial-alpha overlap was {}",
            partial_overlap.pixel(32, 32).unwrap().alpha()
        );
        let replay = v16_scene(&[(32.5, 32.5, 0.1), (32.75, 32.5, 0.15)], 1., true);
        let xml = roxmltree::Document::parse(&replay).unwrap();
        assert!(
            xml.descendants()
                .any(|node| { node.attribute("transform") == Some("matrix(1,0,0,1,32.5,32.5)") }),
            "native fractional stamp centers must survive transform serialization"
        );
        let parts = xml
            .descendants()
            .filter_map(|node| node.attribute("data-replay-part"))
            .collect::<Vec<_>>();
        assert_eq!(parts, ["1", "2"]);
        assert!(!replay.contains("<image"));
        assert!(!replay.contains("<filter "));
        assert!(!replay.contains("mix-blend-mode:lighten"));
    }

    #[cfg(feature = "pdf")]
    #[test]
    fn fountain_pdf_retains_vector_shading_and_blending() {
        let page = crate::RenderedPage {
            text_diagnostics: Vec::new(),
            object_diagnostics: Vec::new(),
            geometry_diagnostics: Vec::new(),
            paint_diagnostics: Vec::new(),
            background_diagnostics: Vec::new(),
            source_page_index: 0,
            width: 64,
            height: 64,
            svg: shaded_stamp(Point { x: 1., y: 0. }, 2),
        };
        let bytes = crate::render_svg_pages_pdf(&[page], &crate::PdfOptions::default()).unwrap();
        let pdf = lopdf::Document::load_mem(&bytes).unwrap();
        let mut shading = false;
        let mut lighten = false;
        for object in pdf.objects.values() {
            let dict = match object {
                lopdf::Object::Dictionary(dict) => dict,
                lopdf::Object::Stream(stream) => &stream.dict,
                _ => continue,
            };
            assert_ne!(
                dict.get(b"Subtype").and_then(lopdf::Object::as_name).ok(),
                Some(b"Image".as_slice())
            );
            shading |= dict.has(b"ShadingType");
            lighten |= dict.get(b"BM").and_then(lopdf::Object::as_name).ok()
                == Some(b"Lighten".as_slice());
        }
        assert!(shading, "gradient must remain a PDF shading");
        assert!(
            lighten,
            "maximum coverage must retain its vector blend mode"
        );
    }

    #[cfg(feature = "pdf")]
    #[test]
    fn v16_pdf_retains_radial_opacity_shading_and_source_over_without_images() {
        let page = crate::RenderedPage {
            text_diagnostics: Vec::new(),
            object_diagnostics: Vec::new(),
            geometry_diagnostics: Vec::new(),
            paint_diagnostics: Vec::new(),
            background_diagnostics: Vec::new(),
            source_page_index: 0,
            width: 64,
            height: 64,
            svg: v16_scene(&[(32.5, 32.5, 2.)], 0.5, false),
        };
        let bytes = crate::render_svg_pages_pdf(&[page], &crate::PdfOptions::default()).unwrap();
        let pdf = lopdf::Document::load_mem(&bytes).unwrap();
        let mut radial_shading = false;
        let mut non_normal_blend = false;
        for object in pdf.objects.values() {
            let dict = match object {
                lopdf::Object::Dictionary(dict) => dict,
                lopdf::Object::Stream(stream) => &stream.dict,
                _ => continue,
            };
            assert_ne!(
                dict.get(b"Subtype").and_then(lopdf::Object::as_name).ok(),
                Some(b"Image".as_slice())
            );
            if dict.has(b"ShadingType") {
                radial_shading |= dict
                    .get(b"ShadingType")
                    .and_then(lopdf::Object::as_i64)
                    .is_ok_and(|kind| kind == 3);
            }
            non_normal_blend |= dict.get(b"BM").and_then(lopdf::Object::as_name).ok()
                == Some(b"Lighten".as_slice());
        }
        assert!(
            radial_shading,
            "V16 radial opacity must remain a PDF shading"
        );
        assert!(
            !non_normal_blend,
            "V16 instances use source-over compositing"
        );
    }

    #[cfg(feature = "pdf")]
    #[test]
    fn v17_rainbow_pdf_uses_bounded_circles_and_falls_back_atomically() {
        let render_one = |radius: f64, retained: bool, count: usize| {
            let reference: serde_json::Value =
                serde_json::from_str(include_str!("../../../../conformance/fountain-v14.json"))
                    .unwrap();
            let stroke: Stroke = serde_json::from_value(reference["stroke"].clone()).unwrap();
            let mut paint = crate::prepare_stroke(&stroke, false);
            paint.profile = Some("FountainPen");
            paint.points = Cow::Owned(vec![Point { x: 32.25, y: 31.75 }; count]);
            paint.dot_radii = Some(vec![radius; count]);
            paint.dot_directions = None;
            paint.bounds = Some(BoundingBox {
                x_min: 0.0,
                y_min: 0.0,
                x_max: 64.0,
                y_max: 64.0,
            });
            paint.color = "#000000".into();
            paint.opacity = 0.5;
            paint.fountain_v17_rgb = Some(vec![[0.25, 0.5, 0.75]; count]);
            let mut scene = Scene::new(Svg::new().width(64).height(64));
            if retained {
                scene.retain_text();
            }
            assert!(render(&mut scene, &paint, false, None));
            if retained {
                let ink = scene.take_native_ink();
                let text = scene.take_native_text();
                let svg = scene.finish();
                return (svg, ink, text);
            }
            (
                scene.finish(),
                super::super::native_ink::NativeInkRegistry::default(),
                super::super::text::native::NativeTextRegistry::default(),
            )
        };

        let (_, wide_ink, _) = render_one(20.0, true, 1);
        assert_eq!(
            wide_ink.iter().count(),
            1,
            "wide full-coverage circles are supported"
        );
        let (fallback_svg, fallback_ink, _) = render_one(1.5, true, 4097);
        let (canonical_fallback_svg, _, _) = render_one(1.5, false, 4097);
        assert_eq!(fallback_svg, canonical_fallback_svg);
        assert!(fallback_ink.is_empty());
        let (_, thin_fallback_ink, _) = render_one(0.25, true, 1);
        assert!(
            thin_fallback_ink.is_empty(),
            "partial-coverage thin stamps retain the original SVG field"
        );

        let (svg, ink, text) = render_one(1.5, true, 1);
        let entries = ink.iter().collect::<Vec<_>>();
        assert_eq!(entries.len(), 1);
        assert_eq!(
            entries[0].1.opacity, 1.0,
            "saved opacity lives in the Form once"
        );
        let crate::render::native_ink::NativeInkPayload::RadialCircles {
            records,
            saved_alpha,
        } = &entries[0].1.payload
        else {
            panic!("expected Fountain V17 radial-circle payload");
        };
        assert_eq!(
            records[0][2], 1.5,
            "native radius is expanded extent minus the half-unit fringe"
        );
        assert_eq!(records[0][4], 0.0);
        assert_eq!(records[0][5], 1.0);
        assert_eq!(records[0][6..9], [0.25, 0.5, 0.75]);
        assert_eq!(*saved_alpha, 0.5);
        assert!(svg.contains("id=\"sdocx-native-ink-0\""));
        assert!(!svg.contains("<image"));

        let page = crate::RenderedPage {
            text_diagnostics: Vec::new(),
            object_diagnostics: Vec::new(),
            geometry_diagnostics: Vec::new(),
            paint_diagnostics: Vec::new(),
            background_diagnostics: Vec::new(),
            source_page_index: 0,
            width: 64,
            height: 64,
            svg,
        };
        let scene = crate::render::RenderedScene {
            page,
            text,
            ink,
            text_error: None,
        };
        let bytes = crate::pdf::render_scenes_pdf_with_progress(
            &[scene],
            &crate::PdfOptions::default(),
            &mut |_| {},
        )
        .unwrap();
        let pdf = lopdf::Document::load_mem(&bytes).unwrap();
        let mut circle_form = false;
        for object in pdf.objects.values() {
            let lopdf::Object::Stream(stream) = object else {
                continue;
            };
            assert_ne!(
                stream
                    .dict
                    .get(b"Subtype")
                    .and_then(lopdf::Object::as_name)
                    .ok(),
                Some(b"Image".as_slice()),
                "retained Fountain V17 remains vector-only"
            );
            if stream
                .dict
                .get(b"Subtype")
                .and_then(lopdf::Object::as_name)
                .ok()
                != Some(b"Form".as_slice())
            {
                continue;
            }
            let content = stream.decompressed_content().unwrap_or_default();
            let operators = lopdf::content::Content::decode(&content).unwrap();
            if operators
                .operations
                .iter()
                .any(|operation| operation.operator == "rg")
                && operators
                    .operations
                    .iter()
                    .any(|operation| operation.operator == "c")
            {
                let group = stream.dict.get(b"Group").unwrap().as_dict().unwrap();
                assert!(matches!(group.get(b"I"), Ok(lopdf::Object::Boolean(true))));
                assert!(matches!(group.get(b"K"), Ok(lopdf::Object::Boolean(false))));
                assert!(
                    !operators
                        .operations
                        .iter()
                        .any(|operation| operation.operator == "re")
                );
                circle_form = true;
            }
        }
        assert!(
            circle_form,
            "PDF marker is replaced with the radial-circle form"
        );
    }

    #[test]
    fn saved_fountain_argb_alpha_matches_each_renderer_path() {
        let reference: serde_json::Value =
            serde_json::from_str(include_str!("../../../../conformance/fountain-v14.json"))
                .unwrap();
        let mut stroke: Stroke = serde_json::from_value(reference["stroke"].clone()).unwrap();
        stroke.points = vec![
            Point { x: 20., y: 20. },
            Point { x: 35., y: 20. },
            Point { x: 50., y: 20. },
        ];
        stroke.pressures = vec![0.7; 3];
        stroke.timestamps = vec![0, 10, 20];
        let render_stroke = |stroke: &Stroke, expect_filter: bool| {
            let mut svg = Scene::new(Svg::new().width(64).height(64));
            let mut calligraphy_session = crate::ink::calligraphy::CalligraphySession::new();
            crate::render::render_stroke(
                &mut svg,
                stroke,
                crate::RenderTheme::for_canvas(false),
                None,
                &mut calligraphy_session,
                true,
                [64, 64],
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
            );
            let svg = svg.finish();
            assert!(!svg.contains("<image"));
            if expect_filter {
                assert_eq!(svg.matches("<filter ").count(), 1);
            } else {
                assert!(!svg.contains("<filter"));
            }
            preview(&svg)
        };
        for settings in ["14;", "18;0;100;"] {
            let v16 = settings == "18;0;100;";
            stroke.rendering.as_mut().unwrap().advanced_settings = Some(settings.into());
            stroke.rendering.as_mut().unwrap().style.color_argb = Some(0xff000000);
            let opaque = render_stroke(&stroke, !v16);
            assert!(opaque.pixels().iter().any(|p| p.alpha() == 255));
            for alpha in [0, 1, 64, 128, 254, 255] {
                stroke.rendering.as_mut().unwrap().style.color_argb = Some(alpha << 24);
                let paint = crate::prepare_stroke(&stroke, false);
                assert!(paint.dot_radii.is_some());
                assert_eq!(paint.opacity, (alpha as f32 / 255.) as f64);
                let image = render_stroke(&stroke, !v16);
                for (actual, full) in image.pixels().iter().zip(opaque.pixels()) {
                    if v16 {
                        // V16 multiplies the source alpha into every instance,
                        // then blends those instances source-over. This is
                        // intentionally nonlinear where stamps overlap.
                        if alpha == 0 {
                            assert_eq!(actual.alpha(), 0, "V16 alpha-zero source");
                        } else {
                            assert!(actual.alpha() <= full.alpha());
                            if alpha == 255 {
                                assert_eq!(actual.alpha(), full.alpha());
                            }
                        }
                    } else {
                        let expected = full.alpha() as f64 * paint.opacity;
                        assert!(
                            (actual.alpha() as f64 - expected).abs() <= 2.,
                            "{settings}, alpha {alpha}"
                        );
                    }
                }
            }
        }
    }
}
