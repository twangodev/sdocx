#[cfg(test)]
pub(crate) fn pencil2_control_form(frames: &[Vec<[u8; 4]>]) -> Option<Prepared> {
    use std::fmt::Write as _;
    if frames.len() > 168 { return None; }
    let mut w = Writer { doc: Document::with_version("1.7"), objects: 0, bytes: 0, commands: 0, width: 192., height: 192. };
    let mut states = lopdf::Dictionary::new();
    for alpha in 1..=255 {
        let id = w.add(dictionary! {"Type" => "ExtGState", "ca" => Object::Real(alpha as f32 / 255.)})?;
        states.set(format!("A{alpha}"), id);
    }
    let mut commands = String::from("0.005208333333333333 0 0 0.005208333333333333 0 0 cm\n");
    for frame in frames {
        if frame.len() != 192 * 192 { return None; }
        for (i, p) in frame.iter().enumerate() {
            let a = p[3]; if a == 0 { continue; }
            let rgb = [p[0], p[1], p[2]].map(|v| (f64::from(v)*255./f64::from(a)).round().min(255.) / 255.);
            writeln!(commands, "/A{a} gs {} {} {} rg {} {} 1 1 re f", rgb[0], rgb[1], rgb[2], i%192, 191-i/192).ok()?;
            if commands.len() > 2_000_000 { return None; }
        }
    }
    w.bytes = commands.len();
    let root = w.add(Stream::new(dictionary! {"Type"=>"XObject", "Subtype"=>"Form", "FormType"=>1, "BBox"=>vec![0.into(),0.into(),1.into(),1.into()], "Group"=>dictionary!{"S"=>"Transparency","CS"=>"DeviceRGB","I"=>true}, "Resources"=>dictionary!{"ExtGState"=>states}}, commands.into_bytes()))?;
    Some(Prepared { writer: w, root })
}
