"""Capture a persisted Fountain object through native Drawing and GLES paint."""

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
HELPER = (
    ROOT
    / "tmp/vector-parity-plan/pen-family-audit/colored-pencil-v4-mesa-paint-v20-public-color-controls/capture_candidate.py"
)
HELPER_SHA = "0ffc0a80181faaba1c31a1b4c7e3592bcc66e63d90bc59431961d060a158dd68"
PERSISTED = (
    ROOT
    / "tmp/conformance-cleanup/native-capture-a241793/conformance/fountain_persisted_drawing_native.py"
)
PERSISTED_SHA = "9e0ef723a936b697c65b2ed0c476123287430fc2ff9d66b41945c2815ca5f59a"
SAVED_SHA = "de23cf21f239fc9e12794bc55777fa18406d1129162f8198b1544b8f009f932d"
REFERENCE = ROOT / "conformance/fountain-v14-scaled-source-rgba-native.json"
TAG = b"com.samsung.android.sdk.pen.pen.preload.FountainPen;0;"

INJECT = """    if raw!=CP_BASELINE:raise RuntimeError('original ColoredPencil baseline differs')
    original_ns=ns['log_vcall'].__globals__
    original_ns['run_init'](LIB/'libSPenFountainPen.so',0)
    def fountain_dlopen():
        name=original_ns['read_cstr'](p.x(0))
        if name!='libSPenFountainPen.so':raise RuntimeError(f'unapproved DSO {name}')
        BRIDGES['loader'].append({'function':'dlopen','name':name})
        p.x(0,0x621ec)
    def fountain_dlsym():
        name=original_ns['read_cstr'](p.x(1))
        if p.x(0)!=0x621ec or name!='createPenInst':raise RuntimeError(f'unapproved symbol {name}')
        BRIDGES['loader'].append({'function':'dlsym','name':name,'address':'0x621ec'})
        p.x(0,0x621ec)
    p.callbacks['dlopen']=fountain_dlopen;p.callbacks['dlsym']=fountain_dlsym
    def guest_memchr():
        pointer,value,length=p.x(0),p.x(1)&255,p.x(2)
        if length>1_000_000:raise RuntimeError('memchr length bound')
        data=bytes(p.u.mem_read(pointer,length)) if length else b''
        index=data.find(bytes([value]))
        p.x(0,pointer+index if index>=0 else 0)
        BRIDGES['memchr_calls']+=1
    p.callbacks['memchr']=guest_memchr
    def guest_gl_get_boolean():
        enum,pointer=p.x(0),p.x(1)
        count={0x0c11:1,0x0c23:4}.get(enum)
        if count is None:raise RuntimeError(f'unsupported Boolean query {enum:#x}')
        values=(C.c_ubyte*count)()
        mesa_scope['glfunc']('glGetBooleanv',None,[C.c_uint,C.POINTER(C.c_ubyte)])(enum,values)
        p.u.mem_write(pointer,bytes(values))
        BRIDGES['boolean_queries'].append({'enum':enum,'values':list(values)})
    p.callbacks['glGetBooleanv']=guest_gl_get_boolean
    p.callbacks.pop('SPen::SPGraphicsFactory::CreatePenCanvas(SPen::IGLMsgQueue*, SPen::ISPBitmap*, int)',None)
    for symbol in ('uuid_create','uuid_make','uuid_export','uuid_destroy'):p.callbacks[symbol]=lambda:p.x(0,0)
    mgr=p.alloc(8)
    native_call(p,'SPen::StringIDManager::StringIDManager()',[mgr])
    native_call(p,'SPen::StringIDManager::Construct()',[mgr])
    mb=p.alloc(len(MANAGER_BYTES));p.u.mem_write(mb,MANAGER_BYTES)
    native_call(p,'SPen::StringIDManager::ApplyBinary(unsigned char*, int)',[mgr,mb,len(MANAGER_BYTES)])
    ctx=p.alloc(0x800);native_call(p,'SPen::ModelContext::ModelContext()',[ctx])
    native_call(p,'SPen::ModelContext::SetStringIDManager(SPen::StringIDManager*)',[ctx,mgr])
    obj=p.alloc(256);native_call(p,'SPen::ObjectStroke::ObjectStroke()',[obj]);native_call(p,'SPen::ObjectStroke::Construct()',[obj])
    data=p.alloc(len(PAYLOAD));p.u.mem_write(data,PAYLOAD)
    consumed=native_call(p,'SPen::ObjectStroke::NewApplyBinary(unsigned char*, int, SPen::Orientation, int)',[obj,data,len(PAYLOAD),0,1848],5_000_000)
    if consumed!=len(PAYLOAD):raise RuntimeError(f'NewApplyBinary consumed {consumed}')
    native_call(p,'SPen::ObjectStroke::OnAttach(SPen::ModelContext*, int)',[obj,ctx,0])
    if native_call(p,'SPen::ObjectStroke::GetPointCount() const',[obj])!=161:raise RuntimeError('wrong source count')
    if native_call(p,'SPen::ObjectStroke::GetColor() const',[obj])&0xffffffff!=0xff000000:raise RuntimeError('wrong original color')
    pen_canvas=native_call(p,'SPen::SPGraphicsFactory::CreatePenCanvas(SPen::IGLMsgQueue*, SPen::ISPBitmap*, int)',[queue,outbmp,1],10_000_000)
    state={'clear_calls':0,'init_calls':0,'canvas_calls':0,'rt':0,'factory_return':None,'pen':0}
    native_streams=[];pending_stamps={}
    def observe_fountain(u,address,size,data):
        if address==0x621ec:state['factory_return']=u.reg_read(R.UC_ARM64_REG_LR)
        elif address==state['factory_return']:
            state['pen']=p.x(0);state['factory_return']=None
        if address==0x10492e8:
            pointer=p.get(u.reg_read(R.UC_ARM64_REG_X29)-16,'Q')[0]
            value=bytes(u.mem_read(pointer,len(TAG)+1))
            if value!=TAG+b'\\0':raise RuntimeError('native PenUtil converted tag differs')
            CONVERTED_TAGS[pointer]=value
        if address==0x9dcf4:state['rt']=p.x(0);state['canvas_calls']+=1
        if address==0x9d1f0:state['init_calls']+=1
        if address==0x9de34:state['clear_calls']+=1
        if address==0x9de50:
            pending_stamps[u.reg_read(R.UC_ARM64_REG_LR)]=[p.x(0),[struct.unpack('<f',struct.pack('<I',u.reg_read(getattr(R,f'UC_ARM64_REG_S{i}'))))[0] for i in range(6)]]
        elif address in pending_stamps:
            stamp_rt,arguments=pending_stamps.pop(address)
            native_streams.append(arguments)
            vector=p.get(stamp_rt+80,'Q')[0]
            begin,end,capacity=p.get(vector,'3Q')
            state['stream']=bytes(p.u.mem_read(begin,end-begin)) if end>begin else b''
    p.u.hook_add(UC_HOOK_CODE,observe_fountain)
    original_instanced=p.callbacks['glDrawArraysInstanced']
    def observed_instanced():
        native_rt=state['rt']
        DRAW_MODES.append([bool(p.get(native_rt+68,'B')[0]),bool(p.get(native_rt+25,'B')[0])])
        original_instanced()
    p.callbacks['glDrawArraysInstanced']=observed_instanced
    bind_fb=mesa_scope['glfunc']('glBindFramebuffer',None,[C.c_uint,C.c_uint])
    bind_tex=mesa_scope['glfunc']('glBindTexture',None,[C.c_uint,C.c_uint])
    upload=mesa_scope['glfunc']('glTexSubImage2D',None,[C.c_uint,C.c_int,C.c_int,C.c_int,C.c_int,C.c_int,C.c_uint,C.c_uint,C.c_void_p])
    read_pixels=mesa_scope['glfunc']('glReadPixels',None,[C.c_int,C.c_int,C.c_int,C.c_int,C.c_uint,C.c_uint,C.c_void_p])
    attachment=C.c_int();bind_fb(0x8d40,binding.value)
    get_attachment(0x8d40,0x8ce0,0x8cd1,C.byref(attachment));target_texture=attachment.value
    matrix=p.alloc(36)
    for origin in ['top','bottom']:
      for scale in [1,2]:
       for view,source_origin in [('center',[921-32//scale,1435-32//scale]),('corner',[924,1440]),('outside',[0,0])]:
        sy=-scale if origin=='top' else scale
        transform=[scale,0,0,0,sy,0,-source_origin[0]*scale,(h if origin=='top' else 0)-source_origin[1]*sy,1]
        p.put(matrix,'9f',*transform)
        native_call(p,'SPen::SPPenCanvas::SetMatrix(SPen::Matrix3<float> const&)',[pen_canvas,matrix])
        for color in [0xff000000,0x8044aaee,0x00c04020]:
         native_call(p,'SPen::ObjectStroke::SetColor(unsigned int)',[obj,color])
         for prior_name,prior in PRIORS.items():
          name=f'drawing_{view}_{origin}_{scale}_{color:08x}_{prior_name}'
          bind_fb(0x8d40,binding.value)
          saved=C.c_int();gl_getint(0x8069,C.byref(saved))
          bind_tex(0x0de1,target_texture);storage=C.create_string_buffer(prior)
          upload(0x0de1,0,0,0,w,h,0x1908,0x1401,storage);bind_tex(0x0de1,saved.value)
          read_pixels(0,0,w,h,0x1908,0x1401,read)
          if read.raw!=prior:raise RuntimeError('actual output prior differs')
          first_event=len(actual_gl_events);first_draw=len(draw_attribute_joins);first_stamp=len(native_streams)
          native_call(p,'SPen::ObjectDrawing::drawObjectStroke(SPen::ISPCanvas&, SPen::ISPPenCanvas&, SPen::ObjectStroke const*, bool)',[p.drawing,canvas,pen_canvas,obj,1],50_000_000)
          native_rt=state['rt']
          selected_gl=native_call(p,'SPen::FountainPen::GetStrokeDrawableGL()',[state['pen']])
          if p.get(selected_gl+16,'Q')[0]!=native_rt:raise RuntimeError('Drawing/RT object join differs')
          arguments=native_streams[first_stamp:]
          expected=EXPECTED_ARGUMENTS[scale]
          if arguments and b''.join(struct.pack('<6f',*v) for v in arguments)!=expected:raise RuntimeError(f'complete Drawing stamp arguments differ: {name} {len(arguments)}')
          stream=state['stream']
          if stream!=EXPECTED_STREAMS[scale]:raise RuntimeError(f'complete Drawing stream differs: {name} {len(stream)}')
          events=actual_gl_events[first_event:];draws=draw_attribute_joins[first_draw:]
          if view=='outside':
              if draws:raise RuntimeError('native Drawing did not cull the outside view')
              compact_draws=[];draw_mode=None
          else:
              if len(draws)!=1 or draws[0]['framebuffer_binding']!=binding.value:raise RuntimeError('native Drawing target/draw count differs')
              draw=draws[0]
              for attribute in draw['attributes']:
                  if attribute['index']>=2 and attribute['index']<=5:
                      if attribute['buffer_upload_sha256']!=hashlib.sha256(stream).hexdigest() or attribute['buffer_upload_size']!=len(stream):raise RuntimeError('actual VBO differs from native callback geometry')
              if draw['instances']!=len(stream)//36:raise RuntimeError('native instance count differs')
              projection=matrix_uploads[draw['matrix_upload_index']]
              draw={key:draw[key] for key in ['mode','count','instances','program','viewport','framebuffer_binding','blend_state','uniform_setters_since_program_bind']}
              draw['projection']=projection
              created=max(i for i,event in enumerate(actual_gl_events) if event.get('name')=='glCreateProgram' and event['program']==draw['program'])
              draw['shader_source_sha256']=[]
              for attached_index,attached in enumerate(actual_gl_events):
                  if attached_index>created and attached.get('name')=='glAttachShader' and attached['program']==draw['program']:
                      shader=next(row for row in reversed(shader_source_payloads) if row['shader']==attached['shader'] and row['event_index']<attached_index)
                      shader_bytes=b''.join(part['bytes'] for part in shader['parts'])
                      shader_sha=hashlib.sha256(shader_bytes).hexdigest()
                      SHADERS[shader_sha]={'source':shader_bytes.decode(),'sha256':shader_sha}
                      draw['shader_source_sha256'].append(shader_sha)
              if len(draw['shader_source_sha256'])!=2:raise RuntimeError('active program source join differs')
              compact_draws=[draw];draw_mode=DRAW_MODES[-1]
          bind_fb(0x8d40,binding.value);read_pixels(0,0,w,h,0x1908,0x1401,read)
          if mesa_scope['gf']['glGetError']():raise RuntimeError('Drawing GL error')
          rgba=read.raw
          RESULTS[name]={'paint_mode':'direct','view':view,'origin':origin,'scale':scale,'source_origin':source_origin,'argb':color,'prior':prior_name,'prior_verified':True,'matrix3':transform,'apply_consumed':consumed,'argument_count':len(arguments),'argument_sha256':hashlib.sha256(expected).hexdigest(),'stream_sha256':hashlib.sha256(stream).hexdigest(),'stream_bytes':len(stream),'aa':bool(p.get(native_rt+68,'B')[0]),'redraw':bool(p.get(native_rt+25,'B')[0]),'rt_color':list(p.get(native_rt+44,'4f')),'inverse_scale':list(p.get(native_rt+264,'2f')),'native_counts':{key:state[key] for key in ['clear_calls','init_calls','canvas_calls']},'native_draws':compact_draws,'draw_mode':draw_mode,'gl_draw_names':[e['name'] for e in events if e.get('name') in ['glDrawArrays','glDrawArraysInstanced']],'output':{'sha256':hashlib.sha256(rgba).hexdigest(),'rgba_zlib_base64':base64.b64encode(zlib.compress(rgba,9)).decode()}}
          print(name,'AA',RESULTS[name]['aa'],'redraw',RESULTS[name]['redraw'],RESULTS[name]['gl_draw_names'],flush=True)
"""


def sha(data):
    return hashlib.sha256(data).hexdigest()


def replace(source, old, new):
    assert source.count(old) == 1, old
    return source.replace(old, new)


def main():
    source = HELPER.read_text()
    assert sha(source.encode()) == HELPER_SHA
    persisted = PERSISTED.read_bytes()
    assert sha(persisted) == PERSISTED_SHA
    constants = {
        node.targets[0].id: ast.literal_eval(node.value)
        for node in ast.parse(persisted).body
        if isinstance(node, ast.Assign)
        and len(node.targets) == 1
        and isinstance(node.targets[0], ast.Name)
        and node.targets[0].id in ("OBJECT_PREFIX_HEX", "manager_hex")
    }
    saved = (PERSISTED.parent / "fountain-saved-color-native.json").read_bytes()
    assert sha(saved) == SAVED_SHA
    frame = next(
        r
        for r in json.loads(saved)["real_serialized_frames"]
        if r["id"] == "real_v14_handwritten_flat1445"
    )
    payload = bytes.fromhex(constants["OBJECT_PREFIX_HEX"] + frame["frame_hex"])
    manager = bytes.fromhex(constants["manager_hex"])
    assert (
        sha(payload)
        == "f9666805d1390cfbb7606d6dbdb62bc0c95408e51174058faf1146bfbba66f83"
    )
    reference = json.loads(REFERENCE.read_bytes())
    priors = {
        key: zlib.decompress(base64.b64decode(row["rgba_zlib_base64"]))
        for key, row in reference["prior_inputs"].items()
    }
    drawing_path = ROOT / "conformance/fountain-v14-drawing-scale-native.json"
    drawing = json.loads(drawing_path.read_bytes())
    arguments = {
        scale: b"".join(
            struct.pack("<6f", *v)
            for v in next(
                row for row in drawing["cases"] if row["id"] == f"top_{scale}_crop"
            )["addpoint_arguments"]
        )
        for scale in [1, 2]
    }
    baseline = json.loads(
        (ROOT / "conformance/colored-pencil-paint-native.json").read_bytes()
    )["cases"][0]
    assert baseline["argb"] == "0x8044aaee"
    sparse = baseline["sparse_pixels"]
    data = (ROOT / "conformance/colored-pencil-paint-sparse.bin").read_bytes()
    data = data[sparse["offset"] : sparse["offset"] + sparse["length"]]
    assert sha(data) == sparse["sha256"]
    cp_baseline = bytearray(64 * 64 * 4)
    for x, y, red, green, blue, alpha in struct.iter_unpack("<HH4B", data):
        if x < 64 and 255 - y < 64:
            i = ((255 - y) * 64 + x) * 4
            cp_baseline[i : i + 4] = bytes([red, green, blue, alpha])
    branch = """            elif lr==0x1049318:
                tag=bounded_guest_cstr(tag_pointer,128,'Fountain private canvas tag')
                if CONVERTED_TAGS.get(tag_pointer)!=tag+b'\\0' or tag!=TAG:raise RuntimeError('native converted tag pointer/value join differs')
                cap=128;source_kind='native_PenCommon_converted_string'
                owner={'dso':'libSPenPenCommon.so','caller_lr_rva':'0x49318','conversion_return_rva':'0x492e8','fountain_literal_rva':'0x3b990'}
"""
    source = replace(
        source,
        "            elif lr-COLORED_PENCIL_DSO_BASE in CP_BUILD_TEXTURES_BITMAP_SOURCES:",
        branch
        + "            elif lr-COLORED_PENCIL_DSO_BASE in CP_BUILD_TEXTURES_BITMAP_SOURCES:",
    )
    anchor = "            else:raise RuntimeError(f'unknown CreateBitmap tag ownership/source kind: {source_kind!r}')"
    source = replace(
        source,
        anchor,
        "            elif source_kind=='native_PenCommon_converted_string':\n                if CONVERTED_TAGS.get(pointer)!=raw+b'\\0' or raw!=TAG:raise RuntimeError('native converted tag ownership differs')\n"
        + anchor,
    )
    source = replace(
        source,
        "if lr==GRAPHICS_DSO_BASE+GRAPHICS_STD_STRING_STRCMP_LR_RVA:",
        "if lr in (GRAPHICS_DSO_BASE+GRAPHICS_STD_STRING_STRCMP_LR_RVA,0x97f90):",
    )
    source = replace(
        source,
        "if address==GRAPHICS_DSO_BASE+GRAPHICS_STD_STRING_COMPARE_ENTRY_RVA:",
        "if address in (GRAPHICS_DSO_BASE+GRAPHICS_STD_STRING_COMPARE_ENTRY_RVA,0x97f58):",
    )
    source = replace(
        source,
        "if caller_lr not in {GRAPHICS_DSO_BASE+x for x in GRAPHICS_STD_STRING_COMPARE_LR_RVAS}:",
        "if caller_lr not in {GRAPHICS_DSO_BASE+x for x in GRAPHICS_STD_STRING_COMPARE_LR_RVAS}|{0x97f10,0x97f3c,0x980a4,0x980c4}:",
    )
    marker = "    return {'schema':'sdocx-colored-pencil-v4-native-rtv1-mesa-draw.v6'"
    source = replace(source, marker, INJECT + marker)
    scope = {
        "__name__": "fountain_persisted_drawing_capture",
        "__file__": str(HELPER),
        "MANAGER_BYTES": manager,
        "PAYLOAD": payload,
        "CP_BASELINE": bytes(cp_baseline),
        "TAG": TAG,
        "CONVERTED_TAGS": {},
        "EXPECTED_ARGUMENTS": arguments,
        "EXPECTED_STREAMS": {
            int(key.removeprefix("scale_")): b"".join(
                struct.pack("<9f", *r) for r in row["records"]
            )
            for key, row in reference["native_streams"].items()
        },
        "PRIORS": priors,
        "RESULTS": {},
        "SHADERS": {},
        "DRAW_MODES": [],
        "BRIDGES": {"loader": [], "memchr_calls": 0, "boolean_queries": []},
        "base64": base64,
        "zlib": zlib,
    }
    exec(compile(source, str(HELPER), "exec"), scope)
    sys.path.insert(0, str(scope["CP_CANDIDATE"].parent))
    scope["preflight"]()
    native = scope["capture"](
        argparse.Namespace(argb=0x8044AAEE, width=64, height=64, failure_telemetry=None)
    )
    receipt = {
        "schema": "sdocx-fountain-v14-persisted-drawing-paint-native-v1",
        "capture": {
            "script": "tools/fountain_v14/drawing_capture.py",
            "script_sha256": sha(Path(__file__).read_bytes()),
            "base_helper_sha256": HELPER_SHA,
            "persisted_helper_sha256": PERSISTED_SHA,
            "saved_fixture_sha256": SAVED_SHA,
            "paint_receipt_sha256": sha(REFERENCE.read_bytes()),
            "drawing_scale_receipt_sha256": sha(drawing_path.read_bytes()),
            "cp_baseline_sha256": sha(cp_baseline),
        },
        "scope": "Native Model NewApplyBinary/OnAttach of the original Fountain V14 saved payload, native Drawing factory/selection, GLV14 saved redraw, native callback and queued tasks, RTV4 private bitmap/canvas construction/Init/Update/Draw and actual Mesa GLES readback in one invocation. Only original color and two public Model SetColor controls vary. Both origins/scales and three verified priors. AA/redraw and retained private-canvas lifecycle are observed, not forced. Native fields, geometry buffers, shader, uniform and sampler state are not rewritten. Host context/display/identity color conversion, loader, allocation/ownership and synchronous queue adapters remain explicit. Not outer SDK page/note loading or Samsung device parity.",
        "source_provenance": reference["source_provenance"],
        "source_stroke": reference["source_stroke"],
        "persisted_input": {
            "payload_hex": payload.hex(),
            "payload_sha256": sha(payload),
            "manager_hex": manager.hex(),
            "manager_sha256": sha(manager),
            "page_width": 1848,
        },
        "host": native["host_mesa"],
        "pins": native["pins"],
        "bridges": scope["BRIDGES"],
        "prior_inputs": reference["prior_inputs"],
        "cases": scope["RESULTS"],
        "shaders": scope["SHADERS"],
    }
    assert len(receipt["cases"]) == 108
    assert all(
        row["draw_mode"] is None or row["draw_mode"][0] is False
        for row in receipt["cases"].values()
    )
    output = ROOT / "conformance/fountain-v14-drawing-paint-native.json"
    output.write_text(
        json.dumps(scope["json_safe"](receipt), separators=(",", ":")) + "\n"
    )
    print(output, output.stat().st_size)
    print(
        "During-draw modes:",
        {
            tuple(row["draw_mode"])
            for row in receipt["cases"].values()
            if row["draw_mode"] is not None
        },
    )


if __name__ == "__main__":
    main()
