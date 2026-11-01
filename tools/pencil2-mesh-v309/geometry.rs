use super::prepare_stroke;
use crate::Stroke;
#[test]
fn serialized_geometry_dump() {
    let data: serde_json::Value = serde_json::from_slice(&std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/../../conformance/fountain-v14-saved-scene-inputs.json")).unwrap()).unwrap();
    let mut stroke: Stroke = serde_json::from_value(data["strokes"][2]["stroke"].clone()).unwrap();
    stroke.pen_width = f32::from_bits(0x4111de6a) as _;
    stroke.color = None;
    stroke.rendering.as_mut().unwrap().style.color_argb = Some(4294425088);
    stroke.rendering.as_mut().unwrap().pen_name = Some("com.samsung.android.sdk.pen.pen.preload.Pencil2".into());
    {
    let source_rendering = stroke.rendering.as_ref().unwrap();
    let mut style = serde_json::to_value(crate::StrokeStyle::default()).unwrap();
    style["pen_size"] = serde_json::json!(stroke.pen_width);
    style["color_argb"] = serde_json::json!(source_rendering.style.color_argb);
    style["size_level"] = serde_json::json!(0); style["stroke_type_raw"] = serde_json::json!(0); style["color_type_raw"] = serde_json::json!(0);
    let props = serde_json::json!({"compressed":true,"stylus_channels":true,"replay_only":false,"eraser":false,"fixed_width":false,"millisecond_timestamps":false,"top_layer_pen":false,"alpha_lock":false,"binary_added":false,"generated":false,"fixed_opacity":false,"rainbow_effect":false,"straighten":false,"reveal_mode":false});
    stroke.rendering = Some(serde_json::from_value(serde_json::json!({"metadata":null,"pen_name":"com.samsung.android.sdk.pen.pen.preload.Pencil2","advanced_settings":"14;","tool_type_raw":2,"properties":props,"style":style})).unwrap());
    }
let records = prepare_stroke(&stroke, 1.).unwrap();
    let bytes: Vec<u8> = records.iter().flat_map(|r| r.lanes.iter().flat_map(|f| f.to_le_bytes())).collect();
    std::fs::write(std::path::Path::new(&std::env::var("PENCIL_MESH_GEOMETRY_OUT").unwrap()).join("note.bin"), &bytes).unwrap();
    let native = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../tools/pencil2-mesh-v309/note-native-geometry.vbo"));
    assert_eq!(bytes.len(), native.len());
    let differences: Vec<_> = native.chunks_exact(4).zip(bytes.chunks_exact(4)).enumerate()
        .filter_map(|(i, (a, b))| (a != b).then(|| (i / 14, i % 14, u32::from_le_bytes(a.try_into().unwrap()), u32::from_le_bytes(b.try_into().unwrap())))).collect();
    assert_eq!(differences, vec![(52, 8, 0x3dd57232, 0x3dd57231), (60, 8, 0x3dc8cfaf, 0x3dc8cfae)]);
}

#[test]
fn synthetic_geometry_dump(){
 use crate::{StrokeStyle,StrokeRendering,BoundingBox,Point,Color};
 let mut style=serde_json::to_value(StrokeStyle::default()).unwrap();
 for (k,v) in [("pen_size",serde_json::json!(2.)),("color_argb",serde_json::json!(0x80336699u32)),("size_level",serde_json::json!(0)),("stroke_type_raw",serde_json::json!(0)),("color_type_raw",serde_json::json!(0))] {style[k]=v;}
 let props=serde_json::json!({"compressed":true,"stylus_channels":true,"replay_only":false,"eraser":false,"fixed_width":false,"millisecond_timestamps":false,"top_layer_pen":false,"alpha_lock":false,"binary_added":false,"generated":false,"fixed_opacity":false,"rainbow_effect":false,"straighten":false,"reveal_mode":false});
 let rendering:StrokeRendering=serde_json::from_value(serde_json::json!({"metadata":null,"pen_name":"com.samsung.android.sdk.pen.pen.preload.Pencil2","advanced_settings":"14;","tool_type_raw":2,"properties":props,"style":style})).unwrap();
 let s=Stroke{bbox:BoundingBox::default(),points:vec![Point{x:0.,y:0.},Point{x:10.,y:0.},Point{x:20.,y:0.},Point{x:30.,y:10.}],pressures:vec![0.25,0.5,0.75,1.],tilts:vec![0.4;4],orientations:vec![0.;4],timestamps:vec![10,20,30,40],color:Some(Color{r:0x33,g:0x66,b:0x99}),pen_width:2.,rendering:Some(rendering)};
 for idx in 0..2 {let mut c=s.clone(); let (_tr,_grain)=if idx==0 {([6.,9.],[6.,9.])} else { c.points=vec![Point{x:0.,y:2.},Point{x:10.,y:2.},Point{x:20.,y:4.},Point{x:30.,y:12.}];c.pressures=vec![0.9,0.7,0.5,0.3];c.pen_width=10.;c.color=Some(Color{r:0xcc,g:0x33,b:0x44});let mut r=serde_json::to_value(c.rendering.as_ref().unwrap()).unwrap();r["style"]["pen_size"]=serde_json::json!(10.);r["style"]["color_argb"]=serde_json::json!(0xc0cc3344u32);c.rendering=Some(serde_json::from_value(r).unwrap());([12.,12.],[12.,13.999998])};
 let rows=prepare_stroke(&c,1.).unwrap();
 let bytes:Vec<u8>=rows.iter().flat_map(|r|r.lanes.iter().flat_map(|f|f.to_le_bytes())).collect();
 let root=std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
 let path=if idx==0 {root.join("tools/pencil2-worker-v259/native-geometry.vbo")} else {root.join("tools/pencil2-mixed-scene-controls/white-1-second.vbo")};
 assert_eq!(bytes,std::fs::read(path).unwrap());
 std::fs::write(std::path::Path::new(&std::env::var("PENCIL_MESH_GEOMETRY_OUT").unwrap()).join(format!("synthetic{idx}.bin")),bytes).unwrap();
 }
}
