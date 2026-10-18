use super::{FormulaMetadata, FormulaStroke};

const PEN_PREFIX: &str = "com.samsung.android.sdk.pen.pen.preload.";

/// Return the native cached-drawn-rect formula for a saved child when its
/// resolved pen name and size are available.
fn stroke_drawn_bounds(stroke: &FormulaStroke, resources_complete: bool) -> Option<[f32; 4]> {
    let base = &stroke.base;
    let rendering = stroke.stroke.rendering.as_ref()?;
    let pen_name_id = rendering
        .style
        .pen_name_id
        .filter(|id| *id != -1)
        .or(rendering.style.legacy_pen_name_id);
    if pen_name_id.is_some() && rendering.pen_name.is_none() && !resources_complete {
        return None;
    }
    let pen_size = rendering.style.pen_size.unwrap_or(0.0);
    let raw = [
        base.bbox.x_min as f32,
        base.bbox.y_min as f32,
        base.bbox.x_max as f32,
        base.bbox.y_max as f32,
    ];
    if !raw.into_iter().all(f32::is_finite) || !pen_size.is_finite() {
        return None;
    }

    let margin = match pen_category(rendering.pen_name.as_deref()) {
        0 | 4 | 8 | 9 | 12 => pen_size + 4.0,
        1 => pen_size * 0.5 + 20.0,
        3 | 5 => pen_size * 2.0 + 4.0,
        6 | 7 => pen_size * 9.0 + 4.0,
        10 => (pen_size * 0.5).mul_add(1.3_f32, 4.0),
        11 => pen_size * 35.0 + 4.0,
        _ => pen_size * 0.5 + 4.0,
    };
    if !margin.is_finite() {
        return None;
    }

    let expanded = [
        raw[0] - margin,
        raw[1] - margin,
        raw[2] + margin,
        raw[3] + margin,
    ];
    if !expanded.into_iter().all(f32::is_finite) {
        return None;
    }
    let rotation = base.rotation_degrees.unwrap_or(0.0) as f32;
    if !rotation.is_finite() {
        return None;
    }
    if rotation == 0.0 {
        return Some(expanded);
    }

    Some(rotated_bounds(expanded, rotation))
}

/// Native Formula::GetRectByStrokeList(true) union, including its empty result.
pub(crate) fn formula_drawn_bounds(
    formula: &FormulaMetadata,
    resources_complete: bool,
) -> Option<[f32; 4]> {
    let mut bounds = None;
    for stroke in formula.strokes.iter().chain(&formula.answer_strokes) {
        if !stroke.base.visible {
            continue;
        }
        let next = stroke_drawn_bounds(stroke, resources_complete)?;
        bounds = Some(union(bounds, next));
    }
    Some(bounds.unwrap_or([0.0; 4]))
}

fn pen_category(pen_name: Option<&str>) -> u8 {
    let Some(suffix) = pen_name.and_then(|name| name.strip_prefix(PEN_PREFIX)) else {
        return 2;
    };
    match suffix {
        "Marker" => 0,
        "Beautify" | "Beautify2" => 1,
        "Crayon" | "Crayon2" => 3,
        "BrushPen" => 4,
        "Pencil2" => 5,
        "Pencil3" => 6,
        "PatternPen" => 7,
        "InkPen" => 8,
        "ObliquePen" => 9,
        "OilBrush3" => 10,
        "ColoredPencil" => 11,
        "TapePen" => 12,
        _ => 2,
    }
}

fn rotated_bounds(rect: [f32; 4], rotation_degrees: f32) -> [f32; 4] {
    let center_x = (rect[0] + rect[2]) * 0.5;
    let center_y = (rect[1] + rect[3]) * 0.5;
    let radians = (f64::from(rotation_degrees) / 180.0) * std::f64::consts::PI;
    let (sin, cos) = radians.sin_cos();
    let mut out = [
        f32::INFINITY,
        f32::INFINITY,
        f32::NEG_INFINITY,
        f32::NEG_INFINITY,
    ];
    for (x, y) in [
        (rect[0], rect[1]),
        (rect[2], rect[1]),
        (rect[2], rect[3]),
        (rect[0], rect[3]),
    ] {
        let dx = f64::from(x - center_x);
        let dy = f64::from(y - center_y);
        let rotated_x = center_x + dx.mul_add(cos, -(dy * sin)) as f32;
        let rotated_y = center_y + dx.mul_add(sin, dy * cos) as f32;
        out[0] = out[0].min(rotated_x);
        out[1] = out[1].min(rotated_y);
        out[2] = out[2].max(rotated_x);
        out[3] = out[3].max(rotated_y);
    }
    out
}

fn union(left: Option<[f32; 4]>, right: [f32; 4]) -> [f32; 4] {
    let Some(left) = left else { return right };
    [
        left[0].min(right[0]),
        left[1].min(right[1]),
        left[2].max(right[2]),
        left[3].max(right[3]),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        BoundingBox, ObjectMetadata, Stroke, StrokeProperties, StrokeRendering, StrokeStyle,
    };

    const FIXTURE: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../conformance/formula-drawn-bounds.json"
    ));

    fn f32_value(hex: &str) -> f32 {
        let bytes = (0..4)
            .map(|index| u8::from_str_radix(&hex[index * 2..index * 2 + 2], 16).unwrap())
            .collect::<Vec<_>>();
        f32::from_bits(u32::from_le_bytes(bytes.try_into().unwrap()))
    }

    fn f32_array(value: &serde_json::Value) -> [f32; 4] {
        value
            .as_array()
            .unwrap()
            .iter()
            .map(|value| f32_value(value.as_str().unwrap()))
            .collect::<Vec<_>>()
            .try_into()
            .unwrap()
    }

    fn object_base(rect: [f32; 4], rotation: f32, visible: bool) -> ObjectMetadata {
        ObjectMetadata {
            format_version: 5500,
            uuid: String::new(),
            modified_time_raw: 0,
            bbox: BoundingBox {
                x_min: f64::from(rect[0]),
                y_min: f64::from(rect[1]),
                x_max: f64::from(rect[2]),
                y_max: f64::from(rect[3]),
            },
            replay_timestamp_raw: 0,
            resize_mode_raw: 0,
            rotatable: false,
            selectable: true,
            movable: true,
            visible,
            replayable: false,
            out_of_canvas_enabled: false,
            template: false,
            flip_enabled: false,
            float_drawn_rect: false,
            locked: false,
            removable: true,
            rotation_degrees: Some(f64::from(rotation)),
            property_mask: Vec::new(),
            field_mask: Vec::new(),
            fixed_trailing_data: Vec::new(),
            flexible_trailing_data: Vec::new(),
        }
    }

    fn stroke(
        name: Option<&str>,
        size: f32,
        rect: [f32; 4],
        rotation: f32,
        visible: bool,
    ) -> FormulaStroke {
        FormulaStroke {
            base: object_base(rect, rotation, visible),
            stroke: Stroke {
                rendering: Some(StrokeRendering {
                    metadata: None,
                    pen_name: name.map(|name| format!("{PEN_PREFIX}{name}")),
                    advanced_settings: None,
                    tool_type_raw: 0,
                    properties: StrokeProperties {
                        compressed: false,
                        replay_only: false,
                        stylus_channels: false,
                        eraser: false,
                        fixed_width: false,
                        millisecond_timestamps: false,
                        top_layer_pen: false,
                        alpha_lock: false,
                        binary_added: false,
                        generated: false,
                        fixed_opacity: false,
                        rainbow_effect: false,
                        straighten: false,
                        reveal_mode: false,
                    },
                    style: StrokeStyle {
                        pen_size: Some(size),
                        ..StrokeStyle::default()
                    },
                }),
                bbox: BoundingBox::default(),
                points: Vec::new(),
                pressures: Vec::new(),
                timestamps: Vec::new(),
                tilts: Vec::new(),
                orientations: Vec::new(),
                color: None,
                pen_width: size,
            },
            object_data: Vec::new(),
        }
    }

    fn formula(source: Vec<FormulaStroke>, answer: Vec<FormulaStroke>) -> FormulaMetadata {
        FormulaMetadata {
            base: object_base([0.0; 4], 0.0, true),
            has_trigonometry_calculation: false,
            plottable: false,
            latex: Vec::new(),
            latex_result_rect: None,
            nine_patch_rect: None,
            latex_image_media_id: None,
            latex_result: Vec::new(),
            angle_type: None,
            font_size: None,
            strokes: source,
            answer_strokes: answer,
            answer: None,
            answer_stroke_color: None,
            relative_original_formula_rect: None,
            relative_original_answer_rect: None,
            expression_type_raw: None,
            label_graphs: Vec::new(),
            substitution_latex: Vec::new(),
            property_mask: Vec::new(),
            field_mask: Vec::new(),
            fixed_trailing_data: Vec::new(),
            flexible_trailing_data: Vec::new(),
            trailing_data: Vec::new(),
        }
    }

    #[test]
    fn stroke_bounds_match_native_categories_and_rotations() {
        let fixture: serde_json::Value = serde_json::from_str(FIXTURE).unwrap();
        let raw_default = [10.0, 20.0, 110.0, 70.0];
        for case in fixture["stroke_rect_cases"].as_array().unwrap() {
            let raw = case["raw_rect_f32_bits"]
                .as_array()
                .map(|values| {
                    values
                        .iter()
                        .map(|value| f32_value(value.as_str().unwrap()))
                        .collect::<Vec<_>>()
                        .try_into()
                        .unwrap()
                })
                .unwrap_or(raw_default);
            let rotation = f32_value(case["rotation_f32_bits"].as_str().unwrap());
            let size = f32_value(case["size_f32_bits"].as_str().unwrap());
            let name = case["pen_name"].as_str().unwrap();
            let value = stroke(Some(name), size, raw, rotation, true);
            let actual = stroke_drawn_bounds(&value, true).unwrap();
            let expected = f32_array(&case["drawn_rect_f32_bits"]);
            assert_eq!(
                actual.map(f32::to_bits),
                expected.map(f32::to_bits),
                "{name} {rotation}"
            );
        }
    }

    #[test]
    fn formula_bounds_match_native_visible_union_and_empty_cases() {
        let fixture: serde_json::Value = serde_json::from_str(FIXTURE).unwrap();
        for case in fixture["formula_union_cases"].as_array().unwrap() {
            let source_rects = case["source_rects_f32_bits"].as_array().unwrap();
            let source_visible = case["source_visible"].as_array().unwrap();
            let answer_rects = case["answer_rects_f32_bits"].as_array().unwrap();
            let answer_visible = case["answer_visible"].as_array().unwrap();
            let marker_margin = 8.0_f32;
            let source = source_rects
                .iter()
                .zip(source_visible)
                .map(|(rect, visible)| {
                    let mut rect = f32_array(rect);
                    rect[0] += marker_margin;
                    rect[1] += marker_margin;
                    rect[2] -= marker_margin;
                    rect[3] -= marker_margin;
                    stroke(Some("Marker"), 4.0, rect, 0.0, visible.as_bool().unwrap())
                })
                .collect();
            let answer = answer_rects
                .iter()
                .zip(answer_visible)
                .map(|(rect, visible)| {
                    let mut rect = f32_array(rect);
                    rect[0] += marker_margin;
                    rect[1] += marker_margin;
                    rect[2] -= marker_margin;
                    rect[3] -= marker_margin;
                    stroke(Some("Marker"), 4.0, rect, 0.0, visible.as_bool().unwrap())
                })
                .collect();
            let actual = formula_drawn_bounds(&formula(source, answer), true);
            let expected = f32_array(&case["native_union_f32_bits"]);
            assert_eq!(
                actual.map(|bounds| bounds.map(f32::to_bits)),
                Some(expected.map(f32::to_bits)),
                "{}",
                case["name"]
            );
        }
    }

    #[test]
    fn missing_saved_pen_name_and_size_use_native_constructor_defaults_when_table_is_complete() {
        let mut child = stroke(None, 1.0, [10.0, 20.0, 110.0, 70.0], 0.0, true);
        let rendering = child.stroke.rendering.as_mut().unwrap();
        rendering.style.pen_size = None;
        rendering.style.pen_name_id = Some(12);
        assert_eq!(
            stroke_drawn_bounds(&child, true),
            Some([6.0, 16.0, 114.0, 74.0])
        );
        assert_eq!(stroke_drawn_bounds(&child, false), None);
    }
}
