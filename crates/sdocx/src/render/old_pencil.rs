use super::vector::{
    Data, Definition, Definitions, Group, ImageId, Paint, Path, Scene, Styled, Transform, Use,
};
use crate::Stroke;
use crate::ink::old_pencil::{
    PencilStyleProfile, SavedPoint, SavedStroke, sample_saved_stroke_equal_threshold_bounded,
};
const ATLAS: &[u8; 144 * 144 * 4] = include_bytes!("old_pencil_atlas.bin");
const NAME: &str = "com.samsung.android.sdk.pen.pen.preload.Pencil";

fn prepare(
    stroke: &Stroke,
    theme: super::RenderTheme,
    budget: &std::cell::Cell<usize>,
    point_budget: &std::cell::Cell<usize>,
) -> Option<(Vec<[f32; 4]>, u32)> {
    let r = stroke.rendering.as_ref()?;
    let p = r.properties;
    if r.pen_name.as_deref().is_some_and(|name| name != NAME)
        || r.advanced_settings.is_some()
        || r.tool_type_raw != 2
        || !p.compressed
        || !p.stylus_channels
        || !p.binary_added
        || p.replay_only
        || p.eraser
        || p.fixed_width
        || p.top_layer_pen
        || p.alpha_lock
        || p.generated
        || p.fixed_opacity
        || p.rainbow_effect
        || p.straighten
        || p.reveal_mode
    {
        return None;
    }
    let size = r.style.pen_size?;
    let argb = r.style.color_argb?;
    let expected = crate::StrokeStyle {
        pen_name_id: Some(2),
        color_argb: Some(argb),
        pen_size: Some(size),
        fixed_width: r.style.fixed_width,
        particle_density: r.style.particle_density,
        initial_tolerance: r.style.initial_tolerance,
        stroke_type_raw: r.style.stroke_type_raw,
        ..Default::default()
    };
    let rgb = crate::Color {
        r: (argb >> 16) as u8,
        g: (argb >> 8) as u8,
        b: argb as u8,
    };
    if r.metadata
        .as_ref()
        .is_none_or(|m| m.rotation_degrees.is_some_and(|v| v.to_bits() != 0))
        || r.style
            .initial_tolerance
            .is_some_and(|v| !v.is_finite() || !(0.0..=100.0).contains(&v))
        || r.style
            .particle_density
            .is_some_and(|v| !(0..=8).contains(&v))
        || r.style
            .fixed_width
            .is_some_and(|v| !v.is_finite() || !(0.0..=20.0).contains(&v))
        || r.style != expected
        || stroke.color != Some(rgb)
        || theme.foreground_color(rgb) != rgb
        || !p.millisecond_timestamps
    {
        return None;
    }
    if !size.is_finite()
        || size <= 0.0
        || !(3.0..=12.0).contains(&size)
        || size.to_bits() != stroke.pen_width.to_bits()
        || r.style.stroke_type_raw.is_some_and(|v| v != 0)
        || stroke.points.len() < 2
        || stroke.points.len() > 4096
        || stroke.pressures.len() != stroke.points.len()
        || stroke.tilts.len() != stroke.points.len()
        || stroke.orientations.len() != stroke.points.len()
        || stroke.timestamps.len() != stroke.points.len()
    {
        return None;
    }
    let work = stroke.points.len().max(1);
    if work > point_budget.get() {
        point_budget.set(0);
        return None;
    }
    point_budget.set(point_budget.get() - work);
    let f = |v: f64| {
        let x = v as f32;
        (v.is_finite() && x.is_finite() && f64::from(x) == v).then_some(x)
    };
    let points = stroke
        .points
        .iter()
        .map(|p| {
            Some(SavedPoint {
                x: f(p.x)?,
                y: f(p.y)?,
            })
        })
        .collect::<Option<Vec<_>>>()?;
    if points
        .iter()
        .any(|p| !(0.0..=1920.0).contains(&p.x) || !(0.0..=1080.0).contains(&p.y))
    {
        return None;
    }
    let pressures = stroke
        .pressures
        .iter()
        .map(|&v| f(v))
        .collect::<Option<Vec<_>>>()?;
    if stroke
        .tilts
        .iter()
        .chain(&stroke.orientations)
        .any(|&v| f(v).is_none())
    {
        return None;
    }
    let Ok(sample) = sample_saved_stroke_equal_threshold_bounded(
        &SavedStroke {
            pen_name: NAME,
            tool_type: 2,
            stroke_type: 0,
            curve_enabled: true,
            points: &points,
            pressures: &pressures,
            size,
        },
        PencilStyleProfile {
            size,
            point_width_override: None,
        },
        budget.get().min(128),
    ) else {
        return None;
    };
    if sample.records.is_empty()
        || sample.records.len() > 128
        || sample.records.len().max(32) > budget.get()
        || sample.records.iter().any(|r| {
            r[2] <= 0.0
                || r[2] > 256.0
                || r[0] < 0.0
                || r[0] > 1920.0
                || r[1] < 0.0
                || r[1] > 1080.0
                || !r.iter().all(|v| v.is_finite())
                || !(0.0..7.0).contains(&r[3])
        })
    {
        return None;
    }
    budget.set(budget.get() - sample.records.len().max(32));
    Some((sample.records, argb))
}

pub(super) fn render(
    svg: &mut Scene,
    stroke: &Stroke,
    theme: super::RenderTheme,
    budget: &std::cell::Cell<usize>,
    point_budget: &std::cell::Cell<usize>,
) -> bool {
    let Some((records, argb)) = prepare(stroke, theme, budget, point_budget) else {
        return false;
    };
    emit_records(svg, &records, argb);
    true
}

fn emit_records(svg: &mut Scene, records: &[[f32; 4]], argb: u32) {
    let ids: Vec<Definition<ImageId>> = (0..7).map(|_| svg.definition()).collect();
    let color = format!("#{:06x}", argb & 0xffffff);
    let opacity = (argb >> 24) as f64 / 255.0;
    svg.scope(Definitions::new(), |svg| {
        for tile in 0..7 {
            svg.scope(Group::new().id(&ids[tile].id_text()), |svg| {
                for alpha in 1..=255u8 {
                    let mut data = Data::new();
                    let mut present = false;
                    for y in 0..48usize {
                        let mut x = 0;
                        while x < 48 {
                            let a =
                                ATLAS[(((tile / 3) * 48 + y) * 144 + (tile % 3) * 48 + x) * 4 + 3];
                            let start = x;
                            x += 1;
                            while x < 48
                                && ATLAS
                                    [(((tile / 3) * 48 + y) * 144 + (tile % 3) * 48 + x) * 4 + 3]
                                    == a
                            {
                                x += 1
                            }
                            if a == alpha {
                                present = true;
                                data = data
                                    .move_to((start as f64, y as f64))
                                    .horizontal_line_by((x - start) as f64)
                                    .vertical_line_by(1.0)
                                    .horizontal_line_by(-((x - start) as f64))
                                    .close();
                            }
                        }
                    }
                    if present {
                        svg.push(
                            Path::new()
                                .data(data)
                                .crisp_edges()
                                .fill(Paint::from_hex(&color))
                                .fill_opacity(f64::from(alpha) / 255.0 * opacity),
                        );
                    }
                }
            });
        }
    });
    for &[x, y, r, tile] in records {
        let s = f64::from(2.0 * r) / 48.0;
        let transform =
            Transform::matrix_unrounded([s, 0.0, 0.0, s, f64::from(x - r), f64::from(y - r)]);
        svg.scope(Group::new().transformed(transform), |svg| {
            svg.push(Use::href(&ids[tile as usize].id_text()))
        });
    }
}

#[cfg(feature = "pdf")]
pub(super) fn render_pdf(
    svg: &mut Scene,
    stroke: &Stroke,
    theme: super::RenderTheme,
    budget: &std::cell::Cell<usize>,
    point_budget: &std::cell::Cell<usize>,
) -> bool {
    use super::vector::Group;
    // The SVG interval does not broaden the separately evidenced PDF profile.
    if stroke
        .rendering
        .as_ref()
        .and_then(|r| r.style.pen_size)
        .is_none_or(|size| {
            ![8.0_f32.to_bits(), 9.1168_f32.to_bits(), 12.0_f32.to_bits()].contains(&size.to_bits())
        })
    {
        return false;
    }
    if !svg.retains_text() {
        return false;
    }
    let previous_budget = budget.get();
    let Some((records, argb)) = prepare(stroke, theme, budget, point_budget) else {
        return false;
    };
    if stroke
        .rendering
        .as_ref()
        .and_then(|r| r.style.pen_size)
        .is_some_and(|size| size.to_bits() == 12.0_f32.to_bits())
        && records.len() == 1
    {
        budget.set(previous_budget);
        return false;
    }
    let Some((form, bounds)) =
        super::legacy_marker_vector::pdf::prepare_pencil(&records, argb, ATLAS)
    else {
        budget.set(previous_budget);
        return false;
    };
    let id = svg.native_ink().register_legacy_marker(form, bounds);
    svg.scope(Group::new().native_ink_id(id), |svg| {
        emit_records(svg, &records, argb)
    });
    true
}
