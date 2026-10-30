"""Diagnostic interpolation readback using the shipped WaterColor vertex shader."""

import hashlib
import json
from pathlib import Path
import sys
import tempfile

ROOT = Path(__file__).resolve().parents[2]
HELPER = ROOT / "tools/watercolor_host/frame_capture.py"
PIN = "d87055488b895e4ee6dce35ca06f615f69c04a2d2f1cf28d3b7ad9786618ffd8"
PROBE = "  gl=self.gles.gl\n  def glfun(name,result,params):return ip.cfunc(gl,name,result,params)\n  uint=C.c_uint;integer=C.c_int;pointer=C.c_void_p\n  vertex_source=next(''.join(entry['source']) for entry in self.shader_source_calls if 'aTransform' in ''.join(entry['source']) and 'gl_Position' in ''.join(entry['source'])).encode()\n  original_fragment=next(''.join(entry['source']) for entry in self.shader_source_calls if 'uPatternTexture' in ''.join(entry['source']) and 'FragColor' in ''.join(entry['source'])).encode()\n  if hashlib.sha256(vertex_source).hexdigest()!='9a0f2923ef1d11e31667892397425ed7c020f538a0149ebfe170a2ee10b807e6':raise RuntimeError('native vertex source drift')\n  if hashlib.sha256(original_fragment).hexdigest()!='d898ba674f16b75a65783ed7937a0e609596dc3e24213f6955394ce58621985e':raise RuntimeError('native fragment source drift')\n  fragment_source=original_fragment.replace(b'out vec4 FragColor;',b'layout(location=0) out vec4 FragColor; layout(location=1) out highp vec4 ProbeUV;')\n  fragment_source=fragment_source.replace(b'0.0, 1.0); }',b'0.0, 1.0); ProbeUV=vec4(vUV,vAlpha,gl_FragCoord.w); }')\n  if fragment_source==original_fragment or b'ProbeUV=vec4' not in fragment_source:raise RuntimeError('diagnostic shader extension failed')\n  compiled=[]\n  for kind,text in [(0x8b31,vertex_source),(0x8b30,fragment_source)]:\n   shader=self.gles.f['glCreateShader'](kind)\n   texts=(C.c_char_p*1)(text);lengths=(integer*1)(len(text))\n   self.gles.f['glShaderSource'](shader,1,texts,lengths);self.gles.f['glCompileShader'](shader)\n   status=integer();self.gles.f['glGetShaderiv'](shader,0x8b81,C.byref(status))\n   if status.value!=1:raise RuntimeError('diagnostic shader compilation failed')\n   compiled.append(shader)\n  program=self.gles.f['glCreateProgram']()\n  for shader in compiled:self.gles.f['glAttachShader'](program,shader)\n  for i,name in enumerate([b'aRectPoint',b'aTransform',b'aRadius',b'aAlpha']):self.gles.f['glBindAttribLocation'](program,i,name)\n  self.gles.f['glLinkProgram'](program);self.gles.f['glGetProgramiv'](program,0x8b82,C.byref(status))\n  if status.value!=1:raise RuntimeError('diagnostic shader link failed')\n  location=glfun('glGetUniformLocation',integer,[uint,C.c_char_p])\n  matrix=(C.c_float*16)()\n  glfun('glGetUniformfv',None,[uint,integer,C.POINTER(C.c_float)])(before['program'],location(before['program'],b'uProjectionMatrix'),matrix)\n  self.gles.f['glUseProgram'](program)\n  self.gles.f['glUniformMatrix4fv'](location(program,b'uProjectionMatrix'),1,0,C.cast(matrix,C.POINTER(C.c_float)))\n  self.gles.f['glUniform1i'](location(program,b'uPatternTexture'),0)\n  textures=(uint*2)();framebuffer=uint()\n  self.gles.f['glGenTextures'](2,textures);self.gles.f['glGenFramebuffers'](1,C.byref(framebuffer))\n  saved_texture=self._query_int(0x8069)\n  self.gles.f['glBindFramebuffer'](0x8d40,framebuffer.value)\n  for index,texture in enumerate(textures):\n   self.gles.f['glBindTexture'](0x0de1,texture)\n   self.gles.f['glTexImage2D'](0x0de1,0,0x8058 if index==0 else 0x8814,ARGS.width,ARGS.height,0,0x1908,0x1401 if index==0 else 0x1406,None)\n   self.gles.f['glFramebufferTexture2D'](0x8d40,0x8ce0+index,0x0de1,texture,0)\n  self.gles.f['glBindTexture'](0x0de1,saved_texture)\n  buffers=(uint*2)(0x8ce0,0x8ce1)\n  glfun('glDrawBuffers',None,[integer,C.POINTER(uint)])(2,buffers)\n  if self.gles.f['glCheckFramebufferStatus'](0x8d40)!=0x8cd5:raise RuntimeError('diagnostic MRT incomplete')\n  blended=glfun('glIsEnabledi',C.c_ubyte,[uint,uint])(0x0be2,1)\n  glfun('glDisablei',None,[uint,uint])(0x0be2,1)\n  zero=(C.c_float*4)(0,0,0,0)\n  clear=glfun('glClearBufferfv',None,[uint,integer,C.POINTER(C.c_float)])\n  clear(0x1800,0,zero);clear(0x1800,1,zero)\n  bind=glfun('glBindBuffer',None,[uint,uint])\n  attribute=glfun('glVertexAttribPointer',None,[uint,integer,uint,C.c_ubyte,integer,pointer])\n  attributes=[a for a in before['attributes'] if a['divisor']==1]\n  saved_buffer=self._query_int(0x8894)\n  read_buffer=glfun('glReadBuffer',None,[uint])\n  read=glfun('glReadPixels',None,[integer,integer,integer,integer,uint,uint,pointer])\n  prefix=[];varyings=[]\n  for instance in range(480):\n   for a in attributes:\n    bind(0x8892,a['buffer'])\n    attribute(a['index'],a['size'],a['type'],a['normalized'],a['stride'],pointer(int(a['pointer'],16)+instance*a['stride']))\n   clear(0x1800,1,zero)\n   self.gles.f['glDrawArraysInstanced'](vals[0],vals[1],vals[2],1)\n   pixel=(C.c_ubyte*4)();rgba=(C.c_float*4)()\n   read_buffer(0x8ce0);read(44,631,1,1,0x1908,0x1401,C.cast(pixel,pointer))\n   read_buffer(0x8ce1);read(44,631,1,1,0x1908,0x1406,C.cast(rgba,pointer))\n   if self._drain_gl_errors():raise RuntimeError('diagnostic MRT draw/read failed')\n   prefix.append(list(pixel));varyings.append(list(rgba))\n  for a in attributes:\n   bind(0x8892,a['buffer'])\n   attribute(a['index'],a['size'],a['type'],a['normalized'],a['stride'],pointer(int(a['pointer'],16)))\n  bind(0x8892,saved_buffer)\n  if blended:glfun('glEnablei',None,[uint,uint])(0x0be2,1)\n  self.gles.f['glBindFramebuffer'](0x8d40,before['framebuffer']);read_buffer(0x8ce0)\n  self.gles.f['glUseProgram'](before['program'])\n  glfun('glDeleteFramebuffers',None,[integer,C.POINTER(uint)])(1,C.byref(framebuffer))\n  glfun('glDeleteTextures',None,[integer,C.POINTER(uint)])(2,textures)\n  glfun('glDeleteProgram',None,[uint])(program)\n  for shader in compiled:glfun('glDeleteShader',None,[uint])(shader)\n  self.varying_probe={'source_vertex_sha256':hashlib.sha256(vertex_source).hexdigest(),'source_fragment_sha256':hashlib.sha256(original_fragment).hexdigest(),'fragment_source':fragment_source.decode(),'matrix':list(matrix),'prefix_rgba':prefix,'observations':[{'instance':i,'uv_alpha_reciprocal_w':varyings[i],'f32_bits':[hex(struct.unpack('<I',struct.pack('<f',x))[0]) for x in varyings[i]]} for i in [148,328]]}\n"
source = HELPER.read_text()
assert hashlib.sha256(source.encode()).hexdigest() == PIN
anchor = "    exec(\n"
assert source.count(anchor) == 1
injection = """    target="  self.gles.f['glDrawArraysInstanced'](*vals)"
    assert code.count(target)==1
    code=code.replace(target,PROBE+target)
    code=code.replace(schema,schema+",'varying_probe':p.varying_probe")
"""
source = source.replace(anchor, injection + anchor)
with tempfile.TemporaryDirectory(prefix="sdocx-watercolor-varying-") as directory:
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
                "__name__": "watercolor_varying_probe",
                "__file__": str(HELPER),
                "PROBE": PROBE,
            },
        )
    except SystemExit as done:
        if done.code not in (None, 0):
            raise
    native = json.loads(report.read_text())
    baseline = json.loads(
        (ROOT / "conformance/watercolor-redraw-frames-native.json").read_text()
    )["cases"]["768x640_2_32"]
    assert native["capture_status"] == "captured_without_host_gl_errors"
    assert native["readback"]["sha256"] == baseline["final_sha256"]
    assert (
        native["native_mask_pass_readback"]["pixels_sha256"] == baseline["mask_sha256"]
    )
    original_prefix = json.loads(
        (ROOT / "conformance/watercolor-precision-pixel-native.json").read_text()
    )["prefix_rgba"]
    diagnostic_prefix = native["varying_probe"].pop("prefix_rgba")
    differences = [
        (i, a, b)
        for i, (a, b) in enumerate(zip(diagnostic_prefix, original_prefix))
        if a != b
    ]
    native["varying_probe"]["original_prefix_count"] = len(original_prefix)
    native["varying_probe"]["matching_prefix_count"] = sum(
        a == b for a, b in zip(diagnostic_prefix, original_prefix)
    )
    native["varying_probe"]["valid_as_original_interpolation_oracle"] = not differences
    native["varying_probe"]["prefix_mismatches"] = differences[:1]
    native["varying_probe"]["diagnostic_prefix_sha256"] = hashlib.sha256(
        bytes(c for pixel in diagnostic_prefix for c in pixel)
    ).hexdigest()
    receipt = {
        "schema": "sdocx-watercolor-varying-probe-v1",
        "frame": [768, 640],
        "pixel_bottom_origin": [44, 631],
        "capture": {
            "script": "tools/watercolor_host/varying_probe.py",
            "script_sha256": hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
            "frame_helper_sha256": PIN,
        },
        "scope": "Diagnostic MRT program extends the shipped fragment shader with a highp UV/alpha/reciprocal-W output, keeps its original mask output and exact shipped vertex source, and compares all 480 mask prefix pixels against the unmodified native control. A mismatch rejects the diagnostic as an original-pipeline UV oracle. Native full mask and final output also required unchanged after restoration. Added shader output is an explicit diagnostic boundary.",
        "native": native["varying_probe"],
        "mask_sha256": baseline["mask_sha256"],
        "final_sha256": baseline["final_sha256"],
    }
    target = ROOT / "conformance/watercolor-varying-native.json"
    target.write_text(json.dumps(receipt, separators=(",", ":")) + "\n")
    print(target, target.stat().st_size)
