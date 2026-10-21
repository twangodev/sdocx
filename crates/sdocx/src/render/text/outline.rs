use std::{collections::HashMap, sync::Arc};

use crate::fonts::{ResolvedFace, fontdb};

#[derive(Debug, Clone, Copy)]
pub(super) enum OutlineCommand {
    Move([f32; 2]),
    Line([f32; 2]),
    Quad([[f32; 2]; 2]),
    Cubic([[f32; 2]; 3]),
    Close,
}

pub(super) trait OutlineSink {
    fn move_to(&mut self, point: [f32; 2]);
    fn line_to(&mut self, point: [f32; 2]);
    fn quad_to(&mut self, control: [f32; 2], point: [f32; 2]);
    fn cubic_to(&mut self, first: [f32; 2], second: [f32; 2], point: [f32; 2]);
    fn close(&mut self);
}

pub(super) fn serialize(outline: &[OutlineCommand], sink: &mut impl OutlineSink) {
    for command in outline {
        match *command {
            OutlineCommand::Move(point) => sink.move_to(point),
            OutlineCommand::Line(point) => sink.line_to(point),
            OutlineCommand::Quad([control, point]) => sink.quad_to(control, point),
            OutlineCommand::Cubic([first, second, point]) => {
                sink.cubic_to(first, second, point);
            }
            OutlineCommand::Close => sink.close(),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub(super) enum OutlineError {
    #[error("invalid font face {0}")]
    InvalidFace(String),
    #[error("glyph {0} has no supported outline")]
    UnsupportedGlyph(u32),
}

#[derive(Default)]
pub(super) struct OutlineCache {
    outlines: HashMap<(fontdb::ID, u32, u32), Arc<[OutlineCommand]>>,
}

impl OutlineCache {
    pub fn get(
        &mut self,
        face: &ResolvedFace,
        glyph_id: u32,
    ) -> Result<Arc<[OutlineCommand]>, OutlineError> {
        let key = (face.id, face.index, glyph_id);
        if let Some(outline) = self.outlines.get(&key) {
            return Ok(outline.clone());
        }
        let parsed = rustybuzz::ttf_parser::Face::parse(face.bytes(), face.index)
            .map_err(|_| OutlineError::InvalidFace(face.family.clone()))?;
        let id = u16::try_from(glyph_id)
            .ok()
            .filter(|&id| id < parsed.number_of_glyphs())
            .map(rustybuzz::ttf_parser::GlyphId)
            .ok_or(OutlineError::UnsupportedGlyph(glyph_id))?;
        let tables = parsed.tables();
        if (tables.glyf.is_none() && tables.cff.is_none())
            || parsed.is_color_glyph(id)
            || parsed.glyph_svg_image(id).is_some()
            || parsed.glyph_raster_image(id, u16::MAX).is_some()
        {
            return Err(OutlineError::UnsupportedGlyph(glyph_id));
        }
        let mut outline = GlyphOutline::default();
        if parsed.outline_glyph(id, &mut outline).is_none()
            && (!outline.0.is_empty() || tables.glyf.is_some_and(|table| table.bbox(id).is_some()))
        {
            return Err(OutlineError::UnsupportedGlyph(glyph_id));
        }
        let outline: Arc<[OutlineCommand]> = outline.0.into();
        self.outlines.insert(key, outline.clone());
        Ok(outline)
    }
}

#[derive(Default)]
struct GlyphOutline(Vec<OutlineCommand>);

impl rustybuzz::ttf_parser::OutlineBuilder for GlyphOutline {
    fn move_to(&mut self, x: f32, y: f32) {
        self.0.push(OutlineCommand::Move([x, y]));
    }

    fn line_to(&mut self, x: f32, y: f32) {
        self.0.push(OutlineCommand::Line([x, y]));
    }

    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        self.0.push(OutlineCommand::Quad([[x1, y1], [x, y]]));
    }

    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        self.0
            .push(OutlineCommand::Cubic([[x1, y1], [x2, y2], [x, y]]));
    }

    fn close(&mut self) {
        self.0.push(OutlineCommand::Close);
    }
}

pub(super) fn transformed(
    outline: &[OutlineCommand],
    origin: [f64; 2],
    scale: f64,
    skew_x: f32,
) -> Option<Vec<OutlineCommand>> {
    let point = |[x, y]: [f32; 2]| {
        let dy = -f64::from(y) * scale;
        let point = [
            origin[0] + f64::from(x) * scale + f64::from(skew_x) * dy,
            origin[1] + dy,
        ];
        point
            .into_iter()
            .all(f64::is_finite)
            .then(|| point.map(|value| value as f32))
            .filter(|point| point.iter().all(|value| value.is_finite()))
    };
    outline
        .iter()
        .map(|command| match *command {
            OutlineCommand::Move(p) => Some(OutlineCommand::Move(point(p)?)),
            OutlineCommand::Line(p) => Some(OutlineCommand::Line(point(p)?)),
            OutlineCommand::Quad([p1, p]) => Some(OutlineCommand::Quad([point(p1)?, point(p)?])),
            OutlineCommand::Cubic([p1, p2, p]) => {
                Some(OutlineCommand::Cubic([point(p1)?, point(p2)?, point(p)?]))
            }
            OutlineCommand::Close => Some(OutlineCommand::Close),
        })
        .collect()
}
