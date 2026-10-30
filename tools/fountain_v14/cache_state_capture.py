"""Capture a short saved stroke first generated after a longer stroke, then cached."""

import ast
import hashlib
import json
from pathlib import Path

import drawing_capture as base

ROOT = Path(__file__).resolve().parents[2]
BASE = ROOT / "tools/fountain_v14/drawing_capture.py"
BASE_SHA = "969fef6e967a41c888aa1121783da525bda435082533733b8807c425eda5358a"
INPUTS = ROOT / "conformance/fountain-v14-saved-scene-inputs.json"

SCENE = """    source_objects={};source_colors={};MODEL_PROOF={}
    for source in SAVED_INPUTS['strokes']:
        source_id=source['index'];payload=bytes.fromhex(source['payload_hex'])
        native_object=p.alloc(256)
        native_call(p,'SPen::ObjectStroke::ObjectStroke()',[native_object]);native_call(p,'SPen::ObjectStroke::Construct()',[native_object])
        data=p.alloc(len(payload));p.u.mem_write(data,payload)
        consumed=native_call(p,'SPen::ObjectStroke::NewApplyBinary(unsigned char*, int, SPen::Orientation, int)',[native_object,data,len(payload),source['orientation'],source['page_width']],5_000_000)
        if consumed!=len(payload):raise RuntimeError('native object application consumed a different size')
        native_call(p,'SPen::ObjectStroke::OnAttach(SPen::ModelContext*, int)',[native_object,ctx,0])
        count=native_call(p,'SPen::ObjectStroke::GetPointCount() const',[native_object])
        if count!=source['point_count']:raise RuntimeError('native point count differs')
        arrays={}
        for field,getter in [('points','GetPoint'),('pressures','GetPressure'),('timestamps','GetTimeStamp'),('tilts','GetTilt'),('orientations','GetOrientation')]:
            pointer=native_call(p,'SPen::ObjectStroke::'+getter+'() const',[native_object])
            values=source['stroke'][field]
            if field=='points':values=[v for point in values for v in (point['x'],point['y'])]
            kind='i' if field=='timestamps' else 'f'
            expected=struct.pack('<'+kind*len(values),*values)
            actual=bytes(p.u.mem_read(pointer,len(expected)))
            if actual!=expected:raise RuntimeError(f'native source array differs: {source_id}/{field}')
            arrays[field]=hashlib.sha256(actual).hexdigest()
        color=native_call(p,'SPen::ObjectStroke::GetColor() const',[native_object])&0xffffffff
        native_call(p,'SPen::ObjectStroke::GetPenSize() const',[native_object])
        size_bits=p.u.reg_read(R.UC_ARM64_REG_S0)
        if size_bits!=struct.unpack('<I',struct.pack('<f',source['pen_width']))[0]:raise RuntimeError('native saved size differs')
        source_objects[source_id]=native_object;source_colors[source_id]=color
        MODEL_PROOF[source_id]={'consumed':consumed,'point_count':count,'color_argb':color,'size_f32_bits':size_bits,'array_sha256':arrays}
    STREAMS={};SCENE_MODEL.update(MODEL_PROOF)
    matrix=p.alloc(36);sequence=[29,3,29,30]
    for origin in ['top']:
      for scale in [1]:
        source_origin=[80,72] if scale==1 else [104,168]
        sy=-scale if origin=='top' else scale
        transform=[scale,0,0,0,sy,0,-source_origin[0]*scale,(h if origin=='top' else 0)-source_origin[1]*sy,1]
        p.put(matrix,'9f',*transform)
        native_call(p,'SPen::SPPenCanvas::SetMatrix(SPen::Matrix3<float> const&)',[pen_canvas,matrix])
        for prior_name,prior in [('clear', PRIORS['clear'])]:
          bind_fb(0x8d40,binding.value)
          saved=C.c_int();gl_getint(0x8069,C.byref(saved))
          bind_tex(0x0de1,target_texture);storage=C.create_string_buffer(prior)
          upload(0x0de1,0,0,0,w,h,0x1908,0x1401,storage);bind_tex(0x0de1,saved.value)
          read_pixels(0,0,w,h,0x1908,0x1401,read)
          if read.raw!=prior:raise RuntimeError('actual output prior differs')
          for step,source_id in enumerate(sequence):
            name=f'scene_{origin}_{scale}_{prior_name}_{step}_{source_id}'
            first_draw=len(draw_attribute_joins);first_stamp=len(native_streams)
            before=read.raw
            native_call(p,'SPen::ObjectDrawing::drawObjectStroke(SPen::ISPCanvas&, SPen::ISPPenCanvas&, SPen::ObjectStroke const*, bool)',[p.drawing,canvas,pen_canvas,source_objects[source_id],1],50_000_000)
            draws=draw_attribute_joins[first_draw:];stream_key=None;mode=None
            if len(draws)>1:raise RuntimeError('native direct Drawing emitted multiple draws')
            if draws:
              draw=draws[0]
              if draw['framebuffer_binding']!=binding.value:raise RuntimeError('native Drawing target differs')
              attribute=next(row for row in draw['attributes'] if row['index']==2)
              upload_row=buffer_store[attribute['array_buffer']]
              stream=upload_row['bytes'];stream_key=f'{source_id}_{scale}_{step}'
              if len(stream)!=draw['instances']*36:raise RuntimeError('native stream/instance count differs')
              if len(native_streams)>first_stamp and state['stream']!=stream:raise RuntimeError('native AddPoint/VBO bytes differ')
              observed={'sha256':hashlib.sha256(stream).hexdigest(),'stream_hex':stream.hex(),'records':draw['instances']}
              if stream_key in STREAMS and STREAMS[stream_key]!=observed:raise RuntimeError('cached object stream changed')
              STREAMS[stream_key]=observed
              mode=DRAW_MODES[-1]
              if mode!=[False,True]:raise RuntimeError('native paint mode differs')
            bind_fb(0x8d40,binding.value);read_pixels(0,0,w,h,0x1908,0x1401,read)
            if mesa_scope['gf']['glGetError']():raise RuntimeError('native Drawing GL error')
            rgba=read.raw
            RESULTS[name]={'origin':origin,'scale':scale,'source_origin':source_origin,'prior':prior_name,'step':step,'source':source_id,'prefix':sequence[:step+1],'stream':stream_key,'argument_count':len(native_streams)-first_stamp,'draw_mode':mode,'native_draws':len(draws),'native_counts':{key:state[key] for key in ['clear_calls','init_calls','canvas_calls']},'before_sha256':hashlib.sha256(before).hexdigest(),'output':{'sha256':hashlib.sha256(rgba).hexdigest(),'rgba_zlib_base64':base64.b64encode(zlib.compress(rgba,9)).decode()}}
            print(name,'draws',len(draws),'arguments',len(native_streams)-first_stamp,flush=True)
    SCENE_STREAMS.update(STREAMS)
"""


def main():
    source = BASE.read_text()
    assert hashlib.sha256(source.encode()).hexdigest() == BASE_SHA
    inputs = json.loads(INPUTS.read_bytes())
    assert [row["index"] for row in inputs["strokes"]] == [3, 6, 7, 18, 19, 28, 29, 30]
    injection = base.INJECT[: base.INJECT.index("    matrix=p.alloc(36)")] + SCENE
    function = next(
        node
        for node in ast.parse(source).body
        if isinstance(node, ast.FunctionDef) and node.name == "main"
    )
    body = ast.get_source_segment(source, function)
    replacements = [
        ("priors = {", "priors = {"),
        (
            "    drawing_path = ROOT",
            "    assert inputs['manager_hex']==manager.hex()\n    priors={\n        'clear':bytes(128*256*4), 'white':bytes([255])*(128*256*4),\n        'checker':b''.join(bytes([64,32,96,128] if ((x//3)+(y//5))%2 else [220,190,160,255]) for y in range(256) for x in range(128))\n    }\n    drawing_path = ROOT",
        ),
        (
            "cp_baseline = bytearray(64 * 64 * 4)",
            "cp_baseline = bytearray(128 * 256 * 4)",
        ),
        ("if x < 64 and 255 - y < 64:", "if x < 128 and 255 - y < 256:"),
        ("i = ((255 - y) * 64 + x) * 4", "i = ((255 - y) * 128 + x) * 4"),
        (
            '"DRAW_MODES": [],',
            '"DRAW_MODES": [], "SAVED_INPUTS":inputs,"SCENE_MODEL":scene_model,"SCENE_STREAMS":scene_streams,',
        ),
        ("width=64, height=64", "width=128, height=256"),
        (
            '"sdocx-fountain-v14-persisted-drawing-paint-native-v1"',
            '"sdocx-fountain-v14-short-reuse-native-v1", "frame":[128,256]',
        ),
        (
            '"script": "tools/fountain_v14/drawing_capture.py"',
            '"script": "tools/fountain_v14/cache_state_capture.py", "connected_capture_sha256":BASE_SHA, "inputs_sha256":sha(INPUTS.read_bytes()), "rust_input_adapter_source_sha256":sha((ROOT/"crates/sdocx/examples/saved_ink_inputs.rs").read_bytes())',
        ),
        (
            '"source_stroke": reference["source_stroke"]',
            '"inputs":"conformance/fountain-v14-saved-scene-inputs.json", "model":scene_model,"streams":scene_streams',
        ),
        (
            '"prior_inputs": reference["prior_inputs"]',
            '"prior_inputs":{key:{"sha256":sha(value),"rgba_zlib_base64":base64.b64encode(zlib.compress(value,9)).decode()} for key,value in priors.items()}',
        ),
        (
            'row["draw_mode"] is None or row["draw_mode"][0] is False',
            'row["draw_mode"] is None or row["draw_mode"]==[False,True]',
        ),
        (
            "conformance/fountain-v14-drawing-paint-native.json",
            "conformance/fountain-v14-cache-state-native.json",
        ),
    ]
    replacements.append(("assert len(receipt[\"cases\"]) == 108", "assert len(receipt[\"cases\"]) == 4"))
    for old, new in replacements:
        body = base.replace(body, old, new)
    # Original single-object provenance is replaced by the selected raw inputs.
    body = body.replace(
        '"source_provenance": reference["source_provenance"]',
        '"source_provenance":{"document":"tmp/stroke-conformance/handwritten.sdocx","document_sha256":inputs["document_sha256"],"stored_indices":[row["index"] for row in inputs["strokes"]]}',
    )
    original_scope = next(
        ast.literal_eval(value)
        for node in ast.walk(function)
        if isinstance(node, ast.Dict)
        for key, value in zip(node.keys, node.values)
        if isinstance(key, ast.Constant) and key.value == "scope"
    )
    body = body.replace(
        original_scope,
        "Eight unchanged saved Fountain V14 objects with original yellow/green/omitted-black colors, native StringID table and per-object NewApplyBinary/OnAttach; no public Model style/color setters. Native channel getter bytes match Rust's unmodified decoded Stroke channels. One cached native Drawing/GL/RT paints source 29, source 3, cached source 29, then source 30, at one crop and scale, with a clear initial prior and cumulative later frames. Actual instance VBOs are joined to native AddPoint or cached source streams. Frame 128x256. Active Fountain drawable fields, geometry, shader, uniforms and samplers are unmodified; inherited host context/loader/allocation/queue/Renderer singleton services remain explicit. This is an ordered saved-object excerpt, not outer SDK note/page loading, a full page, mixed drawing-tool families or device parity.",
    )
    scope = dict(
        vars(base),
        __file__=str(Path(__file__).resolve()),
        INJECT=injection,
        inputs=inputs,
        scene_model={},
        scene_streams={},
        BASE_SHA=BASE_SHA,
        INPUTS=INPUTS,
    )
    exec(compile(body, str(BASE), "exec"), scope)
    scope["main"]()


if __name__ == "__main__":
    main()
