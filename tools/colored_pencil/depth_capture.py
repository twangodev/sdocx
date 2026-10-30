"""Capture ColoredPencil's native optional depth bitmap branch without field writes."""

import argparse
import ast
import base64
import hashlib
import json
from pathlib import Path
import struct
import sys
import zlib

ROOT = Path(__file__).resolve().parents[2]
BATCH = ROOT / "tools/colored_pencil/capture.py"
BATCH_SHA = "0159d2b4a94bbb8e735ef734aeb4b786a75910075d61d8611087f2757ba1689a"
HELPER = (
    ROOT
    / "tmp/vector-parity-plan/pen-family-audit/colored-pencil-v4-mesa-paint-v20-public-color-controls/capture_candidate.py"
)
HELPER_SHA = "0ffc0a80181faaba1c31a1b4c7e3592bcc66e63d90bc59431961d060a158dd68"
GEOMETRY = ROOT / "conformance/colored-pencil-v4-source-context-native.json"


def sha(data):
    return hashlib.sha256(data).hexdigest()


SET_DEPTH = """        meta=BATCH_META[stream_name]
        depth_name=meta['depth']
        if depth_name not in DEPTH_BITMAPS:
            dw,dh,depth_rgba=DEPTH_INPUTS[depth_name]
            depth_pixels=p.alloc(len(depth_rgba));p.u.mem_write(depth_pixels,depth_rgba)
            upload_start=len(upload_rows)
            depth_bitmap=call_stack(CREATE_BITMAP,[queue,dw,dh,fmt,depth_pixels,1,1024,1],[6,2])
            if not depth_bitmap:raise RuntimeError('native depth bitmap constructor returned null')
            uploads=upload_rows[upload_start:]
            matches=[row for row in uploads if row.get('pixel_pointer')==hex(depth_pixels)]
            if len(matches)!=1 or matches[0].get('upload_sha256')!=hashlib.sha256(depth_rgba).hexdigest():
                raise RuntimeError(f'depth upload is not the specified native input: {matches}')
            DEPTH_BITMAPS[depth_name]=(depth_bitmap,matches[0]['bound_texture'])
            DEPTH_PROOF[depth_name]={'upload':matches[0],'dimensions':[dw,dh],'rgba_sha256':hashlib.sha256(depth_rgba).hexdigest()}
        depth_bitmap,depth_texture=DEPTH_BITMAPS[depth_name]
        result=native_call(p,'SPen::ColoredPencilStrokeDrawableRTV1::SetDepthMapBitmap(SPen::ISPBitmap*)',[rt,depth_bitmap])
        if result!=1 or p.get(rt+248,'Q')[0]!=depth_bitmap:raise RuntimeError('native depth setter did not latch bitmap')
        if p.get(rt+256,'2f')!=DEPTH_INPUTS[depth_name][:2]:raise RuntimeError('native depth dimensions differ')
        p.f(0,meta['background_scale'])
        native_call(p,'SPen::ColoredPencilStrokeDrawableRTV1::SetBackgroundScale(float)',[rt])
        if p.get(rt+264,'f')[0]!=meta['background_scale']:raise RuntimeError('native background scale differs')
        p.f(0,meta['size'])
        native_call(p,'SPen::PenDrawableRT::SetPenData(float, int)',[rt,args.argb])
"""

VERIFY_DEPTH = """                    if p.get(rt+248,'Q')[0]!=depth_bitmap:raise RuntimeError('Draw changed depth bitmap')
                    joined=draw_attribute_joins[-1]
                    if joined['textures_by_unit'].get('1')!=depth_texture:raise RuntimeError('Draw did not bind source depth texture at unit 1')
                    sampler=joined['samplers_by_unit'].get(1,joined['samplers_by_unit'].get('1'))
                    state={str(row['pname']):row['value'] for row in sampler_parameters if row['sampler']==sampler}
                    uniforms=joined['uniform_setters_since_program_bind']
                    paper_scales=[u['values'] for u in uniforms if u['name']=='glUniform2fv']
                    if len(paper_scales)!=1:raise RuntimeError('missing native paper-scale upload')
"""

SAVE_DEPTH = """                    BATCH_RESULTS[name].update({'source':meta['source'],'depth':depth_name,'background_scale':meta['background_scale'],'native_depth_pointer_verified':True,'depth_texture_unit':1,'paper_scale_upload':paper_scales[0],'depth_sampler_state':state,'uniforms':uniforms})
"""


def main():
    assert sha(BATCH.read_bytes()) == BATCH_SHA
    source = HELPER.read_text()
    assert sha(source.encode()) == HELPER_SHA
    geometry = json.loads(GEOMETRY.read_bytes())
    patterns = {
        name: (1, 1, bytes([r, 77, 199, 255]))
        for name, r in [("white", 255), ("black", 0), ("half", 128)]
    }
    for name, alpha in [("grid", 255), ("grid_alpha0", 0)]:
        patterns[name] = (
            31,
            17,
            bytes(
                value
                for y in range(17)
                for x in range(31)
                for value in [(x * 37 + y * 61) % 256, 11, 229, alpha]
            ),
        )
    streams, metadata = {}, {}
    for row in geometry["cases"]:
        if row["id"] not in ("size_8", "size_64"):
            continue
        stream = bytes.fromhex(row["stream_hex"])
        assert sha(stream) == row["stream_sha256"]
        selected_size = struct.unpack(
            "<f", bytes.fromhex(row["pen_data"]["size_f32_bits"])
        )[0]
        for depth in patterns:
            for background_scale in (0.5, 0.75, 2.0):
                name = f"{row['id']}_{depth}_paper{background_scale}"
                streams[name] = stream
                metadata[name] = {
                    "source": row["id"],
                    "depth": depth,
                    "background_scale": background_scale,
                    "size": selected_size,
                }
    tree = ast.parse(BATCH.read_text())
    inject = next(
        ast.literal_eval(node.value)
        for node in tree.body
        if isinstance(node, ast.Assign)
        and any(isinstance(t, ast.Name) and t.id == "INJECT" for t in node.targets)
    )
    target = "    for stream_name,stream in BATCH_STREAMS.items():\n"
    assert inject.count(target) == 1
    inject = inject.replace(target, target + SET_DEPTH)
    target = "                    if p.get(rt+248,'Q')[0]:raise RuntimeError('batch populated a depth map')\n"
    assert inject.count(target) == 1
    inject = inject.replace(target, VERIFY_DEPTH)
    target = "'sparse_zlib_base64':base64.b64encode(zlib.compress(sparse,9)).decode()"
    assert inject.count(target) == 1
    inject = inject.replace(
        target, "'rgba_zlib_base64':base64.b64encode(zlib.compress(image,9)).decode()"
    )
    target = "        print(stream_name,len(stream)//20,flush=True)\n"
    assert inject.count(target) == 1
    inject = inject.replace(target, SAVE_DEPTH + target)
    marker = "    return {'schema':'sdocx-colored-pencil-v4-native-rtv1-mesa-draw.v6'"
    assert source.count(marker) == 1
    namespace = {
        "__name__": "colored_pencil_depth_capture",
        "__file__": str(HELPER),
        "BATCH_STREAMS": streams,
        "BATCH_META": metadata,
        "DEPTH_INPUTS": patterns,
        "DEPTH_BITMAPS": {},
        "DEPTH_PROOF": {},
        "BATCH_BACKGROUNDS": {
            "clear": [0, 0, 0, 0],
            "white": [255] * 4,
            "purple": [64, 32, 96, 128],
        },
        "BATCH_SCALES": [1, 2],
        "BATCH_RESULTS": {},
        "base64": base64,
        "zlib": zlib,
    }
    exec(
        compile(source.replace(marker, inject + marker), str(HELPER), "exec"), namespace
    )
    sys.path.insert(0, str(namespace["CP_CANDIDATE"].parent))
    namespace["preflight"]()
    native = namespace["capture"](
        argparse.Namespace(
            argb=0x8044AAEE, width=256, height=256, failure_telemetry=None
        )
    )
    reference = json.loads(
        (ROOT / "conformance/colored-pencil-paint-native.json").read_bytes()
    )
    baseline = next(row for row in reference["cases"] if row["argb"] == "0x8044aaee")
    assert (
        native["native_draw"]["rgba8_sha256"]
        == baseline["native_rgba8_raw_gl_readback_sha256"]
    )
    receipt = {
        "schema": "sdocx-colored-pencil-v4-depth-native-v1",
        "coordinates": "Bottom-origin premultiplied RGBA8 frames, full zlib/base64; depth RGBA bytes in native input row order.",
        "capture": {
            "script": "tools/colored_pencil/depth_capture.py",
            "script_sha256": sha(Path(__file__).read_bytes()),
            "batch_script_sha256": BATCH_SHA,
            "base_source_sha256": HELPER_SHA,
            "geometry_receipt_sha256": sha(GEOMETRY.read_bytes()),
        },
        "scope": "Synthetic native source records and host-specified RGBA depth inputs. Native SPGraphicsFactory bitmap creation/upload, public RTV1 SetDepthMapBitmap/SetBackgroundScale/SetPenData and callback/AddPoint/Update/Draw; no drawable-field, buffer, shader or uniform overwrites. Verifies native upload SHA, latched pointer/dimensions/scale, unit-1 binding and actual prior upload/readback. Existing null-depth baseline remains unchanged. Background scale and depth input are explicit host choices, not inferred saved-note/device context. Setter null is a native no-op, so this separate capture ends with its last non-null map installed.",
        "host": native["host_mesa"],
        "pins": native["pins"],
        "baseline_sha256": native["native_draw"]["rgba8_sha256"],
        "depth_inputs": {
            name: {
                "width": w,
                "height": h,
                "rgba_hex": rgba.hex(),
                "proof": namespace["DEPTH_PROOF"][name],
            }
            for name, (w, h, rgba) in patterns.items()
        },
        "shader_sha256": [
            part["sha256"]
            for row in native["native_paint_material"]["shader_source_payloads"]
            for part in row["parts"]
        ],
        "cases": namespace["BATCH_RESULTS"],
    }
    assert len(receipt["cases"]) == 360
    output = ROOT / "conformance/colored-pencil-v4-depth-native.json"
    temporary = output.with_suffix(".json.tmp")
    temporary.write_text(
        json.dumps(namespace["json_safe"](receipt), separators=(",", ":")) + "\n"
    )
    temporary.replace(output)
    print(output, len(receipt["cases"]), output.stat().st_size)


if __name__ == "__main__":
    main()
