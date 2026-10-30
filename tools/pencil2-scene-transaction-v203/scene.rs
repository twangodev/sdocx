use crate::{Stroke, ink::pencil2};
use lopdf::{Document, Object, ObjectId, Stream, dictionary};
use std::fmt::Write;

const PIXELS: usize = 192 * 192;
const PER_PIXEL: usize = 256;
#[derive(Clone, Copy)]
pub struct Grid { pub width: usize, pub height: usize, pub scale: f32, pub translation: [f32; 2], pub grain_offset: [f32; 2] }
pub struct Item<'a> { pub stroke: &'a Stroke, pub grid: Grid }
#[derive(Clone, Copy)]
pub struct Limits { pub source_points: usize, pub frames: usize, pub work: usize, pub svg_bytes: usize, pub pdf_bytes: usize, pub pdf_objects: usize }

fn preflight(items: &[Item<'_>], doc: &Document, limits: Limits, mut on_plan: impl FnMut()) -> Option<()> {
    let mut points = 0usize;
    let mut frames = 0usize;
    let mut fields = 0usize;
    if doc.objects.keys().any(|id| id.0 > doc.max_id) { return None; }
    for item in items {
        let g = item.grid;
        if g.width != 192 || g.height != 192 || ![1., 2.].contains(&g.scale) || g.translation.into_iter().chain(g.grain_offset).any(|v| !v.is_finite()) { return None; }
        points = points.checked_add(item.stroke.points.len())?;
        fields = fields.checked_add(1)?;
        if points > limits.source_points { return None; }
        let objects = fields.checked_mul(256)?;
        if objects > limits.pdf_objects || doc.max_id.checked_add(u32::try_from(objects).ok()?).is_none() { return None; }
        on_plan();
        frames = frames.checked_add(pencil2::source::plan_stroke(item.stroke, 1. / g.scale).ok()?)?;
        let pixels = frames.checked_mul(PIXELS)?;
        let work = pixels.checked_mul(2)?.checked_add(frames)?;
        let bytes = pixels.checked_mul(PER_PIXEL)?.checked_add(fields.checked_mul(256)?)?;
        if frames > limits.frames || work > limits.work || bytes > limits.svg_bytes || bytes > limits.pdf_bytes { return None; }
    }
    Some(())
}

// Diagnostic hook counts actual plan invocations without changing their results.
pub fn observed_preflight(items: &[Item<'_>], doc: &Document, limits: Limits) -> (bool, usize) {
    let mut planned = 0;
    let result = preflight(items, doc, limits, || planned += 1).is_some();
    (result, planned)
}

// Staging is detached from both destinations. A failure discards all staged output.
pub fn transaction(items: &[Item<'_>], svg: &mut Vec<String>, doc: &mut Document, limits: Limits, fail_after: Option<usize>) -> Option<Vec<ObjectId>> {
    preflight(items, doc, limits, || {})?;
    let mut staged_doc = Document::with_version(&doc.version);
    staged_doc.max_id = doc.max_id;
    let mut staged_svg = Vec::new();
    let mut forms = Vec::new();
    for (index, item) in items.iter().enumerate() {
        let g = item.grid;
        let records = pencil2::prepare_stroke(item.stroke, 1. / g.scale).ok()?;
        let ctx = pencil2::device_raster::Context { scale: g.scale, translation: g.translation, grain_offset: g.grain_offset, ..Default::default() };
        let mut svg_field = String::new();
        let mut commands = String::from("0.005208333333333333 0 0 0.005208333333333333 0 0 cm\n");
        for record in records {
            let frame = pencil2::device_raster::prepare(&[record.lanes], ctx).ok()?.render(None)?;
            if frame.len() != PIXELS { return None; }
            for (i, p) in frame.iter().enumerate() {
                let a = p[3]; if a == 0 { continue; }
                let rgb = [p[0],p[1],p[2]].map(|v| (f64::from(v)*255./f64::from(a)).round().min(255.));
                let before = svg_field.len();
                write!(svg_field,"<rect x=\"{}\" y=\"{}\" width=\"1\" height=\"1\" fill=\"rgb({},{},{})\" fill-opacity=\"{}\"/>",i%192,i/192,rgb[0],rgb[1],rgb[2],f64::from(a)/255.).ok()?;
                if svg_field.len().checked_sub(before)? > PER_PIXEL { return None; }
                let before = commands.len();
                writeln!(commands,"/A{a} gs {} {} {} rg {} {} 1 1 re f",rgb[0]/255.,rgb[1]/255.,rgb[2]/255.,i%192,191-i/192).ok()?;
                if commands.len().checked_sub(before)? > PER_PIXEL { return None; }
            }
        }
        let mut states = lopdf::Dictionary::new();
        for a in 1..=255 { let id=staged_doc.add_object(dictionary! {"Type" => "ExtGState", "ca" => Object::Real(a as f32 / 255.)}); states.set(format!("A{a}"),id); }
        forms.push(staged_doc.add_object(Stream::new(dictionary! {"Type" => "XObject", "Subtype" => "Form", "BBox" => vec![0.into(),0.into(),1.into(),1.into()], "Group" => dictionary! {"S" => "Transparency", "CS" => "DeviceRGB", "I" => true}, "Resources" => dictionary! {"ExtGState" => states}},commands.into_bytes())));
        staged_svg.push(svg_field);
        if fail_after == Some(index) { return None; }
    }
    doc.objects.append(&mut staged_doc.objects); doc.max_id = staged_doc.max_id; svg.extend(staged_svg); Some(forms)
}
