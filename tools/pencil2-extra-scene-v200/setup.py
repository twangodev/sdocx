from pathlib import Path
import sys
root=Path('tools/pencil2-scene-transport-v199')
work=Path(sys.argv[1])
helper=(root/'helper.rs').read_text().replace('let ctx = ink::pencil2::device_raster::Context { translation, grain_offset, ..Default::default() };','let scale: f32 = std::env::var("SCALE").unwrap().parse().unwrap(); let ctx = ink::pencil2::device_raster::Context { scale, translation, grain_offset, ..Default::default() };')
with (work/'crates/sdocx/src/lib.rs').open('a') as f: f.write(helper)
probe=(root/'probe.rs').read_text()
probe=probe.replace('let mut svg=String::from("<svg xmlns=\\"http://www.w3.org/2000/svg\\" width=\\"192\\" height=\\"192\\"><rect width=\\"192\\" height=\\"192\\" fill=\\"rgb(64,128,191)\\"/>");', '''let backdrop=std::env::var("BACKDROP").unwrap(); let scale: f32=std::env::var("SCALE").unwrap().parse().unwrap();
 let mut svg=String::from("<svg xmlns=\\"http://www.w3.org/2000/svg\\" width=\\"192\\" height=\\"192\\">");
 match backdrop.as_str() { "white" => svg.push_str("<rect width=\\"192\\" height=\\"192\\" fill=\\"white\\"/>"), "blue" => svg.push_str("<rect width=\\"192\\" height=\\"192\\" fill=\\"rgb(64,128,191)\\"/>"), "transparent" => (), _ => panic!() };''')
probe=probe.replace('[12.,13.999998])};','[12.,if scale == 2. {16.0000019} else {13.999998}])};')
probe=probe.replace('1_999_000', '7_999_000').replace('2_000_000', '8_000_000')
assert 'let backdrop=' in probe and 'if scale == 2.' in probe
(work/'crates/sdocx/examples/pencil200.rs').write_text(probe)
