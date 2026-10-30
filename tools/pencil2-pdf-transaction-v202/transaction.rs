//! Isolated PDF mutation experiment; not a renderer admission path.
use lopdf::{Document, Object, ObjectId, Stream, dictionary};
use std::fmt::Write;

const PIXELS: usize = 192 * 192;
const PER_PIXEL: usize = 256;
const PREFIX: &str = "0.005208333333333333 0 0 0.005208333333333333 0 0 cm\n";

type Frame = Vec<[u8; 4]>;

#[derive(Clone, Copy)]
struct Limits {
    frames: usize,
    bytes: usize,
    objects: usize,
}

// This accepts already-existing frames; it does not bound sampler allocations.
// No document, stream, resource or frame allocation happens before this check.
fn preflight(doc: &Document, fields: &[Vec<Frame>], limits: Limits) -> Option<()> {
    let frames = fields
        .iter()
        .try_fold(0usize, |n, f| n.checked_add(f.len()))?;
    if fields.iter().flatten().any(|f| f.len() != PIXELS) {
        return None;
    }
    let bytes = frames
        .checked_mul(PIXELS)?
        .checked_mul(PER_PIXEL)?
        .checked_add(fields.len().checked_mul(PREFIX.len())?)?;
    let objects = fields.len().checked_mul(256)?; // 255 states and one form per field
    let objects_u32 = u32::try_from(objects).ok()?;
    doc.max_id.checked_add(objects_u32)?;
    if doc.objects.keys().any(|id| id.0 > doc.max_id) {
        return None;
    }
    (frames <= limits.frames && bytes <= limits.bytes && objects <= limits.objects).then_some(())
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Failure {
    Stream(usize),
    Resource(usize, u8),
    Form(usize),
}

// Stage only new objects after preflight. Failures drop the staging document.
fn transaction(
    doc: &mut Document,
    fields: &[Vec<Frame>],
    limits: Limits,
    fail_at: Option<Failure>,
) -> Option<Vec<ObjectId>> {
    preflight(doc, fields, limits)?;
    let mut staged = Document::with_version(&doc.version);
    staged.max_id = doc.max_id;
    let mut forms = Vec::with_capacity(fields.len());
    for (index, frames) in fields.iter().enumerate() {
        let mut commands = String::from(PREFIX);
        for frame in frames {
            for (i, px) in frame.iter().enumerate() {
                let a = px[3];
                if a != 0 {
                    let rgb = [px[0], px[1], px[2]]
                        .map(|v| (f64::from(v) * 255. / f64::from(a)).round().min(255.) / 255.);
                    let before = commands.len();
                    writeln!(
                        commands,
                        "/A{a} gs {} {} {} rg {} {} 1 1 re f",
                        rgb[0],
                        rgb[1],
                        rgb[2],
                        i % 192,
                        191 - i / 192
                    )
                    .ok()?;
                    if commands.len().checked_sub(before)? > PER_PIXEL
                        || fail_at == Some(Failure::Stream(index))
                    {
                        return None;
                    }
                }
            }
        }
        let mut states = lopdf::Dictionary::new();
        for a in 1..=255 {
            let id = staged.add_object(
                dictionary! {"Type" => "ExtGState", "ca" => Object::Real(a as f32 / 255.)},
            );
            states.set(format!("A{a}"), id);
            if fail_at == Some(Failure::Resource(index, a)) {
                return None;
            }
        }
        let form = Stream::new(
            dictionary! {"Type" => "XObject", "Subtype" => "Form", "BBox" => vec![0.into(),0.into(),1.into(),1.into()], "Group" => dictionary! {"S" => "Transparency", "CS" => "DeviceRGB", "I" => true}, "Resources" => dictionary! {"ExtGState" => states}},
            commands.into_bytes(),
        );
        forms.push(staged.add_object(form));
        if fail_at == Some(Failure::Form(index)) {
            return None;
        }
    }
    doc.objects.append(&mut staged.objects);
    doc.max_id = staged.max_id;
    Some(forms)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn limits() -> Limits {
        Limits {
            frames: 2,
            bytes: 2 * PIXELS * PER_PIXEL + 2 * PREFIX.len(),
            objects: 512,
        }
    }
    fn scene() -> (Document, ObjectId) {
        let mut doc = Document::with_version("1.5");
        let marker = doc.add_object(Stream::new(
            dictionary! {"marker" => "prior"},
            b"prior scene content".to_vec(),
        ));
        doc.trailer.set("Root", marker);
        (doc, marker)
    }
    fn fields() -> Vec<Vec<Frame>> {
        let mut a = vec![[0; 4]; PIXELS];
        a[0] = [10, 20, 30, 40];
        let mut b = vec![[0; 4]; PIXELS];
        b[PIXELS - 1] = [20, 30, 40, 50];
        vec![vec![a], vec![b]]
    }
    fn unchanged(doc: &Document, before: &Document) {
        assert_eq!(doc.objects, before.objects);
        assert_eq!(doc.max_id, before.max_id);
        assert_eq!(doc.trailer, before.trailer);
        assert_eq!(format!("{doc:?}"), format!("{before:?}"));
    }
    #[test]
    fn rejection_and_mid_transaction_failure_preserve_existing_document() {
        let (mut doc, _) = scene();
        let before = doc.clone();
        let f = fields();
        let l = limits();
        for limit in [
            Limits { frames: 1, ..l },
            Limits {
                bytes: l.bytes - 1,
                ..l
            },
            Limits { objects: 511, ..l },
        ] {
            assert!(transaction(&mut doc, &f, limit, None).is_none());
            unchanged(&doc, &before);
        }
        assert!(transaction(&mut doc, &f, l, Some(Failure::Form(0))).is_none());
        unchanged(&doc, &before);
        assert!(transaction(&mut doc, &f, l, Some(Failure::Form(1))).is_none());
        unchanged(&doc, &before);
        for failure in [
            Failure::Stream(0),
            Failure::Stream(1),
            Failure::Resource(0, 1),
            Failure::Resource(0, 128),
            Failure::Resource(1, 255),
        ] {
            assert!(transaction(&mut doc, &f, l, Some(failure)).is_none());
            unchanged(&doc, &before);
        }
        let malformed = vec![vec![vec![[0; 4]; 1]]];
        assert!(transaction(&mut doc, &malformed, l, None).is_none());
        unchanged(&doc, &before);
        doc.max_id = u32::MAX;
        let overflow = doc.clone();
        assert!(transaction(&mut doc, &f, l, None).is_none());
        unchanged(&doc, &overflow);
    }
    #[test]
    fn dense_stream_obeys_checked_bound() {
        let (mut doc, _) = scene();
        let frame: Frame = (0..PIXELS)
            .map(|i| {
                let a = (i % 255 + 1) as u8;
                [i as u8, (i >> 2) as u8, (i >> 5) as u8, a]
            })
            .collect();
        let l = Limits {
            frames: 1,
            bytes: PREFIX.len() + PIXELS * PER_PIXEL,
            objects: 256,
        };
        let id = transaction(&mut doc, &[vec![frame]], l, None).unwrap()[0];
        let stream = doc.objects[&id].as_stream().unwrap();
        assert_eq!(
            stream.content.iter().filter(|&&b| b == b'\n').count(),
            PIXELS + 1
        );
        assert!(stream.content.len() <= l.bytes);
        for line in std::str::from_utf8(&stream.content)
            .unwrap()
            .lines()
            .skip(1)
        {
            assert!(line.len() + 1 <= PER_PIXEL);
        }
    }
    #[test]
    fn accepted_two_forms_preserve_prior_object_and_order() {
        let (mut doc, marker) = scene();
        let prior = doc.objects[&marker].clone();
        let forms = transaction(&mut doc, &fields(), limits(), None).unwrap();
        assert_eq!(forms.len(), 2);
        assert!(forms[0] < forms[1]);
        assert_eq!(doc.objects[&marker], prior);
        assert_eq!(
            doc.trailer.get(b"Root").unwrap(),
            &Object::Reference(marker)
        );
        for (id, pixel) in forms.iter().zip(["0 191 1 1 re f", "191 0 1 1 re f"]) {
            let stream = doc.objects[id].as_stream().unwrap();
            assert!(
                std::str::from_utf8(&stream.content)
                    .unwrap()
                    .contains(pixel)
            );
            let states = stream
                .dict
                .get(b"Resources")
                .unwrap()
                .as_dict()
                .unwrap()
                .get(b"ExtGState")
                .unwrap()
                .as_dict()
                .unwrap();
            assert_eq!(states.len(), 255);
            for a in 1..=255u8 {
                let reference = states
                    .get(format!("A{a}").as_bytes())
                    .unwrap()
                    .as_reference()
                    .unwrap();
                let state = doc.objects[&reference].as_dict().unwrap();
                assert_eq!(
                    state.get(b"Type").unwrap(),
                    &Object::Name(b"ExtGState".to_vec())
                );
                assert_eq!(state.get(b"ca").unwrap(), &Object::Real(a as f32 / 255.));
            }
            assert!(stream.content.len() <= PREFIX.len() + PIXELS * PER_PIXEL);
        }
    }
}
