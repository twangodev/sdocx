"""Bounded diagnostic: try native ObjectLine on the retained InkPen2 bitmap."""
from pathlib import Path
import hashlib
root=Path(__file__).resolve().parents[2]
import sys
import tempfile
output_dir = Path(tempfile.mkdtemp(prefix='sdocx-mixed-line-'))
assert '--input-json' in sys.argv
input_path = Path(sys.argv[sys.argv.index('--input-json') + 1])
assert hashlib.sha256(input_path.read_bytes()).hexdigest() == '667ce1567150447bca4e5358f18a2f3f6fd8a8335d3d88b7f7da6c39a02d4069'
print(f'private frame directory: {output_dir}', flush=True)
probe = root/'tools/inkpen2-v1-tablet-half/mixed_scene_native_line_probe.py'
assert hashlib.sha256(probe.read_bytes()).hexdigest() == 'c7f1d0adc0a06364a5d7487d439e40117203f410666ae00caed6c60b544a23e6'
source=root/'tools/inkpen2-v1-tablet-half/overlap_native.py'
wrapper=source.read_text()
assert hashlib.sha256(source.read_bytes()).hexdigest() == 'e8f71ed5e884bf4352da73b7698aa180f31407d7393ad2ed9494a25f6b3d1b69'
anchor="exec(compile(code, str(source), 'exec'), {'__name__':'__main__','__file__':str(source), 'opaque_background': opaque_background})"
skia = root / 'scratch/apk-analysis-native/arm64-v8a/libSPenSkia.so'
assert hashlib.sha256(skia.read_bytes()).hexdigest() == '42636cb9ac06cc286114b42c1b9d8f4b78d33761843251cde2b443b101ffb88d'

loader_inject = """
code = code.replace("'Renderer':LIBS/'libSPenRenderer.so','BrushPen':LIBS/'libSPenBrushPen.so'}", "'Renderer':LIBS/'libSPenRenderer.so','BrushPen':LIBS/'libSPenBrushPen.so','Skia':LIBS/'libSPenSkia.so'}")
code = code.replace("'Renderer':0x5000000,'BrushPen':0x3000000}", "'Renderer':0x5000000,'BrushPen':0x3000000,'Skia':0xB000000}")
code = code.replace("for extra in ('InkPen2','Drawing','Composer'):", "for extra in ('InkPen2','Drawing','Composer','Skia'):")
code = code.replace("{6:4096,16:256,26:0}", "{6:4096,16:0,26:0}")
code = code.replace("  join_matrix=p.alloc(64);", '''  def trace_gl_target(name, original):
   def wrapped():
    fb=bp.C.c_int(); attachment=bp.C.c_int()
    p.mesa.f['glGetIntegerv'](0x8CA6,bp.C.byref(fb)); p.mesa.f['glGetFramebufferAttachmentParameteriv'](0x8D40,0x8CE0,0x8CD1,bp.C.byref(attachment))
    print(json.dumps({'native_gl_draw':name,'framebuffer':fb.value,'attachment':attachment.value}), _always=True)
    return original()
   return wrapped
  for draw_name in ('glDrawArrays','glDrawElements','glDrawArraysInstanced','glDrawElementsInstanced'):
   if draw_name in p.callbacks: p.callbacks[draw_name]=trace_gl_target(draw_name,p.callbacks[draw_name])
  print(json.dumps({'initial_zero':not any(rgba_before),'initial_sha256':hashlib.sha256(rgba_before).hexdigest(),'target':[target_width,target_height],'ink_color_argb':hex(source_color_argb),'bitmap':hex(rgba_bitmap)}),_always=True)
  join_matrix=p.alloc(64);''')

code = code.replace("(LIBS/'libSPenDrawing.so',0x8000000),]:", "(LIBS/'libSPenSkia.so',0xB000000),(LIBS/'libSPenDrawing.so',0x8000000),]:")
"""
assert wrapper.count(anchor) == 1
wrapper = wrapper.replace(anchor, loader_inject + anchor)

inject='''
  names = ["SPen::ObjectLine::ObjectLine()", "SPen::ObjectLine::Construct(SPen::ObjectLine::LineType, SPen::PointF, SPen::PointF, bool)", "SPen::Path::Path()", "SPen::Path::Construct()", "SPen::Path::SetSegment(SPen::Path::Segment const*, int)", "SPen::ObjectLine::SetPathDataOnly(SPen::Path const*)", "SPen::LineStyleEffect::LineStyleEffect()", "SPen::LineStyleEffect::Construct()", "SPen::LineStyleEffect::SetWidth(float)", "SPen::ObjectShapeBase::SetLineStyleEffect(SPen::LineStyleEffect*)", "SPen::ObjectDrawing::drawObjectShape(SPen::ISPCanvas&, SPen::ObjectShape*, SPen::Rect const*)", "SPen::SPCanvas::SPCanvas(SPen::IGLMsgQueue*, SPen::ISPBitmap*)"]
  print(json.dumps({'probe':'mixed-line-symbols','available':{name:hex(p.syms[name]) for name in names if name in p.syms}, 'missing':[name for name in names if name not in p.syms], 'rgba_bitmap':hex(rgba_bitmap), 'rgba_canvas_impl':hex(rgba_canvas), 'rgba_subbitmap':hex(rgba_subbitmap), 'line_invoked':False}), _always=True)
  import sys as _sys
  def stage(v): _sys.stderr.write(v + "\\n"); _sys.stderr.flush()
  uuid_context=p.alloc(64); uuid_sequence=[0]
  def uuid_create(): p.put(p.x(0),"Q",uuid_context); p.x(0,0)
  def uuid_export():
   uuid_sequence[0]+=1; dest=p.get(p.x(2),"Q")[0]; p.u.mem_write(dest,("00000000-0000-4000-8000-%012x" % uuid_sequence[0]).encode()+bytes([0])); p.x(0,0)
  p.callbacks.update({"uuid_create":uuid_create,"uuid_make":lambda:p.x(0,0),"uuid_export":uuid_export,"uuid_destroy":lambda:p.x(0,0)})
  import ctypes as _ctypes
  libm_path = "/usr/lib/x86_64-linux-gnu/libm.so.6"
  assert hashlib.sha256(pathlib.Path(libm_path).read_bytes()).hexdigest() == "6d567d53e895273ca14a1f9dc164fc6c8d39aed2f60aa46a733c2784228915f3"
  libm = _ctypes.CDLL(libm_path)
  libm.pow.argtypes = (_ctypes.c_double, _ctypes.c_double)
  libm.pow.restype = _ctypes.c_double
  def host_pow():
   values = [struct.unpack("<d", struct.pack("<Q", p.u.reg_read(R.UC_ARM64_REG_D0 + i)))[0] for i in (0, 1)]
   p.d(0, libm.pow(*values))
  p.callbacks["pow"] = host_pow
  libm.acosf.argtypes = (_ctypes.c_float,)
  libm.acosf.restype = _ctypes.c_float
  p.callbacks["acosf"] = lambda: p.f(0, libm.acosf(p.f(0)))
  stage("construct_line")
  line=p.alloc(512); path=p.alloc(128); style=p.alloc(128); line_canvas=p.alloc(512); clip=p.alloc(16)
  p.call("SPen::ObjectLine::ObjectLine()",[line],instruction_limit=1000000)
  for i,v in enumerate((4.,16.,60.,16.)): p.f(i,v)
  p.call("SPen::ObjectLine::Construct(SPen::ObjectLine::LineType, SPen::PointF, SPen::PointF, bool)",[line,0,1],instruction_limit=1000000)
  stage("construct_path")
  segments=p.alloc(56); p.u.mem_write(segments,b"".join(struct.pack("<I6f",*row) for row in ((1,4.,16.,0,0,0,0),(2,60.,16.,0,0,0,0))))
  p.call("SPen::Path::Path()",[path],instruction_limit=1000000); p.call("SPen::Path::Construct()",[path],instruction_limit=1000000); p.call("SPen::Path::SetSegment(SPen::Path::Segment const*, int)",[path,segments,2],instruction_limit=1000000); p.call("SPen::ObjectLine::SetPathDataOnly(SPen::Path const*)",[line,path],instruction_limit=1000000)
  stage("construct_style")
  p.call("SPen::LineStyleEffect::LineStyleEffect()",[style],instruction_limit=1000000); p.call("SPen::LineStyleEffect::Construct()",[style],instruction_limit=1000000); p.f(0,6.); p.call("SPen::LineStyleEffect::SetWidth(float)",[style],instruction_limit=1000000); p.call("SPen::ObjectShapeBase::SetLineStyleEffect(SPen::LineStyleEffect*)",[line,style],instruction_limit=1000000)
  stage("construct_canvas_same_bitmap")
  p.call("SPen::SPCanvas::SPCanvas(SPen::IGLMsgQueue*, SPen::ISPBitmap*)",[line_canvas,queue,rgba_bitmap],instruction_limit=1000000)
  p.put(clip,"4i",0,0,target_width,target_height)
  def framebuffer_snapshot(label):
   framebuffer=bp.C.c_int(); attachment=bp.C.c_int()
   p.mesa.f["glGetIntegerv"](0x8CA6,bp.C.byref(framebuffer))
   p.mesa.f["glGetFramebufferAttachmentParameteriv"](0x8D40,0x8CE0,0x8CD1,bp.C.byref(attachment))
   pixels=bp.C.create_string_buffer(target_width*target_height*4)
   p.mesa.f["glReadPixels"](0,0,target_width,target_height,0x1908,0x1401,pixels)
   data=bytes(pixels.raw)
   stage(json.dumps({"snapshot":label,"framebuffer":framebuffer.value,"attachment":attachment.value,"sha256":hashlib.sha256(data).hexdigest(),"nonzero_pixels":sum(any(data[i:i+4]) for i in range(0,len(data),4)),"gl_error":p.mesa.f["glGetError"]()}))
   return data
  pixels_before_line=framebuffer_snapshot("before_line")
  stage("skia lookup=" + str((p.syms.get("SkPaint::SkPaint()"),p.plt.get(0x80ba910))))
  stage("draw_line")
  try:
   p.call("SPen::ObjectDrawing::drawObjectShape(SPen::ISPCanvas&, SPen::ObjectShape*, SPen::Rect const*)",[p.drawing,line_canvas,line,clip],instruction_limit=10000000)
  except Exception:
   stage("draw_line_exception pc=" + hex(p.u.reg_read(R.UC_ARM64_REG_PC)) + " lr=" + hex(p.u.reg_read(R.UC_ARM64_REG_LR)))
   raise
  stage("draw_line_returned")
  pixels_after_line=framebuffer_snapshot("after_line")
  for label, data in (("before-line", pixels_before_line), ("after-line", pixels_after_line)):
   (pathlib.Path(output_dir) / ("mixed-scene-native-" + label + ".rgba.zlib")).write_bytes(zlib.compress(data, 9))
  stage("changed_pixels=" + str(sum(pixels_before_line[i:i+4] != pixels_after_line[i:i+4] for i in range(0,len(pixels_before_line),4))))
  from collections import Counter
  changes=Counter(pixels_after_line[i:i+4].hex() for i in range(0,len(pixels_before_line),4) if pixels_before_line[i:i+4] != pixels_after_line[i:i+4])
  stage("changed_color_counts=" + json.dumps(changes.most_common(12)))
'''
assert wrapper.count(anchor)==1
final = """  final_pixels=framebuffer_snapshot("after_second_stroke")
  (pathlib.Path(output_dir) / "mixed-scene-native-final.rgba.zlib").write_bytes(zlib.compress(final_pixels,9))
  raise SystemExit(0)
"""
wrapper=wrapper.replace(anchor, 'code = code.replace("  first_read=", ' + repr(inject + "\n  first_read=") + ')\n' + 'code = code.replace("  p.u.hook_del(draw_branch_hook)", ' + repr(final + "\n  p.u.hook_del(draw_branch_hook)") + ')\n' + anchor)
wrapper = wrapper.replace(anchor, anchor.replace("'opaque_background': opaque_background}", "'opaque_background': opaque_background, 'output_dir': output_dir}"))
exec(compile(wrapper,str(source),'exec'),{'__name__':'__main__','__file__':str(source), 'output_dir': output_dir})
