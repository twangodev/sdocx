"""Capture real-source Fountain RTV4 masks and RGBA over verified priors."""

import base64
import hashlib
import json
from pathlib import Path
import struct
import subprocess
import sys
import tempfile
import zlib

ROOT = Path(__file__).resolve().parents[2]
ARCHIVE = ROOT / "tmp/conformance-cleanup/native-capture-a241793/conformance"
HELPER = ARCHIVE / "fountain_rtv4_paint_native.py"
HELPER_SHA = "d6ccfffec83582270630a89e31ee6182b89d99284fe02aef634c84a7d8e4975f"
BASELINE_SHA = "c2862567fd9df4b2e0c900fb4ce260e6761fa725431bbcb77db2c45f126b52c1"

INJECT = """
if report['readback']['sha256'] != BASELINE_SHA:raise RuntimeError('native original real-source baseline changed')
bind=p.gles.f['glBindFramebuffer']
bind(0x8d40,2)
attachment=C.c_int()
p.gles.gl.glGetFramebufferAttachmentParameteriv(0x8d40,0x8ce0,0x8cd1,C.byref(attachment))
target_texture=attachment.value
if target_texture<=0:raise RuntimeError('output attachment is missing')
upload=ip.cfunc(p.gles.gl,'glTexSubImage2D',None,[C.c_uint,C.c_int,C.c_int,C.c_int,C.c_int,C.c_int,C.c_uint,C.c_uint,C.c_void_p])
for origin in ['top','bottom']:
 for scale in [1,2]:
  inv=p.alloc(8);p.put(inv,'2f',1./scale,1./scale)
  p.call(watch['set_inverse_scale'],[rt,inv])
  producer=p.alloc(24);upload_vector=p.alloc(24)
  p.put(producer,'3Q',0,0,0);p.put(upload_vector,'3Q',0,0,0)
  p.call('SPen::PenDrawableRT::SetBuffer(std::__ndk1::vector<float, std::__ndk1::allocator<float> >*, std::__ndk1::vector<float, std::__ndk1::allocator<float> >*)',[rt,producer,upload_vector])
  for vals in caller_inputs:p.call(add,[rt],vals)
  begin,end,capacity=p.get(producer,'3Q')
  stream=bytes(p.u.mem_read(begin,end-begin))
  if len(stream)!=24*36:raise RuntimeError('native real-source record count changed')
  records=[list(v) for v in struct.iter_unpack('<9f',stream)]
  p.call('SPen::PenDrawableRT::SendDataToGPU(std::__ndk1::vector<float, std::__ndk1::allocator<float> >*, std::__ndk1::vector<float, std::__ndk1::allocator<float> >*)',[rt,producer,upload_vector])
  p.call(watch['update'],[rt,0],instruction_limit=5_000_000)
  source_origin=[921-32//scale,1435-32//scale]
  sy=-scale if origin=='top' else scale
  matrix=[scale,0,0,0,0,sy,0,0,0,0,1,0,-source_origin[0]*scale,(64 if origin=='top' else 0)-source_origin[1]*sy,0,1]
  mm=p.alloc(64);p.put(mm,'16f',*matrix)
  for argb in [0xff000000,0x8044aaee,0x00c04020]:
   p.call(watch['set_pen_data'],[rt,argb],[pen_size])
   for prior_name,prior in PRIOR_INPUTS.items():
    clear_target(1)
    bind(0x8d40,2)
    saved=C.c_int();p.gles.gl.glGetIntegerv(0x8069,C.byref(saved))
    p.gles.gl.glBindTexture(0x0de1,target_texture)
    data=C.create_string_buffer(prior)
    upload(0x0de1,0,0,0,64,64,0x1908,0x1401,data)
    p.gles.gl.glBindTexture(0x0de1,saved.value)
    prior_read=read_target()
    if prior_read['sha256']!=hashlib.sha256(prior).hexdigest():raise RuntimeError('actual native prior differs')
    first=len(p.gl_calls)
    p.call(watch['draw'],[rt,output_subbitmap,mm],instruction_limit=5_000_000)
    calls=p.gl_calls[first:]
    if any(c.get('error') for c in calls if c.get('call','').startswith('errorAfter:')):raise RuntimeError('native Draw GL error')
    output=read_target()
    if output['draw_framebuffer_binding']!=2 or output['read_framebuffer_binding']!=2:raise RuntimeError('native output target changed')
    bind(0x8d40,1);mask=read_target();bind(0x8d40,2)
    if p.get(rt+25,'B')[0]!=1 or p.get(rt+68,'B')[0]!=1:raise RuntimeError('native AA/redraw mode changed')
    name=f'{origin}_{scale}_{argb:08x}_{prior_name}'
    NEW_CASES[name]={'origin':origin,'scale':scale,'source_origin':source_origin,'argb':argb,'prior':prior_name,'frame':[64,64],'inverse_scale':[1./scale]*2,'matrix':matrix,'native_records':records,'stream_sha256':hashlib.sha256(stream).hexdigest(),'projection_uniform':next(c['values'] for c in calls if c.get('call')=='glUniformMatrix4fv'),'color_uniform_calls':[c for c in calls if c.get('call')=='glUniform4fv'],'blend_calls':[c for c in calls if c.get('call') in ['glBlendFuncSeparate','glBlendEquationSeparate']],'prior_verified':True,'native_draw_calls':[c for c in calls if c.get('call') in ['glDrawArrays','glDrawArraysInstanced']],'mask':mask,'output':output}
    print(name,flush=True)
"""


def sha(data):
    return hashlib.sha256(data).hexdigest()


def main():
    source = HELPER.read_bytes()
    assert sha(source) == HELPER_SHA
    adapter = Path("/tmp/sdocx-agent-target/debug/examples/ink_geometry")
    prepared = json.loads(
        subprocess.check_output(
            [str(adapter), str(ROOT / "tmp/stroke-conformance/handwritten.sdocx")]
        )
    )[1445]
    priors = {"clear": bytes(64 * 64 * 4), "white": b"\xff" * (64 * 64 * 4)}
    priors["checker"] = b"".join(
        bytes([64, 32, 96, 128] if ((x // 3) + (y // 5)) % 2 else [220, 190, 160, 255])
        for y in range(64)
        for x in range(64)
    )
    marker = "path=Path(cli_args.capture_output)"
    assert source.decode().count(marker) == 1
    code = source.decode().replace(marker, INJECT + marker)
    with tempfile.TemporaryDirectory(prefix="sdocx-fountain-v14-") as directory:
        sys.path.insert(0, str(ARCHIVE))
        sys.argv = [
            str(HELPER),
            "--capture-case",
            "full-real",
            "--capture-output",
            str(Path(directory) / "full.json"),
        ]
        scope = {
            "__name__": "fountain_v14_rgb_capture",
            "__file__": str(HELPER),
            "PRIOR_INPUTS": priors,
            "NEW_CASES": {},
            "BASELINE_SHA": BASELINE_SHA,
        }
        exec(compile(code, str(HELPER), "exec"), scope)
        native = scope["report"]
    geometry = prepared["geometry"]
    actual_arguments = [
        [point["x"], point["y"], radius, direction["x"], direction["y"], 1.0]
        for point, radius, direction in zip(
            geometry["points"], geometry["dot_radii"], geometry["dot_directions"]
        )
    ]

    def pack(rows):
        return b"".join(struct.pack("<6f", *row) for row in rows)

    assert pack(actual_arguments) == pack(native["input"]["addpoint_s0_s5_f32"])
    streams = {}
    for case in scope["NEW_CASES"].values():
        records = case.pop("native_records")
        stream_id = f"scale_{case['scale']}"
        if stream_id in streams:
            assert streams[stream_id]["records"] == records
        else:
            streams[stream_id] = {"records": records, "sha256": case["stream_sha256"]}
        case["stream"] = stream_id
    receipt = {
        "schema": "sdocx-fountain-v14-real-source-rgba-native-v1",
        "capture": {
            "script": "tools/fountain_v14/capture.py",
            "script_sha256": sha(Path(__file__).read_bytes()),
            "native_helper_sha256": HELPER_SHA,
            "baseline_sha256": BASELINE_SHA,
            "rust_adapter_sha256": sha(adapter.read_bytes()),
            "rust_adapter_source_sha256": sha(
                (ROOT / "crates/sdocx/examples/ink_geometry.rs").read_bytes()
            ),
        },
        "scope": "Real saved V14 traversal tuples replayed through native RTV4 public setters, AddPoint, Update and Draw. Explicit enhanced-AA/redraw=true selects native MAX coverage then source-over color composite. Both source origins, 1x/2x, original black plus two synthetic color/alpha forks and three verified priors. Retained mask and final RGBA are actual host Mesa readbacks. The native geometry producer is a separate retained capture, not a new native Drawing or SDK deserialization pass. Existing queue/context/ownership/GLES adapters remain host boundaries; no shader, uniform, sampler or native drawable field rewrite. Not Samsung device or full saved-page fidelity.",
        "source_provenance": {
            "document": "tmp/stroke-conformance/handwritten.sdocx",
            "document_sha256": sha(
                (ROOT / "tmp/stroke-conformance/handwritten.sdocx").read_bytes()
            ),
            "stored_stroke_index": 1445,
            "geometry_receipt": str(
                (ARCHIVE / "fountain-v14-real-native.json").relative_to(ROOT)
            ),
            "geometry_receipt_sha256": sha(
                (ARCHIVE / "fountain-v14-real-native.json").read_bytes()
            ),
        },
        "native_sources": native["native_sources"],
        "helper_sources": native["helper_sources"],
        "libcxx_sha256": native["libcxx_sha256"],
        "host_gles": native["host_gles"],
        "addpoint_arguments": native["input"]["addpoint_s0_s5_f32"],
        "source_stroke": prepared["stroke"],
        "source_sample_ends": geometry["sample_ends"],
        "native_streams": streams,
        "shader_sha256": [
            sha("".join(row["source"]).encode())
            for row in native["shader_sources_from_native_glShaderSource"]
        ],
        "prior_inputs": {
            name: {
                "sha256": sha(rgba),
                "rgba_zlib_base64": base64.b64encode(zlib.compress(rgba, 9)).decode(),
            }
            for name, rgba in priors.items()
        },
        "cases": scope["NEW_CASES"],
    }
    assert len(receipt["cases"]) == 36
    output = ROOT / "conformance/fountain-v14-real-source-rgba-native.json"
    temporary = output.with_suffix(".json.tmp")
    temporary.write_text(json.dumps(receipt, separators=(",", ":")) + "\n")
    temporary.replace(output)
    print(output, output.stat().st_size)


if __name__ == "__main__":
    main()
