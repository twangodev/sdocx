"""Capture ColoredPencil V4 native record paint over verified prior RGBA8."""

import argparse
import base64
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import zlib

ROOT = Path(__file__).resolve().parents[2]
HELPER = (
    ROOT
    / "tmp/vector-parity-plan/pen-family-audit/colored-pencil-v4-mesa-paint-v20-public-color-controls/capture_candidate.py"
)
HELPER_SHA = "0ffc0a80181faaba1c31a1b4c7e3592bcc66e63d90bc59431961d060a158dd68"
COLORS = [0x8044AAEE, 0xFF44AAEE, 0x0044AAEE, 0x80C04020]
BACKGROUNDS = {"clear": [0, 0, 0, 0], "white": [255] * 4, "purple": [64, 32, 96, 128]}
parser = argparse.ArgumentParser()
parser.add_argument("--child", action="store_true")
parser.add_argument("--argb", type=lambda s: int(s, 0), default=COLORS[0])
parser.add_argument("--output", type=Path)
parser.add_argument("--width", type=int, default=256)
parser.add_argument("--height", type=int, default=256)
parser.add_argument("--scale", type=int, choices=[1, 2])
args = parser.parse_args()


def sha(data):
    return hashlib.sha256(data).hexdigest()


INJECT = """    if binding.value==0:raise RuntimeError('native output FBO is zero')
    attachment=C.c_int()
    get_attachment(0x8d40,0x8ce0,0x8cd1,C.byref(attachment))
    target_texture=attachment.value
    if target_texture<=0:raise RuntimeError('native output has no texture attachment')
    bind_fb=mesa_scope['glfunc']('glBindFramebuffer',None,[C.c_uint,C.c_uint])
    bind_tex=mesa_scope['glfunc']('glBindTexture',None,[C.c_uint,C.c_uint])
    upload=mesa_scope['glfunc']('glTexSubImage2D',None,[C.c_uint,C.c_int,C.c_int,C.c_int,C.c_int,C.c_int,C.c_uint,C.c_uint,C.c_void_p])
    read_pixels=mesa_scope['glfunc']('glReadPixels',None,[C.c_int,C.c_int,C.c_int,C.c_int,C.c_uint,C.c_uint,C.c_void_p])
    output_buffer=C.create_string_buffer(w*h*4)
    for stream_name,stream in BATCH_STREAMS.items():
        next_callback=p.alloc(0x80)
        native_call(p,PEN_RETURN_CALLBACK_CTOR,[next_callback,rt,0])
        native_call(p,PEN_RETURN_CALLBACK_GET_BUFFER,[next_callback])
        next_vector=p.x(0)
        if p.get(next_vector,'3Q')!=(0,0,0):raise RuntimeError('callback vector is not empty')
        native_call(p,PEN_DRAWABLE_SET_BUFFER,[rt,next_vector,0])
        for center_x,center_y,half_size,row,alpha in struct.iter_unpack('<5f',stream):
            for lane,value in enumerate([center_x,center_y,half_size*2.,alpha]):p.f(lane,value)
            native_call(p,CP_ADDPOINT,[rt,int(row)])
        begin,end,capacity=p.get(next_vector,'3Q')
        if bytes(p.u.mem_read(begin,end-begin))!=stream:raise RuntimeError('native AddPoint records differ')
        native_call(p,PEN_DRAWABLE_SEND_DATA_TO_GPU,[rt,next_vector,0])
        native_call(p,RT_UPDATE,[rt,0],5_000_000)
        for origin in ['top','bottom']:
            for scale in BATCH_SCALES:
                transform=[scale,0,0,0,0,-scale if origin=='top' else scale,0,0,0,0,1,0,0,h if origin=='top' else 0,0,1]
                p.u.mem_write(mat,struct.pack('<16f',*transform))
                for background_name,background in BATCH_BACKGROUNDS.items():
                    prior=bytes(background)*(w*h)
                    bind_fb(0x8d40,binding.value)
                    saved_texture=C.c_int();gl_getint(0x8069,C.byref(saved_texture))
                    bind_tex(0x0de1,target_texture)
                    source_buffer=C.create_string_buffer(prior)
                    upload(0x0de1,0,0,0,w,h,0x1908,0x1401,source_buffer)
                    bind_tex(0x0de1,saved_texture.value)
                    read_pixels(0,0,w,h,0x1908,0x1401,output_buffer)
                    if output_buffer.raw!=prior:raise RuntimeError('prior upload/readback differs')
                    before=len(matrix_uploads)
                    native_call(p,RT_DRAW,[rt,subbitmap,mat],10_000_000)
                    after_binding=C.c_int();gl_getint(0x8ca6,C.byref(after_binding))
                    if after_binding.value!=binding.value:raise RuntimeError('Draw changed its destination FBO')
                    read_pixels(0,0,w,h,0x1908,0x1401,output_buffer)
                    if mesa_scope['gf']['glGetError']():raise RuntimeError('batch native GL error')
                    if p.get(rt+248,'Q')[0]:raise RuntimeError('batch populated a depth map')
                    image=output_buffer.raw
                    sparse=b''.join(struct.pack('<I',i//4)+image[i:i+4] for i in range(0,len(image),4) if image[i:i+4]!=bytes(background))
                    name=f'{stream_name}_{origin}_{scale}_{background_name}'
                    BATCH_RESULTS[name]={'source':stream_name,'argb':args.argb,'origin':origin,'scale':scale,'frame':[w,h],'background':background,'records':len(stream)//20,'stream_sha256':hashlib.sha256(stream).hexdigest(),'frame_sha256':hashlib.sha256(image).hexdigest(),'sparse_zlib_base64':base64.b64encode(zlib.compress(sparse,9)).decode(),'changed_pixels':len(sparse)//8,'prior_verified':True,'matrix':transform,'projection_uploads':matrix_uploads[before:]}
        print(stream_name,len(stream)//20,flush=True)
    native_call(p,PEN_DRAWABLE_SET_BUFFER,[rt,vector,0])
    native_call(p,PEN_DRAWABLE_SEND_DATA_TO_GPU,[rt,vector,0])
    native_call(p,RT_UPDATE,[rt,0],5_000_000)
"""

if args.child:
    source = HELPER.read_text()
    assert sha(source.encode()) == HELPER_SHA
    fixture = json.loads((ROOT / "conformance/colored-pencil-native.json").read_text())
    binary = (ROOT / "conformance/colored-pencil-native.bin").read_bytes()
    streams = {}
    for case in fixture["cases"]:
        if (args.argb != COLORS[0] or args.width != 256) and case[
            "id"
        ] != "saved-glv4-size-8":
            continue
        meta = case["native_stream"]
        streams[case["id"]] = binary[meta["offset"] : meta["offset"] + meta["length"]]
    marker = "    return {'schema':'sdocx-colored-pencil-v4-native-rtv1-mesa-draw.v6'"
    assert source.count(marker) == 1
    namespace = {
        "__name__": "colored_pencil_batch",
        "__file__": str(HELPER),
        "BATCH_STREAMS": streams,
        "BATCH_BACKGROUNDS": BACKGROUNDS if args.width == 256 else {"white": [255] * 4},
        "BATCH_SCALES": [args.scale] if args.scale else [1, 2],
        "BATCH_RESULTS": {},
        "base64": base64,
        "zlib": zlib,
    }
    exec(
        compile(source.replace(marker, INJECT + marker), str(HELPER), "exec"), namespace
    )
    native_args = argparse.Namespace(
        argb=args.argb, width=args.width, height=args.height, failure_telemetry=None
    )
    sys.path.insert(0, str(namespace["CP_CANDIDATE"].parent))
    namespace["preflight"]()
    native = namespace["capture"](native_args)
    reference = json.loads(
        (ROOT / "conformance/colored-pencil-paint-native.json").read_text()
    )
    color = next(row for row in reference["cases"] if int(row["argb"], 0) == args.argb)
    if args.width == args.height == 256:
        assert (
            native["native_draw"]["rgba8_sha256"]
            == color["native_rgba8_raw_gl_readback_sha256"]
        )
    material = native["native_paint_material"]
    report = {
        "results": namespace["BATCH_RESULTS"],
        "host": native["host_mesa"],
        "pins": native["pins"],
        "baseline_sha256": native["native_draw"]["rgba8_sha256"],
        "shader_sha256": [
            part["sha256"]
            for row in material["shader_source_payloads"]
            for part in row["parts"]
        ],
    }
    args.output.write_text(
        json.dumps(namespace["json_safe"](report), separators=(",", ":"))
    )
    raise SystemExit

receipt = {
    "schema": "sdocx-colored-pencil-v4-canvas-native-v1",
    "coordinates": "Bottom-origin premultiplied RGBA8; changed pixels encoded as u32 index plus RGBA8, zlib/base64.",
    "capture": {
        "script": "tools/colored_pencil/capture.py",
        "script_sha256": sha(Path(__file__).read_bytes()),
        "base_source_sha256": HELPER_SHA,
        "geometry_receipt_sha256": sha(
            (ROOT / "conformance/colored-pencil-native.json").read_bytes()
        ),
        "geometry_binary_sha256": sha(
            (ROOT / "conformance/colored-pencil-native.bin").read_bytes()
        ),
    },
    "scope": "Captured source-owned GLV4 records replayed through fresh native callback vectors and AddPoint, then unmodified RTV1 Update/Draw/shaders/textures. Native public ObjectStroke color route; depth map remains null. Each premultiplied prior is uploaded to and verified on the actual native target before Draw. Both source origins, 1x/2x projection and three backdrops. Host glibc/Mesa, not saved-note Drawing context or device fidelity.",
    "cases": {},
}
with tempfile.TemporaryDirectory(prefix="sdocx-colored-pencil-native-") as directory:
    directory = Path(directory)
    controls = [(argb, 256, 256, None) for argb in COLORS] + [
        (COLORS[0], 384, 320, 1),
        (COLORS[0], 768, 640, 2),
    ]
    for argb, width, height, scale in controls:
        output = directory / f"{width}x{height}_{argb:08x}.json"
        process = subprocess.run(
            [
                sys.executable,
                str(Path(__file__).resolve()),
                "--child",
                "--argb",
                hex(argb),
                "--output",
                str(output),
                "--width",
                str(width),
                "--height",
                str(height),
            ]
            + (["--scale", str(scale)] if scale else []),
            cwd=ROOT,
            env=os.environ,
            capture_output=True,
            text=True,
            timeout=180,
        )
        if process.returncode:
            print(process.stdout[-2000:])
            print(process.stderr[-6000:])
            raise SystemExit(process.returncode)
        native = json.loads(output.read_text())
        for name, value in native["results"].items():
            receipt["cases"][
                (f"{width}x{height}_" if width != 256 else "") + f"{argb:08x}_{name}"
            ] = value
        receipt.setdefault("host", native["host"])
        receipt.setdefault("pins", native["pins"])
        receipt.setdefault("shader_sha256", native["shader_sha256"])
        print(f"{width}x{height} {argb:08x}", len(native["results"]), flush=True)
    output = ROOT / "conformance/colored-pencil-v4-canvas-native.json"
    temporary = output.with_suffix(".json.tmp")
    temporary.write_text(json.dumps(receipt, separators=(",", ":")) + "\n")
    temporary.replace(output)
    print(output, output.stat().st_size)
