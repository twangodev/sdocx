use sdocx::Stroke;
use lopdf::Document;
use sdocx::scene203::*;
    use sdocx::{BoundingBox, Color, Point, StrokeStyle, StrokeRendering};
    fn stroke() -> Stroke {
        let mut style=serde_json::to_value(StrokeStyle::default()).unwrap();
        for (k,v) in [("pen_size",serde_json::json!(2.)),("color_argb",serde_json::json!(0x80336699u32)),("size_level",serde_json::json!(0)),("stroke_type_raw",serde_json::json!(0)),("color_type_raw",serde_json::json!(0))] {style[k]=v;}
        let props=serde_json::json!({"compressed":true,"stylus_channels":true,"replay_only":false,"eraser":false,"fixed_width":false,"millisecond_timestamps":false,"top_layer_pen":false,"alpha_lock":false,"binary_added":false,"generated":false,"fixed_opacity":false,"rainbow_effect":false,"straighten":false,"reveal_mode":false});
        let rendering:StrokeRendering=serde_json::from_value(serde_json::json!({"metadata":null,"pen_name":"com.samsung.android.sdk.pen.pen.preload.Pencil2","advanced_settings":"14;","tool_type_raw":2,"properties":props,"style":style})).unwrap();
        Stroke{bbox:BoundingBox::default(),points:vec![Point{x:0.,y:0.},Point{x:10.,y:0.},Point{x:20.,y:0.},Point{x:30.,y:10.}],pressures:vec![0.25,0.5,0.75,1.],tilts:vec![0.4;4],orientations:vec![0.;4],timestamps:vec![10,20,30,40],color:Some(Color{r:0x33,g:0x66,b:0x99}),pen_width:2.,rendering:Some(rendering)}
    }
    fn limits() -> Limits { Limits {source_points: 8, frames: 168, work: usize::MAX, svg_bytes: usize::MAX, pdf_bytes: usize::MAX, pdf_objects: 512} }
    fn grid() -> Grid { Grid {width:192,height:192,scale:1.,translation:[6.,9.],grain_offset:[6.,9.]} }
    fn main() {
        let a=stroke(); let mut b=stroke(); b.points[1].y=2.;
        let items=[Item{stroke:&a,grid:grid()},Item{stroke:&b,grid:grid()}];
        let mut doc=Document::with_version("1.5"); doc.add_object("prior"); let before=doc.clone();
        let mut registry=vec!["prior".to_owned()];
        for fail in [Some(0),Some(1)] { assert!(transaction(&items,&mut registry,&mut doc,limits(),fail).is_none()); assert_eq!(registry,["prior"]); assert_eq!(doc.objects,before.objects); assert_eq!(doc.max_id,before.max_id); }
        let mut invalid=b.clone(); invalid.pressures.pop();
        let invalid_items=[Item{stroke:&a,grid:grid()},Item{stroke:&invalid,grid:grid()}];
        assert!(transaction(&invalid_items,&mut registry,&mut doc,limits(),None).is_none());
        let mut first_exhausts = limits(); first_exhausts.frames = 1;
        assert_eq!(observed_preflight(&invalid_items, &doc, first_exhausts), (false, 1));
        assert_eq!(observed_preflight(&invalid_items, &doc, limits()), (false, 2));
        let l=limits();
        for small in [Limits {source_points:7,..l}, Limits {frames:1,..l}, Limits {work:1,..l}, Limits {svg_bytes:1,..l}, Limits {pdf_bytes:1,..l}, Limits {pdf_objects:511,..l}] {
            assert!(transaction(&items,&mut registry,&mut doc,small,None).is_none());
            assert_eq!(registry,["prior"]); assert_eq!(doc.objects,before.objects); assert_eq!(doc.max_id,before.max_id);
        }
        let forms=transaction(&items,&mut registry,&mut doc,limits(),None).unwrap(); assert_eq!(forms.len(),2); assert_eq!(registry.len(),3); assert!(!registry[1].is_empty());
    }
