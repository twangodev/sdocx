"""Experimental null-setting RTV1 host-Mesa capture, adapted from the joined harness.

The parent harness is retained unmodified. The replacements below select the native
V1 entry points and omit V3-only assertions; emitted observations are not device pixels.
"""
from pathlib import Path
import hashlib
import sys

opaque_background = '--opaque-background' in sys.argv
if opaque_background:
    sys.argv.remove('--opaque-background')

root = Path(__file__).resolve().parents[1]
support = root / 'tmp/vector-parity-plan/chinesebrush-paint-scale-work/conformance'
pins = {
    'oblique_canvas_native.py': '1cabf3f8bf907ce6fd48952682b4cad5208827cec4aa0bd0a1ec864aa750a1f2',
    'brushpen_paint_positive_native.py': '6006e9719a6f32ac6dca9ab80af17d95647eb156871825481d305d89211fde8a',
    'inkpen_paint_native.py': 'a3e65b2b37b2913b4bb2787b72ab2dbad0a9660cb66435b142f5acfa4d5051ae',
    'fountain_native.py': '40cb917c7f286302d8519e9b953c3c21b7fe6ea0b6b0a80ccc13e2e724cfad30',
    'elf_plt.py': '816eceff5b532b35c8ef9ea904a8aa64767422b0799347a18fbd4602b70f373b',
    'brushpen_texture_native.py': 'ba6a8280580f1c737a90849389b5772fc1dd1d82e4837f27fae68f27218ac267',
    'brushpen_saved_native.py': 'af92a74d666f365ffe92fb50b0ab09f77a233bf9ac10e884e05d2a632dfaa9cd',
}
for name, digest in pins.items():
    path = support / name
    if not path.is_file() or hashlib.sha256(path.read_bytes()).hexdigest() != digest:
        raise SystemExit(f'missing or changed pinned capture support: {path}')
source = Path(__file__).resolve().parents[1] / 'scratch/straightinkpen2-alias/inkpen2_joined_paint_native.py'
if hashlib.sha256(source.read_bytes()).hexdigest() != '22d16b75d3324b36606b66cc380e37cd4d6c9f3df14c4d4849120ed0ae294b38':
    raise SystemExit(f'changed joined capture harness: {source}')
code = source.read_text()
code = code.replace("parser.add_argument('--case',", "parser.add_argument('--input-json'); parser.add_argument('--case',")
code = code.replace('input_case=INPUT_CASES[args.case]', "input_case=json.loads(pathlib.Path(args.input_json).read_text()) if args.input_json else INPUT_CASES[args.case]")
code = code.replace("choices=('empty','9'),default='9'", "choices=('null',),default='null'")
code = code.replace("choices=(1.0,2.0,4.0)", "choices=(0.5,1.0,2.0,4.0)")
code = code.replace("default=1440)", "default=1920)").replace("default=2560)", "default=1080)")
code = code.replace("if (args.case,args.width,args.height)!=('line0_curve_short_history_color',1440,2560):", "if args.case != 'line0_curve_short_history_color' or not (0 < args.width <= 65535 and 0 < args.height <= 65535):")
if opaque_background:
    code = code.replace("  before_draw_fbo=bp.C.c_int();", "  p.mesa.f['glClearColor'](1.0,1.0,1.0,1.0); p.mesa.f['glClear'](0x4000)\n  before_draw_fbo=bp.C.c_int();")
code = code.replace('InkPen2StrokeDrawableRTV3::', 'InkPen2StrokeDrawableRTV1::')
# RTV1 CreatePenCanvas stores its canvas at +200, as observed in native code.
code = code.replace("join_rt+208,8", "join_rt+200,8").replace("join_rt+208,'Q'", "join_rt+200,'Q'")
# V3-specific descriptor observations are not interpreted as V1 evidence.
start = code.index("  blend_obj=struct.unpack('<Q',p.u.mem_read(join_rt+232,8))[0]; print('BLEND_SYMS'")
end = code.index('  # Explicitly clear', start)
code = code[:start] + code[end:]
start = code.index("  descriptor232=next(row for row in descriptor_events")
end = code.index('\n except Exception as join_error:', start)
code = code[:start] + '''  jb,je,jc=struct.unpack('<3Q',p.u.mem_read(p.vector,24)); join_bytes=bytes(p.u.mem_read(jb,je-jb))
  assert selected_drawable_version == '1' and len(join_bytes) % 12 == 0
  assert any(row.get('data_hex') == join_bytes.hex() for row in p.graphics_buffer_uploads)
  assert (not any(rgba_before) if not opaque_background else rgba_before == bytes([255])*len(rgba_before)) and fbo_status == 0x8cd5 and attach_type.value == 0x1702
  assert before_draw_fbo.value == before_read_fbo.value == after_draw_fbo.value == after_read_fbo.value
  print(json.dumps({'framebuffer':{'complete':fbo_status == 0x8cd5,'attachment_type':attach_type.value,'texture_level':texture_level,'draw_fbo':after_draw_fbo.value}, 'schema':'inkpen2-null-v1-host-paint-experimental-v1','source':'synthetic native ObjectStroke, Drawing-selected RTV1; host Mesa and host target', 'profile':[args.width,args.height], 'selected_version':selected_drawable_version, 'selected_rt':selected_rt_id, 'geometry_hex':join_bytes.hex(), 'geometry_sha256':hashlib.sha256(join_bytes).hexdigest(), 'output_size':[target_width,target_height], 'draw_matrix':list(matrix_values), 'before_zero':not any(rgba_before), 'rgba_zlib_base64':base64.b64encode(zlib.compress(rgba_draw,9)).decode(), 'rgba_sha256':hashlib.sha256(rgba_draw).hexdigest(), 'nonzero_pixels':len(drawn), 'draw_calls':p.graphics_draw_calls, 'uploads':p.graphics_buffer_uploads, 'gl_events':gl_events, 'shader_programs':{str(k):{'linked':v['linked'],'sources':[src for sh in v['shader_snapshots'] for src in sh['sources']]} for k,v in program_rows.items()}},default=str), _always=True)
  raise SystemExit(0)
''' + code[end:]
exec(compile(code, str(source), 'exec'), {'__name__':'__main__','__file__':str(source), 'opaque_background': opaque_background})
