use super::*;
use crate::{Point, StrokeStyle};
use crate::ink::pencil2::{self, device_raster};

fn stroke(second: bool) -> Stroke {
    let mut style = serde_json::to_value(StrokeStyle::default()).unwrap();
    let (size, argb) = if second {(10., 0xc0cc3344u32)} else {(2., 0x80336699u32)};
    for (k,v) in [("pen_size",serde_json::json!(size)),("color_argb",serde_json::json!(argb)),("size_level",serde_json::json!(0)),("stroke_type_raw",serde_json::json!(0)),("color_type_raw",serde_json::json!(0))] { style[k]=v; }
    let props=serde_json::json!({"compressed":true,"stylus_channels":true,"replay_only":false,"eraser":false,"fixed_width":false,"millisecond_timestamps":false,"top_layer_pen":false,"alpha_lock":false,"binary_added":false,"generated":false,"fixed_opacity":false,"rainbow_effect":false,"straighten":false,"reveal_mode":false});
    let rendering=serde_json::from_value(serde_json::json!({"metadata":null,"pen_name":"com.samsung.android.sdk.pen.pen.preload.Pencil2","advanced_settings":"14;","tool_type_raw":2,"properties":props,"style":style})).unwrap();
    Stroke { bbox: BoundingBox::default(), points: if second {vec![Point{x:0.,y:2.},Point{x:10.,y:2.},Point{x:20.,y:4.},Point{x:30.,y:12.}]} else {vec![Point{x:0.,y:0.},Point{x:10.,y:0.},Point{x:20.,y:0.},Point{x:30.,y:10.}]}, pressures:if second {vec![0.9,0.7,0.5,0.3]} else {vec![0.25,0.5,0.75,1.]}, tilts:vec![0.4;4], orientations:vec![0.;4], timestamps:vec![10,20,30,40], color:Some(if second {Color{r:0xcc,g:0x33,b:0x44}} else {Color{r:0x33,g:0x66,b:0x99}}), pen_width:size, rendering:Some(rendering) }
}

fn transaction(scene: &mut Scene, inputs: &[(&Stroke,[f32;2],[f32;2])], fail_after: Option<usize>) -> Option<()> {
    let mut frames_total = 0usize;
    for (stroke, tr, grain) in inputs {
        if tr.iter().chain(grain).any(|v| !v.is_finite()) { return None; }
        frames_total = frames_total.checked_add(pencil2::plan_stroke(stroke,1.).ok()?)?;
        if frames_total > 168 || frames_total.checked_mul(192*192)?.checked_mul(256)? > 1_600_000_000 { return None; }
    }
    let mut staged = Vec::new();
    for (index, (stroke, tr, grain)) in inputs.iter().enumerate() {
        let records = pencil2::prepare_stroke(stroke,1.).ok()?;
        let context = device_raster::Context {translation:*tr,grain_offset:*grain,..Default::default()};
        let frames: Vec<_> = records.iter().map(|r| device_raster::prepare(&[r.lanes],context).ok()?.render(None)).collect::<Option<_>>()?;
        let form=legacy_marker_vector::pdf::pencil2_control_form(&frames)?;
        staged.push((frames, form));
        if fail_after == Some(index) { return None; }
    }
    for (frames,form) in staged {
        let bounds=BoundingBox{x_min:0.,y_min:0.,x_max:192.,y_max:192.};
        let id=scene.native_ink().register_legacy_marker(form,bounds);
        scene.scope(Group::new().native_ink_id(id), |scene| {
            for frame in frames { for (i,p) in frame.iter().enumerate() {
                let a=p[3]; if a==0 {continue;}
                let rgb=[p[0],p[1],p[2]].map(|v| (f64::from(v)*255./f64::from(a)).round().min(255.) as u8);
                scene.push(Rectangle::new().x((i%192) as f64).y((i/192) as f64).width(1).height(1).fill(Paint::from_hex(&format!("#{:02x}{:02x}{:02x}",rgb[0],rgb[1],rgb[2]))).fill_opacity(f64::from(a)/255.));
            }}
        });
    }
    Some(())
}

fn scene() -> Scene {
    let mut scene=Scene::new(Svg::new().width(192).height(192).view_box(ViewBox::unrounded(0.,0.,192.,192.)));
    scene.retain_text();
    scene.push(Rectangle::new().width(192).height(192).fill(Paint::from_hex("#4080bf")));
    let a=stroke(false);
    transaction(&mut scene,&[(&a,[6.,9.],[6.,9.])],None).unwrap();
    scene
}
fn snapshot(mut scene: Scene) -> (String,String) {let ink=scene.take_native_ink(); (scene.finish(), format!("{ink:?}"))}
#[test]
fn real_registry_stages_two_strokes_and_rolls_back() {
    let a=stroke(false); let b=stroke(true);
    let inputs=[(&a,[6.,9.],[6.,9.]),(&b,[12.,12.],[12.,13.999998])];
    let before=snapshot(scene());
    for failure in [Some(0),Some(1)] {
        let mut s=scene(); assert!(transaction(&mut s,&inputs,failure).is_none()); assert_eq!(snapshot(s),before);
    }
    let mut bad=b.clone(); bad.pressures.pop(); let mut s=scene();
    assert!(transaction(&mut s,&[inputs[0],(&bad,[12.,12.],[12.,13.999998])],None).is_none()); assert_eq!(snapshot(s),before);
    let mut s=scene(); transaction(&mut s,&inputs,None).unwrap();
    let ink=s.take_native_ink(); assert_eq!(ink.len(),3);
    for (_,entry) in ink.iter() { let NativeInkPayload::LegacyMarker {form}= &entry.payload else {panic!("wrong payload")}; let (doc,root)=form.detached(); assert!(doc.objects.contains_key(&root)); }
    let text=s.take_native_text(); let text_error=s.take_native_text_error();
    let out=s.finish(); assert!(out.find("sdocx-native-ink-0").unwrap() < out.find("sdocx-native-ink-1").unwrap()); assert!(out.find("sdocx-native-ink-1").unwrap() < out.find("sdocx-native-ink-2").unwrap());
    let retained=RenderedScene {page:RenderedPage::new(0,192,192,out), text, ink, text_error};
    let pdf=crate::pdf::render_native_scene_pdf_for_test(&retained,&crate::PdfOptions::default()).unwrap();
    let doc=lopdf::Document::load_mem(&pdf).unwrap();
    let page=*doc.get_pages().values().next().unwrap();
    let content=String::from_utf8(doc.get_page_content(page).unwrap()).unwrap();
    let offsets=["/x0 Do", "/x1 Do", "/x2 Do"].map(|name| content.find(name).unwrap());
    assert!(offsets[0] < offsets[1] && offsets[1] < offsets[2]);
    for name in ["x0", "x1", "x2"] {
        assert!(content.contains(&format!("q 144 0 0 144 0 0 cm/{name} Do\nQ")));
    }
    let (direct, ids)=doc.get_page_resources(page).unwrap();
    let resources=direct.or_else(|| ids.iter().find_map(|id| doc.get_dictionary(*id).ok())).unwrap();
    let xobjects=resources.get(b"XObject").unwrap().as_dict().unwrap();
    let mut contents=Vec::new();
    for name in [b"x0",b"x1",b"x2"] {
        let id=xobjects.get(name).unwrap().as_reference().unwrap();
        let form=doc.get_object(id).unwrap().as_stream().unwrap();
        assert_eq!(form.dict.get(b"Subtype").unwrap().as_name().unwrap(),b"Form");
        assert_eq!(form.dict.get(b"BBox").unwrap().as_array().unwrap(), &vec![0.into(),0.into(),1.into(),1.into()]);
        let states=form.dict.get(b"Resources").unwrap().as_dict().unwrap().get(b"ExtGState").unwrap().as_dict().unwrap();
        assert_eq!(states.len(),255);
        for alpha in 1..=255 {
            let key=format!("A{alpha}");
            let state=states.get(key.as_bytes()).unwrap();
            let dict=doc.get_object(state.as_reference().unwrap()).unwrap().as_dict().unwrap();
            let actual=dict.get(b"ca").unwrap().as_float().unwrap();
            assert_eq!(actual, alpha as f32 / 255.);
        }
        let bytes=form.decompressed_content().unwrap_or_else(|_| form.content.clone());
        assert!(bytes.windows(5).any(|w| w == b" re f"));
        let commands=String::from_utf8(bytes.clone()).unwrap();
        for line in commands.lines().skip(1) {
            let name=line.split_whitespace().next().unwrap().strip_prefix('/').unwrap();
            assert!(states.get(name.as_bytes()).is_ok(), "missing alpha state {name}");
        }
        contents.push(bytes);
    }
    assert_eq!(contents[0], contents[1]);
    assert_ne!(contents[1], contents[2]);
} 
