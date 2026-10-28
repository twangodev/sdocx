use lopdf::{Document, Object, Stream, dictionary};
use std::io::{self, Write};
const MAX_BYTES: usize = 400_000;
const MAX_ITEMS: usize = 512;
const MAX_COMMANDS: usize = 12_000;
const MAX_OBJECTS: usize = 2_000;
const MAX_STAGE: usize = 1_000_000;
#[derive(Clone)]
pub(crate) struct Plan {
    pub(crate) width: f32,
    pub(crate) height: f32,
    pub(crate) draws: Vec<Draw>,
}
#[derive(Clone)]
pub(crate) struct Draw {
    pub(crate) path: String,
    pub(crate) color: [f32; 3],
    pub(crate) alpha: f32,
    pub(crate) gradient: Option<[f32; 4]>,
    pub(crate) owner: Option<String>,
    pub(crate) even_odd: bool,
}
struct LimitedOutput {
    bytes: Vec<u8>,
    limit: usize,
}
impl Write for LimitedOutput {
    fn write(&mut self, data: &[u8]) -> io::Result<usize> {
        if self
            .bytes
            .len()
            .checked_add(data.len())
            .is_none_or(|n| n > self.limit)
        {
            return Err(io::Error::other("serialized PDF budget"));
        }
        self.bytes.extend_from_slice(data);
        Ok(data.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
#[derive(Clone, Debug)]
struct Writer {
    doc: Document,
    objects: usize,
    bytes: usize,
    commands: usize,
    width: f32,
    height: f32,
}
impl Writer {
    fn add(&mut self, object: impl Into<Object>) -> Option<lopdf::ObjectId> {
        self.objects = self.objects.checked_add(1)?;
        if self.objects > MAX_OBJECTS {
            return None;
        }
        Some(self.doc.add_object(object))
    }
    fn form(
        &mut self,
        content: String,
        res: lopdf::Dictionary,
        cs: &str,
    ) -> Option<lopdf::ObjectId> {
        self.bytes = self.bytes.checked_add(content.len())?;
        if self.bytes > MAX_STAGE {
            return None;
        }
        self.add(Stream::new(dictionary!{"Type"=>"XObject","Subtype"=>"Form","FormType"=>1,"BBox"=>vec![0.into(),0.into(),self.width.into(),self.height.into()],"Group"=>dictionary!{"S"=>"Transparency","CS"=>Object::Name(cs.as_bytes().to_vec()),"I"=>true},"Resources"=>res},content.into_bytes()))
    }
    fn path(&mut self, s: &str) -> Option<String> {
        let mut out = String::new();
        for line in s.lines() {
            self.commands = self.commands.checked_add(1)?;
            if self.commands > MAX_COMMANDS {
                return None;
            }
            let t: Vec<_> = line.split_whitespace().collect();
            match t.as_slice() {
                ["h"] => out.push_str("h\n"),
                [x, y, op] if *op == "m" || *op == "l" => {
                    for n in [x, y] {
                        let v = n.parse::<f64>().ok()?;
                        if !v.is_finite()
                            || v.abs() > 1_000_000.
                            || n.len() > 32
                            || n.contains(['e', 'E'])
                        {
                            return None;
                        }
                    }
                    out.push_str(line);
                    out.push('\n')
                }
                _ => return None,
            }
        }
        Some(out)
    }
}
#[derive(Default, Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct PageBudget {
    work: usize,
    objects: usize,
    commands: usize,
    bytes: usize,
}
impl PageBudget {
    pub(super) fn charge_attempts_from(&mut self, attempted: Self) {
        self.work = attempted.work;
    }
}
#[derive(Clone, Debug)]
pub(crate) struct Prepared {
    writer: Writer,
    root: lopdf::ObjectId,
}
pub(crate) fn prepare(plan: &Plan, bounds: [f32; 4], budget: &mut PageBudget) -> Option<Prepared> {
    if plan.draws.len() > MAX_ITEMS
        || plan
            .draws
            .iter()
            .try_fold(0usize, |n, d| {
                n.checked_add(d.path.len())?
                    .checked_add(d.owner.as_ref().map_or(0, String::len))
            })
            .is_none_or(|n| n > MAX_BYTES)
    {
        return None;
    }
    let attempted = plan.draws.iter().try_fold(0usize, |n, d| {
        n.checked_add(1)?
            .checked_add(d.path.lines().count())?
            .checked_add(d.owner.as_ref().map_or(0, |p| p.lines().count()))
    })?;
    let remaining = MAX_COMMANDS.saturating_sub(budget.work);
    if attempted > remaining {
        budget.work = MAX_COMMANDS;
        return None;
    }
    budget.work = budget.work.checked_add(attempted)?;
    let mut w = Writer {
        doc: Document::with_version("1.7"),
        objects: 0,
        bytes: 0,
        commands: 0,
        width: plan.width,
        height: plan.height,
    };
    let mut states = lopdf::Dictionary::new();
    let mut xobjects = lopdf::Dictionary::new();
    if bounds.iter().any(|v| !v.is_finite()) {
        return None;
    }
    let [xmin, ymin, xmax, ymax] = [bounds[0], bounds[1], bounds[2], bounds[3]];
    let width = xmax - xmin;
    let height = ymax - ymin;
    if width <= 0. || height <= 0. {
        return None;
    }
    let mut content = format!(
        "q 1 0 0 -1 0 1 cm {} 0 0 {} {} {} cm\n",
        1.0 / width,
        1.0 / height,
        -xmin / width,
        -ymin / height
    );
    for (i, item) in plan.draws.iter().enumerate() {
        let p = w.path(&item.path)?;
        let rgb = item.color;
        let key = format!("A{i}");
        if item.gradient.is_none() {
            let alpha = item.alpha;
            if !(0.0..=1.0).contains(&alpha) {
                return None;
            }
            let state = w.add(dictionary! {"Type"=>"ExtGState","ca"=>alpha,"CA"=>alpha})?;
            states.set(key.as_bytes(), state);
            content.push_str(&format!(
                "q /{key} gs {} {} {} rg\n{p}f\nQ\n",
                rgb[0], rgb[1], rgb[2]
            ));
            continue;
        }
        let coords = item
            .gradient?
            .into_iter()
            .map(Object::Real)
            .collect::<Vec<_>>();
        let shading=w.add(dictionary!{"ShadingType"=>2,"ColorSpace"=>"DeviceGray","Coords"=>coords,"Domain"=>vec![0.into(),1.into()],"Function"=>dictionary!{"FunctionType"=>2,"Domain"=>vec![0.into(),1.into()],"C0"=>vec![0.into()],"C1"=>vec![1.into()],"N"=>1},"Extend"=>vec![true.into(),true.into()]})?;
        let gray = w.form(
            "/S sh".into(),
            dictionary! {"Shading"=>dictionary!{"S"=>shading}},
            "DeviceGray",
        )?;
        let state = w.add(
            dictionary! {"Type"=>"ExtGState","SMask"=>dictionary!{"S"=>"Luminosity","G"=>gray}},
        )?;
        states.set(key.as_bytes(), state);
        let rule = if item.even_odd { "W*" } else { "W" };
        let paint = format!(
            "q /{key} gs {} {} {} rg\n{p}{rule} n 0 0 {} {} re f\nQ\n",
            rgb[0], rgb[1], rgb[2], plan.width, plan.height
        );
        if let Some(owner) = item.owner.as_deref() {
            let owner = w.path(owner)?;
            let mask = w.form(
                format!(
                    "1 g 0 0 {} {} re f\n0 g\n{owner}f\n",
                    plan.width, plan.height
                ),
                dictionary! {},
                "DeviceGray",
            )?;
            let maskstate = w.add(
                dictionary! {"Type"=>"ExtGState","SMask"=>dictionary!{"S"=>"Luminosity","G"=>mask}},
            )?;
            let mut capstate = lopdf::Dictionary::new();
            capstate.set(key.as_bytes(), state);
            let cap = w.form(paint, dictionary! {"ExtGState"=>capstate}, "DeviceRGB")?;
            let mk = format!("M{i}");
            let ck = format!("C{i}");
            states.set(mk.as_bytes(), maskstate);
            xobjects.set(ck.as_bytes(), cap);
            content.push_str(&format!("q /{mk} gs /{ck} Do Q\n"));
        } else {
            content.push_str(&paint)
        }
        if content.len() > MAX_STAGE {
            return None;
        }
    }
    content.push_str("Q\n");
    w.bytes = w.bytes.checked_add(content.len())?;
    if w.bytes > MAX_STAGE {
        return None;
    }
    let form = w.add(Stream::new(dictionary!{"Type"=>"XObject","Subtype"=>"Form","FormType"=>1,"BBox"=>vec![0.into(),0.into(),1.into(),1.into()],"Resources"=>dictionary!{"ExtGState"=>states,"XObject"=>xobjects}},content.into_bytes()))?;
    let mut probe = w.doc.clone();
    probe.trailer.set("Root", form);
    let mut serialized = LimitedOutput {
        bytes: Vec::new(),
        limit: MAX_STAGE,
    };
    probe.save_to(&mut serialized).ok()?;
    let estimated = w.bytes.max(serialized.bytes.len());
    let next = PageBudget {
        work: budget.work,
        objects: budget.objects.checked_add(w.objects)?,
        commands: budget.commands.checked_add(w.commands)?,
        bytes: budget.bytes.checked_add(estimated)?,
    };
    if next.objects > MAX_OBJECTS || next.commands > MAX_COMMANDS || next.bytes > MAX_STAGE {
        return None;
    }
    *budget = next;
    Some(Prepared {
        writer: w,
        root: form,
    })
}
impl Prepared {
    pub(crate) fn detached(&self) -> (Document, lopdf::ObjectId) {
        (self.writer.doc.clone(), self.root)
    }
}
