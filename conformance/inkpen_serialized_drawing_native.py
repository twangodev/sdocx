#!/usr/bin/env python3
"""Replay embedded synthetic ObjectStroke frames through native Model and Drawing."""
import argparse, base64, contextlib, hashlib, io, json, os, struct, subprocess, sys
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
HERE=Path(__file__).resolve().parent
CALLER=HERE/'inkpen_saved_caller_native.py'
FIXTURE=HERE/'inkpen-serialized-drawing-native.json'
MODEL_SHA='4fbcf6d4213e929f1535d32abb487743643fd5d0dfc366e50dfeb2e7d8015b7a'
CPP_SHA='4397241b4bd20a8e579bfb41d21107857e12985f6a01ca0c2a5f83380d1270b4'
CALLER_SHA='c0b1657cdcf94bcf2c173ce68ed7817e55a4de246883b47265bb7b4d3f50dc04'
ANCHOR=" p.call('SPen::ObjectStroke::GetAdvancedPenSetting() const',[obj]); advanced_getter=p.x(0);"
def sha(b): return hashlib.sha256(b).hexdigest()

def replace_once(source, old, new, label):
    n=source.count(old)
    if n!=1: raise RuntimeError(f'{label}: expected one source anchor, got {n}')
    return source.replace(old,new,1)

def instrument_event_boundaries(source):
    source=replace_once(source,
        "('::Draw(','::RedrawPen(','::drawLine(','::drawPoint(','::startPen(','::endPen(')))",
        "('::Draw(','::RedrawPen(','::reDrawLine(','::drawLine(','::drawPoint(','::startPen(','::endPen(')))",
        'V1 method registration')
    source=replace_once(source,"native_draw_entries=[]\n",
        "native_draw_entries=[]\nnative_redraw_sample_ranges=[]\npending_redraw_sample_returns={}\nredraw_sample_indices={}\n",
        'V1 event range state')
    entry="  record={'signatures':method_trace_names_by_pc[a],'phase':phase,'pc':hex(a),'regs':regs,'pointer_heads':pointers}\n"
    added=entry+("  for signature in method_trace_names_by_pc[a]:\n"
        "   if 'SPen::InkPenStrokeDrawableGLV1::reDrawLine(' in signature or 'SPen::InkPenStrokeDrawableGLV1::endPen(' in signature:\n"
        "    this=u.reg_read(R.UC_ARM64_REG_X0); rt=struct.unpack('<Q',u.mem_read(this+16,8))[0]; vector_obj=struct.unpack('<Q',u.mem_read(rt+80,8))[0]; begin,end,capacity=struct.unpack('<3Q',u.mem_read(vector_obj,24)); before=end-begin\n"
        "    if not (begin<=end<=capacity) or before%12: raise RuntimeError(f'invalid native V1 vector at event entry {begin:#x}/{end:#x}/{capacity:#x}')\n"
        "    event='reDrawLine' if '::reDrawLine(' in signature else 'endPen'; idx=redraw_sample_indices.get((phase,event),0); redraw_sample_indices[(phase,event)]=idx+1\n"
        "    sample={'method':event,'phase':phase,'source_history_index':idx+1 if event=='reDrawLine' else None,'entry_pc':hex(a),'receiver':hex(this),'rt':hex(rt),'before_count_bytes':before,'before_record_count':before//12,'args_s0_s2_bits':[hex(u.reg_read(getattr(R,f'UC_ARM64_REG_S{i}'))) for i in range(3)],'raw_x3_register':hex(u.reg_read(R.UC_ARM64_REG_X3)),'return_pc':hex(u.reg_read(R.UC_ARM64_REG_LR)),'vector_obj':vector_obj}\n"
        "    pending_redraw_sample_returns[u.reg_read(R.UC_ARM64_REG_LR)]=sample; native_redraw_sample_ranges.append(sample)\n")
    source=replace_once(source,entry,added,'V1 event entry boundary')
    anchor=" if a in pending_inkpen_buffer_returns:\n"
    returns=(" if a in pending_redraw_sample_returns:\n"
        "  sample=pending_redraw_sample_returns.pop(a); vector_obj=sample['vector_obj']; begin,end,capacity=struct.unpack('<3Q',u.mem_read(vector_obj,24)); after=end-begin\n"
        "  if not (begin<=end<=capacity) or after%12 or after<sample['before_count_bytes']: raise RuntimeError(f'invalid native V1 vector at event return {begin:#x}/{end:#x}/{capacity:#x}')\n"
        "  sample.update({'return_pc':hex(a),'after_count_bytes':after,'after_record_count':after//12,'added_record_count':(after-sample['before_count_bytes'])//12,'after_stream_bytes':bytes(u.mem_read(begin,after)).hex() if after else ''})\n"+anchor)
    return replace_once(source,anchor,returns,'V1 event return boundary')
class StopAfterProbe(BaseException): pass

def main(case_id):
    fixture=json.loads(FIXTURE.read_text())
    case=fixture['input']['cases'][case_id]
    caller=CALLER.read_bytes()
    if sha(caller)!=CALLER_SHA: raise RuntimeError(f'caller pin mismatch: {sha(caller)}')
    table=base64.b64decode(fixture['input']['string_table']['bytes'],validate=True)
    frame=base64.b64decode(case['frame']['bytes'],validate=True)
    frame_size=case['frame']['length']
    if len(table)!=fixture['input']['string_table']['length'] or len(frame)!=frame_size or sha(frame)!=case['frame']['sha256']:
        raise RuntimeError('embedded table/frame payload validation failed')
    expected_frame_size=2510 if 'null_setting_v1' in case_id else 2514
    if frame_size!=expected_frame_size:
        raise RuntimeError('unexpected embedded frame length')
    inject=f'''\n # Native serialized-string resolution probe: exact derivative bytes, not host strings.
 if hashlib.sha256(MODEL.read_bytes()).hexdigest()!={MODEL_SHA!r}: raise RuntimeError('Model DSO pin mismatch')
 if hashlib.sha256(CPP.read_bytes()).hexdigest()!={CPP_SHA!r}: raise RuntimeError('libc++ DSO pin mismatch')
 _tab=bytes.fromhex({table.hex()!r}); _frame=bytes.fromhex({frame.hex()!r})
 _sm=p.alloc(8); p.u.mem_write(_sm,bytes(8)); p.call('SPen::StringIDManager::StringIDManager()',[_sm]); p.call('SPen::StringIDManager::Construct()',[_sm])
 _src=p.alloc(len(_tab)); p.u.mem_write(_src,_tab); p.call('SPen::StringIDManager::ApplyBinary(unsigned char*, int)',[_sm,_src,len(_tab)])
 _apply=p.x(0)
 if _apply!=1: raise RuntimeError(f'StringIDManager::ApplyBinary returned {{_apply}}')
 _ctx=p.alloc(0x800); p.u.mem_write(_ctx,bytes(0x800)); p.call('SPen::ModelContext::ModelContext()',[_ctx]); p.call('SPen::ModelContext::SetStringIDManager(SPen::StringIDManager*)',[_ctx,_sm]); p.call('SPen::ModelContext::GetStringIDManager()',[_ctx])
 if p.x(0)!=_sm: raise RuntimeError('ModelContext manager setter/getter mismatch')
 p.callbacks['uuid_create']=lambda:p.x(0,0); p.callbacks['uuid_make']=lambda:p.x(0,0); p.callbacks['uuid_export']=lambda:p.x(0,0); p.callbacks['uuid_destroy']=lambda:p.x(0,0)
 _obj=p.alloc(0x100); p.call('SPen::ObjectStroke::ObjectStroke()',[_obj]); p.call('SPen::ObjectStroke::Construct()',[_obj]); _impl=p.get(_obj+32,'Q')[0]
 _f=p.alloc(len(_frame)); p.u.mem_write(_f,_frame); _h=p.alloc(0x100); p.u.mem_write(_h,bytes(0x100)); p.call('SPen::ObjectStrokeBinaryHandler::ObjectStrokeBinaryHandler(SPen::ObjectStrokeImpl*)',[_h,_impl]); p.f(0,1.0); p.call('SPen::ObjectStrokeBinaryHandler::NewApplyBinary(unsigned char*, int, unsigned int, float)',[_h,_f,len(_frame),0]); _cons=p.x(0)&0xffffffff
 if _cons!=len(_frame): raise RuntimeError(f'NewApplyBinary consumed {{_cons}} bytes')
 p.call('SPen::ObjectStroke::OnAttach(SPen::ModelContext*, int)',[_obj,_ctx,0]); _onattach=p.x(0)
 _ids_before_attach={{'pen_name':p.get(_impl+16,'I')[0],'advanced_setting':p.get(_impl+32,'I')[0]}}
 p.call('SPen::ObjectStroke::GetPenName() const',[_obj]); _name=p.x(0); p.call('SPen::ObjectStroke::GetAdvancedPenSetting() const',[_obj]); _advanced=p.x(0)
 def _utf8(ptr):
  if not ptr:return None
  p.call('SPen::String::GetUTF8Size() const',[ptr]); n=p.x(0); b=p.alloc(n+1); p.call('SPen::String::GetUTF8(char*, int) const',[ptr,b,n+1]); return bytes(p.u.mem_read(b,n+1)).split(bytes([0]),1)[0].decode('utf-8')
 _manager_strings={{}}
 for _i in range(4):
  p.call('SPen::StringIDManager::GetString(unsigned int)',[_sm,_i]); _ptr=p.x(0); _manager_strings[str(_i)]={{'ptr':hex(_ptr),'text':_utf8(_ptr)}}
 _probe_resolved_result={{'context_pointer':hex(_ctx),'context_manager_field':hex(p.get(_ctx+432,'Q')[0]),'manager_wrapper_field':hex(p.get(_sm,'Q')[0]),'manager_apply_return':_apply,'manager_binary_sha256':hashlib.sha256(_tab).hexdigest(),'manager_binary_length':len(_tab),'frame_sha256':hashlib.sha256(_frame).hexdigest(),'new_apply_consumed':_cons,'raw_x0_after_on_attach':_onattach,'manager_strings':_manager_strings,'implementation_ids_before_attach':_ids_before_attach,'implementation_ids':{{'pen_name':p.get(_impl+16,'I')[0],'advanced_setting':p.get(_impl+32,'I')[0]}},'pen_name_pointer_nonnull':bool(_name),'pen_name_utf8':_utf8(_name),'advanced_pointer_nonnull':bool(_advanced),'advanced_setting_utf8':_utf8(_advanced),'native_model_rvas':{{s:hex(p.syms[s]-BASE) for s in ('SPen::StringIDManager::ApplyBinary(unsigned char*, int)','SPen::ModelContext::ModelContext()','SPen::ModelContext::SetStringIDManager(SPen::StringIDManager*)','SPen::ModelContext::GetStringIDManager()','SPen::ObjectStroke::Construct()','SPen::ObjectStroke::OnAttach(SPen::ModelContext*, int)','SPen::ObjectStroke::GetPenName() const','SPen::ObjectStroke::GetAdvancedPenSetting() const')}}}}
 _array_state={{}}
 for _field,_getter,_unit in (('points','SPen::ObjectStroke::GetPoint() const',8),('pressure','SPen::ObjectStroke::GetPressure() const',4),('timestamps','SPen::ObjectStroke::GetTimeStamp() const',4),('tilt','SPen::ObjectStroke::GetTilt() const',4),('orientation','SPen::ObjectStroke::GetOrientation() const',4)):
  p.call('SPen::ObjectStroke::GetPointCount() const',[_obj]); _count=p.x(0); p.call(_getter,[_obj]); _ptr=p.x(0); _array_state[_field]={{'count':_count,'bytes':bytes(p.u.mem_read(_ptr,_count*_unit)).hex() if _count else ''}}
 p.call('SPen::ObjectStroke::GetColor() const',[_obj]); _native_color=p.x(0)&0xffffffff
 _factory_before=len(factory_created_pens); _event_before=len(event_order); _buffer_before=len(inkpen_buffer_events); _color_before=len(paint_color_calls); phase='serialized_drawing'
 try:
  p.call('SPen::ObjectDrawing::drawObjectStroke(SPen::ISPCanvas&, SPen::ISPPenCanvas&, SPen::ObjectStroke const*, bool)',[p.drawing,p.draw_canvas,p.pen_canvas,_obj,1],limit=5000000); _draw_result=p.x(0); _draw_error=None
 except Exception as _exc:
  _draw_result=None; _draw_error=type(_exc).__name__+': '+str(_exc)
 _serialized_drawing={{'result':_draw_result,'error':_draw_error,'new_factory_count':len(factory_created_pens)-_factory_before,'event_order':event_order[_event_before:],'callback_count':len(inkpen_buffer_events)-_buffer_before,'selected_versions':[]}}
 _serialized_drawing['boundaries']=native_redraw_sample_ranges[:]
 _serialized_drawing['native_getcolor_argb']=f'0x{{_native_color:08x}}'
 _serialized_drawing['color_forwarding_calls']=paint_color_calls[_color_before:]
 _new_events=inkpen_buffer_events[_buffer_before:]
 _callback_delta=b''.join(bytes.fromhex(row['delta_bytes']) for row in _new_events)
 _serialized_drawing['callback_delta_sha256']=hashlib.sha256(_callback_delta).hexdigest()
 _serialized_drawing['callback_delta_bytes']=_callback_delta.hex()
 _serialized_drawing['redraw_entries']=[{{'pc':row['pc'],'signatures':row['signatures'],'phase':row['phase']}} for row in native_draw_entries if row['phase']=='serialized_drawing' and any('RedrawPen' in name for name in row['signatures'])]
 for _newpen in factory_created_pens[_factory_before:]:
  _primary=struct.unpack('<Q',p.u.mem_read(_newpen+112,8))[0]
  _sub=struct.unpack('<Q',p.u.mem_read(_primary+8,8))[0] if _primary else 0
  _target=struct.unpack('<Q',p.u.mem_read(_sub+40,8))[0] if _sub else 0
  _serialized_drawing['selected_versions'].append([name for name,address in p.syms.items() if address==_target])
 _probe_resolved_result['native_source_arrays']=_array_state
 _probe_resolved_result['serialized_drawing']=_serialized_drawing
 raise SystemExit(77)
'''
    src=instrument_event_boundaries(caller.decode())
    if src.count(ANCHOR)!=1: raise RuntimeError('caller insertion anchor changed')
    transformed=src.replace(ANCHOR,inject+'\n'+ANCHOR,1)
    sys.argv=[str(CALLER),'--print-capture','--advanced-setting','null']
    ns={'__name__':'__main__','__file__':str(CALLER),'StopAfterProbe':StopAfterProbe}
    try:
        with contextlib.redirect_stdout(io.StringIO()), contextlib.redirect_stderr(io.StringIO()): exec(compile(transformed,str(CALLER),'exec'),ns)
    except StopAfterProbe: pass
    except SystemExit as e:
        if e.code != 77: raise
    result=ns.get('_probe_resolved_result')
    if not isinstance(result,dict): raise RuntimeError('injected probe did not execute')
    if result['pen_name_utf8']!='com.samsung.android.sdk.pen.pen.preload.InkPen':
        raise RuntimeError(f'native resolution mismatch: {result}')
    _is_null='null_setting_v1' in case_id
    _expected_advanced=None if _is_null else '18;0;100;'
    _expected_version='V1' if _is_null else 'V4'
    if result['advanced_setting_utf8']!=_expected_advanced or result['advanced_pointer_nonnull']==_is_null:
        raise RuntimeError(f'native advanced-setting resolution mismatch: {result}')
    _redraw=' '.join(name for row in result['serialized_drawing']['redraw_entries'] for name in row['signatures'])
    if result['serialized_drawing']['result']!=1 or f'InkPenStrokeDrawableGL{_expected_version}::RedrawPen' not in _redraw:
        raise RuntimeError(f'actual Drawing did not select expected {_expected_version}: {result["serialized_drawing"]}')
    _expected_output=(68,'89ec7813be3240ec2057158ff775dc3432e6326eb68967bd12c881daa07a2029') if _is_null else (75,'ba210c31db12c036facbc682ce85fe69643ed28ee575da0fcc169b443043e934')
    if (result['serialized_drawing']['callback_count'],result['serialized_drawing']['callback_delta_sha256'])!=_expected_output:
        raise RuntimeError(f'native drawing callback stream changed: {result["serialized_drawing"]}')
    result['provenance']={'caller_sha256':sha(caller),'instrumented_caller_sha256':sha(transformed.encode()),'source_archive_sha256':fixture['input']['string_table']['note_original_source_archive_sha256'],'synthetic_case':case_id,'embedded_string_table_sha256':sha(table),'embedded_frame_sha256':sha(frame),'table_mutation':fixture['input']['string_table']['synthetic_mutation'],'frame_mutation':case['frame']['synthetic_mutation'],'host_boundary':'native Model constructors, StringIDManager::ApplyBinary, ModelContext manager setter/getter, ObjectStroke NewApplyBinary, ObjectStroke OnAttach and native string getters execute; context/manager wrappers are host-allocated, and no SDK whole-note loader executes'}
    return result
def file_sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def preflight(fixture, check_runner):
    expected=os.environ.get('INKPEN_RUNNER_PIN') or fixture.get('runner_sha256')
    actual=file_sha(__file__)
    if check_runner and (not expected or actual!=expected):
        raise RuntimeError(f'runner pin mismatch: {actual} != {expected}')
    for name,expected in fixture['source_pins'].items():
        path=Path(name)
        if not path.is_absolute(): path=ROOT/path
        if not path.is_file(): raise RuntimeError(f'pinned runtime input missing: {name}')
        actual=file_sha(path)
        if actual!=expected: raise RuntimeError(f'runtime pin mismatch for {name}: {actual} != {expected}')
    table=base64.b64decode(fixture['input']['string_table']['bytes'],validate=True)
    if len(table)!=fixture['input']['string_table']['length'] or file_sha_bytes(table)!=fixture['input']['string_table']['sha256']:
        raise RuntimeError('embedded StringIDManager table hash/length mismatch')
    for case in fixture['input']['cases'].values():
        frame=base64.b64decode(case['frame']['bytes'],validate=True)
        if len(frame)!=case['frame']['length'] or file_sha_bytes(frame)!=case['frame']['sha256']:
            raise RuntimeError('embedded ObjectStroke frame hash/length mismatch')


def file_sha_bytes(data):
    return hashlib.sha256(data).hexdigest()


def capture_subprocess(case_id):
    env=os.environ.copy()
    env['PYTHONPATH']=os.pathsep.join(str(ROOT/p) for p in (
      'scratch/apk-analysis-runtime/python',
      'scratch/apk-analysis-runtime/binutils-aarch64/usr/bin',
      'conformance'))
    env['LD_LIBRARY_PATH']=str(ROOT/'scratch/apk-analysis-runtime/binutils-aarch64/usr/lib/x86_64-linux-gnu')
    env['INKPEN_RUNNER_PIN']=file_sha(__file__)
    py=ROOT/'conformance/.venv/bin/python'
    done=subprocess.run([str(py),str(Path(__file__).resolve()),'--capture-case',case_id],cwd=ROOT,env=env,text=True,capture_output=True,check=True)
    return json.loads(done.stdout)


def driver():
    parser=argparse.ArgumentParser(description=__doc__)
    mode=parser.add_mutually_exclusive_group()
    mode.add_argument('--write-fixture',action='store_true')
    mode.add_argument('--check-fixture',action='store_true')
    mode.add_argument('--capture-case',choices=('synthetic_full_name_null_setting_v1','synthetic_full_name_saved_setting_v4'))
    args=parser.parse_args()
    os.chdir(ROOT)
    fixture=json.loads(FIXTURE.read_text())
    preflight(fixture,check_runner=not args.write_fixture)
    if args.capture_case:
        print(json.dumps(main(args.capture_case),indent=2,sort_keys=True))
        return 0
    actual={name:{'native_capture':capture_subprocess(name)} for name in fixture['input']['cases']}
    output=dict(fixture)
    output['runner_sha256']=file_sha(__file__)
    output['cases']=actual
    if args.write_fixture:
        FIXTURE.write_text(json.dumps(output,indent=2,sort_keys=True)+'\n')
        print('wrote '+str(FIXTURE.relative_to(ROOT)))
        return 0
    if actual!=fixture['cases']:
        raise SystemExit('serialized InkPen Drawing native fixture mismatch')
    print('verified embedded-frame native Drawing cases: V1/null and V4/saved setting')
    return 0


if __name__=='__main__':
    if '--capture-case' in sys.argv:
        # Capture workers preflight the frozen input and runner before importing caller modules.
        _f=json.loads(FIXTURE.read_text())
        preflight(_f,check_runner=True)
    raise SystemExit(driver())
