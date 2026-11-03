"""Capture full native ColoredPencil Drawing paint and cached depth transitions."""

import argparse
import base64
import hashlib
import json
from pathlib import Path
import sys
import zlib

ROOT = Path(__file__).resolve().parents[2]
HELPER = (
    ROOT
    / "tmp/vector-parity-plan/pen-family-audit/colored-pencil-v4-mesa-paint-v20-public-color-controls/capture_candidate.py"
)
HELPER_SHA = "0ffc0a80181faaba1c31a1b4c7e3592bcc66e63d90bc59431961d060a158dd68"
GEOMETRY = ROOT / "conformance/colored-pencil-v4-source-context-native.json"
DEPTH = ROOT / "conformance/colored-pencil-v4-depth-native.json"

INJECT = """    actual_pen_canvas=native_call(p,'SPen::SPGraphicsFactory::CreatePenCanvas(SPen::IGLMsgQueue*, SPen::ISPBitmap*, int)',[queue,outbmp,1],10_000_000)
    if not actual_pen_canvas:raise RuntimeError('native pen canvas creation returned null')
    gl=ns['selected_drawable']
    source_object=ns['obj']
    factory_count=len(ns['factory_created_pens'])
    original_drawing_depth=p.get(p.drawing+48,'Q')[0]
    if original_drawing_depth or p.get(gl+280,'Q')[0] or p.get(rt+248,'Q')[0]:raise RuntimeError('baseline depth state is not null')
    bitmap_objects={}
    for name,(dw,dh,rgba) in DRAWING_DEPTH_INPUTS.items():
        data=p.alloc(len(rgba));p.u.mem_write(data,rgba)
        before=len(upload_rows)
        bitmap_object=call_stack(CREATE_BITMAP,[queue,dw,dh,fmt,data,1,1024,1],[6,2])
        uploads=[row for row in upload_rows[before:] if row.get('pixel_pointer')==hex(data)]
        if len(uploads)!=1 or uploads[0]['upload_sha256']!=hashlib.sha256(rgba).hexdigest():raise RuntimeError('depth bitmap upload differs')
        bitmap_objects[name]=bitmap_object
        DRAWING_DEPTH_PROOF[name]=uploads[0]
    bitmap_names={pointer:name for name,pointer in bitmap_objects.items()}
    def depth_name(pointer):
        if not pointer:return None
        if pointer not in bitmap_names:raise RuntimeError(f'unknown native depth pointer {pointer:#x}')
        return bitmap_names[pointer]
    attachment=C.c_int()
    bind_fb=mesa_scope['glfunc']('glBindFramebuffer',None,[C.c_uint,C.c_uint])
    bind_tex=mesa_scope['glfunc']('glBindTexture',None,[C.c_uint,C.c_uint])
    bind_fb(0x8d40,binding.value)
    get_attachment(0x8d40,0x8ce0,0x8cd1,C.byref(attachment))
    target_texture=attachment.value
    upload=mesa_scope['glfunc']('glTexSubImage2D',None,[C.c_uint,C.c_int,C.c_int,C.c_int,C.c_int,C.c_int,C.c_uint,C.c_uint,C.c_void_p])
    read_pixels=mesa_scope['glfunc']('glReadPixels',None,[C.c_int,C.c_int,C.c_int,C.c_int,C.c_uint,C.c_uint,C.c_void_p])
    output=C.create_string_buffer(w*h*4)
    model=p.alloc(36)
    actions=[('drawing_A','drawing','A',8.,0x8044aaee),('drawing_null','drawing',None,16.,0x80c04020),('gl_null','gl',None,8.,0x8044aaee),('rt_null','rt',None,16.,0x80c04020),('rt_clear','clear',None,8.,0x8044aaee),('drawing_B','drawing','B',16.,0x80c04020),('drawing_null_B','drawing',None,8.,0x8044aaee),('drawing_A_again','drawing','A',8.,0xff44aaee)]
    for origin in ['top','bottom']:
        for scale in [1,2]:
            matrix=[scale,0,0,0,-scale if origin=='top' else scale,0,0,h if origin=='top' else 0,1]
            p.u.mem_write(model,struct.pack('<9f',*matrix))
            native_call(p,'SPen::SPPenCanvas::SetMatrix(SPen::Matrix3<float> const&)',[actual_pen_canvas,model])
            for background_name,background in [('clear',[0,0,0,0]),('white',[255]*4),('purple',[64,32,96,128])]:
                sequence=f'{origin}_{scale}_{background_name}'
                bind_fb(0x8d40,binding.value)
                saved_texture=C.c_int();gl_getint(0x8069,C.byref(saved_texture))
                bind_tex(0x0de1,target_texture)
                prior=bytes(background)*(w*h)
                storage=C.create_string_buffer(prior)
                upload(0x0de1,0,0,0,w,h,0x1908,0x1401,storage)
                bind_tex(0x0de1,saved_texture.value)
                read_pixels(0,0,w,h,0x1908,0x1401,output)
                if output.raw!=prior:raise RuntimeError('actual native prior differs')
                for step,(action,target,argument,size,color) in enumerate(actions):
                    phase=f'{sequence}_{step}_{action}'
                    ns['log_vcall'].__globals__['phase']=phase
                    bitmap=bitmap_objects[argument] if argument else 0
                    if target=='drawing':native_call(p,'SPen::ObjectDrawing::SetDepthmap(SPen::ISPBitmap*)',[p.drawing,bitmap])
                    elif target=='gl':native_call(p,'SPen::ColoredPencilStrokeDrawableGLV4::SetDepthMapBitmap(SPen::ISPBitmap*)',[gl,bitmap])
                    elif target=='rt':native_call(p,'SPen::ColoredPencilStrokeDrawableRTV1::SetDepthMapBitmap(SPen::ISPBitmap*)',[rt,bitmap])
                    else:native_call(p,'SPen::ColoredPencilStrokeDrawableRTV1::Clear()',[rt])
                    p.f(0,size)
                    native_call(p,'SPen::ObjectStroke::SetPenSize(float)',[source_object])
                    native_call(p,'SPen::ObjectStroke::SetColor(unsigned int)',[source_object,color])
                    first_draw=len(draw_attribute_joins)
                    native_call(p,'SPen::ObjectDrawing::drawObjectStroke(SPen::ISPCanvas&, SPen::ISPPenCanvas&, SPen::ObjectStroke const*, bool)',[p.drawing,canvas,actual_pen_canvas,source_object,1],50_000_000)
                    if len(draw_attribute_joins)!=first_draw+1:raise RuntimeError('Drawing did not emit exactly one native draw')
                    if len(ns['factory_created_pens'])!=factory_count:raise RuntimeError('Drawing did not reuse its cached pen')
                    native_stream=ns['captured_final_vectors'][phase]
                    source_id=f'size_{int(size)}'
                    expected_stream=DRAWING_SOURCE_STREAMS[source_id]
                    if native_stream['vector_bytes']!=expected_stream.hex():raise RuntimeError('full Drawing source geometry differs')
                    bind_fb(0x8d40,binding.value)
                    read_pixels(0,0,w,h,0x1908,0x1401,output)
                    if mesa_scope['gf']['glGetError']():raise RuntimeError('native Drawing GL error')
                    image=output.raw
                    joined=draw_attribute_joins[-1]
                    if joined['framebuffer_binding']!=binding.value:raise RuntimeError('Drawing painted a different target')
                    DRAWING_RESULTS[phase]={'sequence':sequence,'step':step,'action':action,'target':target,'argument':argument,'source':source_id,'argb':color,'origin':origin,'scale':scale,'frame':[w,h],'background':background,'stream_sha256':hashlib.sha256(expected_stream).hexdigest(),'drawing_depth':depth_name(p.get(p.drawing+48,'Q')[0]),'gl_depth':depth_name(p.get(gl+280,'Q')[0]),'effective_depth':depth_name(p.get(rt+248,'Q')[0]),'background_scale':p.get(rt+264,'f')[0],'prior_verified':True,'native_draw_calls':1,'frame_sha256':hashlib.sha256(image).hexdigest(),'rgba_zlib_base64':base64.b64encode(zlib.compress(image,9)).decode(),'projection_uploads':matrix_uploads[joined['matrix_upload_index']:joined['matrix_upload_index']+1],'uniforms':joined['uniform_setters_since_program_bind']}
                print(sequence,flush=True)
"""


def sha(data):
    return hashlib.sha256(data).hexdigest()


def main():
    source = HELPER.read_text()
    assert sha(source.encode()) == HELPER_SHA
    geometry = json.loads(GEOMETRY.read_bytes())
    streams = {
        row["id"]: bytes.fromhex(row["stream_hex"])
        for row in geometry["cases"]
        if row["id"] in ("size_8", "size_16")
    }
    depth = json.loads(DEPTH.read_bytes())["depth_inputs"]
    maps = {
        "A": (
            depth["grid"]["width"],
            depth["grid"]["height"],
            bytes.fromhex(depth["grid"]["rgba_hex"]),
        ),
        "B": (1, 1, bytes.fromhex(depth["black"]["rgba_hex"])),
    }
    marker = "    return {'schema':'sdocx-colored-pencil-v4-native-rtv1-mesa-draw.v6'"
    assert source.count(marker) == 1
    scope = {
        "__name__": "colored_pencil_drawing_capture",
        "__file__": str(HELPER),
        "DRAWING_DEPTH_INPUTS": maps,
        "DRAWING_DEPTH_PROOF": {},
        "DRAWING_SOURCE_STREAMS": streams,
        "DRAWING_RESULTS": {},
        "base64": base64,
        "zlib": zlib,
    }
    exec(compile(source.replace(marker, INJECT + marker), str(HELPER), "exec"), scope)
    sys.path.insert(0, str(scope["CP_CANDIDATE"].parent))
    scope["preflight"]()
    native = scope["capture"](
        argparse.Namespace(
            argb=0x8044AAEE, width=256, height=256, failure_telemetry=None
        )
    )
    baseline = json.loads(
        (ROOT / "conformance/colored-pencil-paint-native.json").read_bytes()
    )
    reference = next(row for row in baseline["cases"] if row["argb"] == "0x8044aaee")
    assert (
        native["native_draw"]["rgba8_sha256"]
        == reference["native_rgba8_raw_gl_readback_sha256"]
    )
    receipt = {
        "schema": "sdocx-colored-pencil-v4-native-drawing-lifecycle-v1",
        "capture": {
            "script": "tools/colored_pencil/drawing_capture.py",
            "script_sha256": sha(Path(__file__).read_bytes()),
            "base_source_sha256": HELPER_SHA,
            "geometry_receipt_sha256": sha(GEOMETRY.read_bytes()),
            "depth_receipt_sha256": sha(DEPTH.read_bytes()),
        },
        "scope": "Native Model size/color setters, cached Drawing pen selection, native factory-created pen canvas and bitmap inputs, GLV4 saved redraw, native callback task, queued RTV1 update and actual GLES paint. No geometry-buffer, native-field, shader, uniform or scheduler rewrites. Host context/display/identity color conversion, queue execution and glibc/Mesa remain explicit boundaries. Twelve cumulative eight-stroke sequences use three saved ARGB values, sizes 8/16, null/clear/depth replacement operations, both origins and 1x/2x projection over verified actual priors. Not SDK deserialization, full mixed-tool page or device parity.",
        "coordinates": "Bottom-origin premultiplied RGBA8 frames, full zlib/base64.",
        "host": native["host_mesa"],
        "pins": native["pins"],
        "baseline_sha256": native["native_draw"]["rgba8_sha256"],
        "depth_inputs": {
            name: {
                "width": w,
                "height": h,
                "rgba_hex": rgba.hex(),
                "native_upload": scope["DRAWING_DEPTH_PROOF"][name],
            }
            for name, (w, h, rgba) in maps.items()
        },
        "cases": scope["DRAWING_RESULTS"],
    }
    assert len(receipt["cases"]) == 96
    output = ROOT / "conformance/colored-pencil-v4-drawing-lifecycle-native.json"
    temporary = output.with_suffix(".json.tmp")
    temporary.write_text(
        json.dumps(scope["json_safe"](receipt), separators=(",", ":")) + "\n"
    )
    temporary.replace(output)
    print(output, output.stat().st_size)


if __name__ == "__main__":
    main()
