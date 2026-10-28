mod path;
mod sampler;
use sampler::{Input, RtCall};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let doc = sdocx::parse("tmp/legacy-marker-source-controls/serialized-marker.sdocx")?;
    for (name, end, pressure, width) in [("baseline", 20., 0.6, 2.), ("pressure", 20., 0.85, 2.), ("size", 20., 0.6, 1.5), ("far_endpoint", 30., 0.6, 2.)] {
        let mut stroke = doc.pages.iter().flat_map(|p| p.strokes()).next().unwrap().clone();
        stroke.points = vec![sdocx::Point{x:128.,y:128.},sdocx::Point{x:138.,y:128.},sdocx::Point{x:128.+end,y:126.}];
        stroke.pressures = vec![0.3, pressure, if name == "pressure" { 0.9 } else { 1. }];
        stroke.pen_width=width;
        let points: Vec<[f32;2]> = stroke.points.iter().map(|p| [p.x as f32 - 128., 128. - p.y as f32]).collect();
        let pressure: Vec<f32> = stroke.pressures.iter().map(|x| *x as f32).collect();
        let records = sampler::sample(Input { points: &points, pressure: &pressure, pen_size: stroke.pen_width as f32 }).unwrap();
        let mut buffers: [Vec<[f32;5]>;3] = std::array::from_fn(|_| Vec::new());
        for r in records { let (idx, vertices) = match r.call { RtCall::Start => (1,sampler::cap_vertices(r.lanes)), RtCall::Point => (0,sampler::point_vertices(r.lanes)), RtCall::End => (2,sampler::cap_vertices(r.lanes)) }; buffers[idx].extend(vertices); }
        println!("{}", serde_json::json!({"case":name,"buffers":buffers}));
    }
    Ok(())
}
