"""Observe shipped vertex outputs without changing either native shader source."""

import ast
import hashlib
import json
from pathlib import Path
import sys
import tempfile

ROOT = Path(__file__).resolve().parents[2]
HELPER = ROOT / "tools/watercolor_host/frame_capture.py"
PIN = "d87055488b895e4ee6dce35ca06f615f69c04a2d2f1cf28d3b7ad9786618ffd8"
PREFIX = ROOT / "tools/watercolor_host/prefix_probe.py"
PREFIX_PIN = "84c82b8c13550d5ee28f0330fa65f1382f28ba1569161201f6f0118fdcaaaf58"
LINK = """
 def gl_link_program_with_feedback(self):
  program=self.x(0)
  shaders=(C.c_uint*8)();count=C.c_int()
  ip.cfunc(self.gles.gl,'glGetAttachedShaders',None,[C.c_uint,C.c_int,C.POINTER(C.c_int),C.POINTER(C.c_uint)])(program,8,C.byref(count),shaders)
  matching=[s for s in shaders[:count.value] if 'aTransform' in ''.join(self.shader_sources.get(str(s),[]))]
  if matching:
   if len(matching)!=1:raise RuntimeError('feedback requires exactly one shipped vertex shader')
   text=''.join(self.shader_sources[str(matching[0])]).encode()
   if hashlib.sha256(text).hexdigest()!='9a0f2923ef1d11e31667892397425ed7c020f538a0149ebfe170a2ee10b807e6':raise RuntimeError('feedback vertex source drift')
   varyings=(C.c_char_p*3)(b'gl_Position',b'vUV',b'vAlpha')
   ip.cfunc(self.gles.gl,'glTransformFeedbackVaryings',None,[C.c_uint,C.c_int,C.POINTER(C.c_char_p),C.c_uint])(program,3,varyings,0x8c8c)
  self.gles.f['glLinkProgram'](program)
"""
BEGIN = """
  tf_buffer=C.c_uint();query=C.c_uint()
  tf_binding=C.c_int()
  ip.cfunc(self.gles.gl,'glGetIntegeri_v',None,[C.c_uint,C.c_uint,C.POINTER(C.c_int)])(0x8c8f,0,C.byref(tf_binding))
  saved_tf_buffer=self._query_int(0x8c8f)
  self.gles.f['glGenBuffers'](1,C.byref(tf_buffer))
  self.gles.f['glBindBuffer'](0x8c8e,tf_buffer.value)
  self.gles.f['glBufferData'](0x8c8e,480*6*7*4,None,0x88e1)
  tf_base=ip.cfunc(self.gles.gl,'glBindBufferBase',None,[C.c_uint,C.c_uint,C.c_uint])
  tf_base(0x8c8e,0,tf_buffer.value)
  ip.cfunc(self.gles.gl,'glGenQueries',None,[C.c_int,C.POINTER(C.c_uint)])(1,C.byref(query))
  ip.cfunc(self.gles.gl,'glBeginQuery',None,[C.c_uint,C.c_uint])(0x8c88,query.value)
  ip.cfunc(self.gles.gl,'glBeginTransformFeedback',None,[C.c_uint])(0x0004)
"""
END = """
  ip.cfunc(self.gles.gl,'glEndTransformFeedback',None,[])()
  ip.cfunc(self.gles.gl,'glEndQuery',None,[C.c_uint])(0x8c88)
  written=C.c_uint()
  ip.cfunc(self.gles.gl,'glGetQueryObjectuiv',None,[C.c_uint,C.c_uint,C.POINTER(C.c_uint)])(query.value,0x8866,C.byref(written))
  if written.value!=960:raise RuntimeError(f'feedback primitive count mismatch: {written.value}')
  mapped=ip.cfunc(self.gles.gl,'glMapBufferRange',C.c_void_p,[C.c_uint,C.c_ssize_t,C.c_ssize_t,C.c_uint])(0x8c8e,0,480*6*7*4,1)
  if not mapped:raise RuntimeError('feedback map failed')
  raw=C.string_at(mapped,480*6*7*4)
  if not ip.cfunc(self.gles.gl,'glUnmapBuffer',C.c_ubyte,[C.c_uint])(0x8c8e):raise RuntimeError('feedback unmap failed')
  tf_base(0x8c8e,0,tf_binding.value)
  self.gles.f['glBindBuffer'](0x8c8e,saved_tf_buffer)
  ip.cfunc(self.gles.gl,'glDeleteBuffers',None,[C.c_int,C.POINTER(C.c_uint)])(1,C.byref(tf_buffer))
  ip.cfunc(self.gles.gl,'glDeleteQueries',None,[C.c_int,C.POINTER(C.c_uint)])(1,C.byref(query))
  if self._drain_gl_errors():raise RuntimeError('feedback GL errors')
  self.vertex_probe={'primitive_count':written.value,'records':480,'vertices_per_record':6,'vertex_fields':['clipX','clipY','clipZ','clipW','u','v','alpha'],'raw_sha256':hashlib.sha256(raw).hexdigest(),'rgba_f32_zlib_base64':base64.b64encode(zlib.compress(raw,9)).decode()}
"""
source = HELPER.read_text()
assert hashlib.sha256(source.encode()).hexdigest() == PIN
prefix = PREFIX.read_text()
assert hashlib.sha256(prefix.encode()).hexdigest() == PREFIX_PIN
split = next(
    ast.literal_eval(n.value)
    for n in ast.parse(prefix).body
    if isinstance(n, ast.Assign)
    and any(isinstance(t, ast.Name) and t.id == "SPLIT" for t in n.targets)
)
split = split.replace("glReadPixels'](10,1079", "glReadPixels'](44,631")
split = split.replace("  self.prefix_probe=[]", BEGIN + "  self.prefix_probe=[]")
split = split.replace(
    "\n  for a in attributes:\n", "\n" + END + "  for a in attributes:\n"
)
anchor = "    exec(\n"
assert source.count(anchor) == 1
injection = """    target="  self.gles.f['glDrawArraysInstanced'](*vals)"
    assert code.count(target)==1
    code=code.replace(target,SPLIT)
    callback="'glLinkProgram':lambda:self.gles.f['glLinkProgram'](self.x(0))"
    assert code.count(callback)==1
    code=code.replace(callback,"'glLinkProgram':self.gl_link_program_with_feedback")
    method=' def gl_draw_arrays_instanced(self):'
    assert code.count(method)==1
    code=code.replace(method,LINK+method)
    code=code.replace(schema,schema+",'vertex_probe':p.vertex_probe,'prefix_probe':p.prefix_probe")
"""
source = source.replace(anchor, injection + anchor)
with tempfile.TemporaryDirectory(prefix="sdocx-watercolor-vertex-") as directory:
    report = Path(directory) / "native.json"
    sys.argv = [
        str(HELPER),
        "--child",
        "--width",
        "768",
        "--height",
        "640",
        "--scale",
        "2",
        "--repeat",
        "32",
        "--output",
        str(report),
    ]
    try:
        exec(
            compile(source, str(HELPER), "exec"),
            {
                "__name__": "watercolor_vertex_probe",
                "__file__": str(HELPER),
                "LINK": LINK,
                "SPLIT": split,
            },
        )
    except SystemExit as done:
        if done.code not in (None, 0):
            raise
    native = json.loads(report.read_text())
    baseline = json.loads(
        (ROOT / "conformance/watercolor-redraw-frames-native.json").read_text()
    )["cases"]["768x640_2_32"]
    expected = json.loads(
        (ROOT / "conformance/watercolor-precision-pixel-native.json").read_text()
    )["prefix_rgba"]
    assert native["capture_status"] == "captured_without_host_gl_errors"
    assert native["readback"]["sha256"] == baseline["final_sha256"]
    assert (
        native["native_mask_pass_readback"]["pixels_sha256"] == baseline["mask_sha256"]
    )
    assert native["prefix_probe"] == expected
    receipt = {
        "schema": "sdocx-watercolor-vertex-feedback-v1",
        "frame": [768, 640],
        "capture": {
            "script": "tools/watercolor_host/vertex_probe.py",
            "script_sha256": hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
            "frame_helper_sha256": PIN,
            "prefix_controller_sha256": PREFIX_PIN,
        },
        "scope": "Transform feedback configured before native mask program link; both shipped shader sources unchanged. Original native mask draw split into one-instance calls. All 480 mask prefix pixels, full mask and final frame required unchanged. Feedback is pre-clipping/pre-viewport shader output, not page or device fidelity.",
        "mask_sha256": baseline["mask_sha256"],
        "final_sha256": baseline["final_sha256"],
        "verified_prefix_count": len(expected),
        "native": native["vertex_probe"],
    }
    target = ROOT / "conformance/watercolor-vertex-native.json"
    target.write_text(json.dumps(receipt, separators=(",", ":")) + "\n")
    print(target, target.stat().st_size)
