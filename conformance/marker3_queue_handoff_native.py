"""Reproduce Marker3's native queue handoff from the saved Drawing caller.

The runner instruments the pinned caller in memory. Its only positive/negative
difference is whether the host canvas stub returns the selected native task
queue. The actual SendDataToGPU call, WorkerTask Run, and vector publication
execute in Unicorn. PenGLRenderMsg stops at the host canvas boundary; this is
not a GPU pixel test.
"""
import argparse
import contextlib
import hashlib
import importlib.util
import io
import json
import os
import struct
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
CALLER = ROOT / 'conformance/marker3_saved_caller_native.py'
CALLER_FIXTURE = ROOT / 'conformance/marker3-saved-caller-native.json'
FIXTURE = ROOT / 'conformance/marker3-queue-handoff-native.json'
SAVED_CASE = 'marker3_linked_saved1:5:tool2:curve1'
SEND_NAME = 'SPen::PenDrawableRT::SendDataToGPU(std::__ndk1::vector<float, std::__ndk1::allocator<float> >*, std::__ndk1::vector<float, std::__ndk1::allocator<float> >*)'
SEND_TASK_VTABLE = '0x10774f8'
SEND_TASK_RUN = '0x10506b4'
RENDER_TASK_VTABLE = '0x10774c8'
VECTOR_DELETE_TASK_VTABLE = '0x1077528'
VECTOR_DELETE_TASK_RUN = '0x1045e6c'
VECTOR_DELETE_TASK_TYPE = 'SPen::WorkerTask::Delete<std::__ndk1::vector<float, std::__ndk1::allocator<float> > >'

# These are the source helpers imported by the original caller. The in-memory
# instrumentation below is owned by this file, so no ignored trace scripts are
# required to reproduce either run.
SOURCE_PINS = {
    'marker3_saved_caller_native.py': '6237076d662e0390711b2a69b43bde4e07369297b9897a675c693550f297550d',
    'marker3-saved-caller-native.json': '6551bc4a2403079c54ddae2a8123b63189030596f9af79f5b7c5af1c2349d090',
    'oblique_canvas_native.py': '1cabf3f8bf907ce6fd48952682b4cad5208827cec4aa0bd0a1ec864aa750a1f2',
    'fountain_native.py': '40cb917c7f286302d8519e9b953c3c21b7fe6ea0b6b0a80ccc13e2e724cfad30',
    'elf_plt.py': '816eceff5b532b35c8ef9ea904a8aa64767422b0799347a18fbd4602b70f373b',
}


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def validate_source_pins():
    for name, digest in SOURCE_PINS.items():
        path = (ROOT / ('conformance/' + name))
        if not path.is_file() or sha(path) != digest:
            raise RuntimeError(f'pinned caller/helper changed: {path.relative_to(ROOT)}')


def vector_record(u, pointer):
    begin, end, capacity = struct.unpack('<QQQ', u.mem_read(pointer, 24))
    if not begin or end < begin or capacity < end:
        raise RuntimeError(f'invalid native vector header at {pointer:#x}')
    raw = bytes(u.mem_read(begin, end - begin))
    return {'object': hex(pointer), 'begin': hex(begin), 'end': hex(end),
            'capacity': hex(capacity), 'float_count': len(raw) // 4,
            'byte_count': len(raw), 'sha256': hashlib.sha256(raw).hexdigest(),
            'bytes_hex': raw.hex()}


def instrumented_source(queue_enabled):
    src = CALLER.read_text()
    class_anchor = 'p=Marker3CanvasProbe()'
    class_patch = r'''_old_canvas_hook=Marker3CanvasProbe.hook_canvas
def _handoff_canvas_hook(self,uc,address,size,data):
 if address==0x200f2040 and getattr(self,'native_task_queue',0):
  self.queue_bridge_returns.append({'stub':hex(address),'queue':hex(self.native_task_queue),'memory_writes':[]})
  self.x(0,self.native_task_queue);uc.reg_write(R.UC_ARM64_REG_PC,uc.reg_read(R.UC_ARM64_REG_LR));return
 return _old_canvas_hook(self,uc,address,size,data)
Marker3CanvasProbe.hook_canvas=_handoff_canvas_hook
'''
    if src.count(class_anchor) != 1:
        raise RuntimeError('caller class injection anchor changed')
    instance_patch = r'''p=Marker3CanvasProbe()
def _trace_noop_delete():
 ptr=p.x(0)
 if getattr(p,'_active_task_vt',0)==0x1077528:
  handoff_events.append({'kind':'host_noop_allocator_delete','phase':phase,
                         'task':hex(p._active_task),'vector_allocation':hex(ptr),
                         'behavior':'host operator delete callback intentionally does not release Unicorn allocation'})
_original_delete=p.callbacks['operator delete(void*)']
p.callbacks['operator delete(void*)']=_trace_noop_delete
'''
    src = src.replace(class_anchor, class_patch + instance_patch)

    queue_anchor = "p._task_returns=[]; p.callbacks['SPen::PenGLDataManager::GetMsgQueue()']"
    queue_replacement = "p._task_returns=[]; p.queue_bridge_returns=[]; "
    if queue_enabled:
        queue_replacement += 'p.native_task_queue=queue; '
    queue_replacement += "p.callbacks['SPen::PenGLDataManager::GetMsgQueue()']"
    if src.count(queue_anchor) != 1:
        raise RuntimeError('caller queue wiring anchor changed')
    src = src.replace(queue_anchor, queue_replacement)

    hook_anchor = 'p.u.hook_add(UC_HOOK_CODE,log_vcall)'
    hook_source = r'''p.u.hook_add(UC_HOOK_CODE,log_vcall)
handoff_methods=(
 'SPen::Marker3StrokeDrawableRTV1::AddPoint(SPen::Vector2<float>)',
 'SPen::Marker3StrokeDrawableRTV1::Update(bool)',
 'SPen::PenDrawableRT::SetBuffer(std::__ndk1::vector<float, std::__ndk1::allocator<float> >*, std::__ndk1::vector<float, std::__ndk1::allocator<float> >*)',
 'SPen::PenDrawableRT::SendDataToGPU(std::__ndk1::vector<float, std::__ndk1::allocator<float> >*, std::__ndk1::vector<float, std::__ndk1::allocator<float> >*)',
 'SPen::PenReturnCallback::PenReturnCallback(SPen::PenDrawableRT*, SPen::ISPPenCanvas*)',
 'SPen::PenReturnCallback::~PenReturnCallback()',
 'SPen::PenReturnCallback::GetBuffer()',
 'SPen::PenStrokeDataManager::GetStrokeData(int)',
 'SPen::PenStrokeDataManager::ReleaseStrokeData(int)',
)
handoff_addr={p.syms[n]:n for n in handoff_methods if n in p.syms}
handoff_events=[]; handoff_pending={}
def _handoff_trace(u,a,z,d):
 if a in handoff_pending:
  names=handoff_pending.pop(a)
  ret={'kind':'return','methods':names,'pc':hex(a),'x0':hex(u.reg_read(R.UC_ARM64_REG_X0))}
  handoff_events.append(ret)
 if a in handoff_addr:
  name=handoff_addr[a]; x0=u.reg_read(R.UC_ARM64_REG_X0)
  event={'kind':'entry','method':name,'phase':phase,'pc':hex(a),
         'lr':hex(u.reg_read(R.UC_ARM64_REG_LR)),
         'args':[hex(u.reg_read(getattr(R,f'UC_ARM64_REG_X{i}'))) for i in range(4)]}
  if name==''' + repr(SEND_NAME) + r''':
   vector=u.reg_read(R.UC_ARM64_REG_X1)
   event['rt_slots_80_120_before_send']=u.mem_read(x0+80,48).hex()
   event['input_vector']=vector_record(u,vector)
  if name.startswith('SPen::PenDrawableRT::SetBuffer'):
   event['rt_slots_80_120_before_setbuffer']=u.mem_read(x0+80,48).hex()
  handoff_events.append(event)
  handoff_pending.setdefault(u.reg_read(R.UC_ARM64_REG_LR),[]).append(name)
p.u.hook_add(UC_HOOK_CODE,_handoff_trace)
'''
    if src.count(hook_anchor) != 1:
        raise RuntimeError('caller hook installation anchor changed')
    src = src.replace(hook_anchor, hook_source)

    task_anchor = "  if not method: raise RuntimeError('task has null Run')"
    task_patch = r'''  handoff_events.append({'kind':'queue_task','phase':phase,'task':hex(task),
                         'vtable':hex(vt),'run':hex(method),
                         'task_head_96_bytes':u.mem_read(task,96).hex()})
  if vt==0x1077528:
   vector=struct.unpack('<Q',u.mem_read(task+16,8))[0]
   handoff_events.append({'kind':'native_vector_delete_dispatch','phase':phase,
                          'task':hex(task),'vtable':hex(vt),'run':hex(method),
                          'vector_object':hex(vector),
                          'vector_before_delete':vector_record(u,vector)})
  if vt==0x10774c8:
   handoff_events.append({'kind':'host_canvas_boundary','phase':phase,
                          'task':hex(task),'vtable':hex(vt),'run':hex(method),
                          'task_head_96_bytes':u.mem_read(task,96).hex(),
                          'reason':'native PenGLRenderMsg Run stopped before host canvas/graphics execution'})
   p.x(0,1); u.reg_write(R.UC_ARM64_REG_PC,u.reg_read(R.UC_ARM64_REG_LR)); return
'''
    if src.count(task_anchor) != 1:
        raise RuntimeError('caller task dispatcher anchor changed')
    src = src.replace(task_anchor, task_patch + task_anchor)

    submit_anchor = 'p._task_returns.append(caller); p.x(0,task); u.reg_write(R.UC_ARM64_REG_LR,0x20001020); u.reg_write(R.UC_ARM64_REG_PC,method)'
    submit_patch = 'p._task_returns.append(caller); p._active_task=task; p._active_task_vt=vt; p.x(0,task); u.reg_write(R.UC_ARM64_REG_LR,0x20001020); u.reg_write(R.UC_ARM64_REG_PC,method)'
    if src.count(submit_anchor) != 1:
        raise RuntimeError('caller task submission anchor changed')
    src = src.replace(submit_anchor, submit_patch)

    return_anchor = 'caller=p._task_returns.pop(); p.x(0,1); u.reg_write(R.UC_ARM64_REG_PC,caller)'
    return_patch = r'''caller=p._task_returns.pop()
  if getattr(p,'_active_task_vt',0)==0x10774f8:
   task=p._active_task; rt=struct.unpack('<Q',u.mem_read(task+16,8))[0]
   vp=struct.unpack('<Q',u.mem_read(rt+88,8))[0]
   handoff_events.append({'kind':'send_task_complete','phase':phase,
                          'task':hex(task),'rt':hex(rt),
                          'rt_slots_80_120_after_task':u.mem_read(rt+80,48).hex(),
                          'published_vector':vector_record(u,vp) if vp else None})
  elif getattr(p,'_active_task_vt',0)==0x1077528:
   handoff_events.append({'kind':'native_vector_delete_complete','phase':phase,
                          'task':hex(p._active_task),'vtable':'0x1077528',
                          'run':'0x1045e6c'})
  p.x(0,1); u.reg_write(R.UC_ARM64_REG_PC,caller)'''
    if src.count(return_anchor) != 1:
        raise RuntimeError('caller task return anchor changed')
    src = src.replace(return_anchor, return_patch)
    return src


def capture_child(queue_enabled):
    validate_source_pins()
    src = instrumented_source(queue_enabled)
    argv = [str(CALLER), '--check-fixture', '--case', 'marker3_linked_saved1', '--advanced-setting', '5']
    old_argv = sys.argv
    sys.argv = argv
    spec = importlib.util.spec_from_file_location('marker3_handoff_caller', CALLER)
    module = importlib.util.module_from_spec(spec)
    module.__file__ = str(CALLER)
    module.vector_record = vector_record
    try:
        with contextlib.redirect_stdout(io.StringIO()):
            exec(compile(src, str(CALLER), 'exec'), module.__dict__)
    finally:
        sys.argv = old_argv
    if any(SEND_NAME in names for names in module.handoff_pending.values()):
        raise RuntimeError('native SendDataToGPU return hook remained pending')
    rt = struct.unpack('<Q', module.p.u.mem_read(module.actual_drawable + 16, 8))[0]
    return {'queue_enabled': queue_enabled, 'events': module.handoff_events,
            'queue_bridge_returns': module.p.queue_bridge_returns,
            'selected_rt': hex(rt),
            'native_saved_caller_vectors': module.captured_final_vectors}


def vector_from_caller(case):
    phases = case['phase_final_vectors']
    draw = phases['draw_object_stroke']
    saved = phases['saved_redraw']
    raw = bytes.fromhex(draw['vector_bytes'])
    if draw != saved or len(raw) != 176 or hashlib.sha256(raw).hexdigest() != draw['sha256']:
        raise AssertionError('canonical caller vectors are not matching 176-byte output')
    return draw, raw


def check_dso_pins(case):
    paths = {}
    libdir = ROOT / 'scratch/apk-analysis-native/arm64-v8a'
    for name, digest in case['libraries'].items():
        filename = name if name.endswith('.so') else 'libSPen' + name + '.so'
        path = libdir / filename
        if not path.is_file() or sha(path) != digest:
            raise RuntimeError(f'pinned DSO mismatch: {path}')
        paths[filename] = {'path': str(path.relative_to(ROOT)), 'sha256': digest}
    return paths


def phase_handoffs(trace, canonical, raw, positive):
    events = trace['events']
    phases = ('draw_object_stroke', 'saved_redraw')
    handoffs = []
    boundaries = []
    send_entries = [e for e in events if e.get('kind') == 'entry' and e.get('method') == SEND_NAME]
    send_tasks = [e for e in events if e.get('kind') == 'queue_task' and e.get('vtable') == SEND_TASK_VTABLE]
    completions = [e for e in events if e.get('kind') == 'send_task_complete']
    if positive:
        if len(send_entries) != 2 or len(send_tasks) != 2 or len(completions) != 2:
            raise AssertionError('positive trace requires two sends, worker tasks, and completions')
        for phase in phases:
            sends = [e for e in send_entries if e['phase'] == phase]
            tasks = [e for e in send_tasks if e['phase'] == phase]
            done = [e for e in completions if e['phase'] == phase]
            if any(len(v) != 1 for v in (sends, tasks, done)):
                raise AssertionError(f'{phase}: missing or repeated native handoff event')
            sent, task, completed = sends[0], tasks[0], done[0]
            input_vector = sent['input_vector']
            published = completed['published_vector']
            if task['task'] != completed['task'] or task['run'] != SEND_TASK_RUN:
                raise AssertionError(f'{phase}: WorkerTask identity/Run mismatch')
            if completed['rt'] != sent['args'][0] or published['object'] != input_vector['object']:
                raise AssertionError(f'{phase}: native RT +88 did not publish the sent vector object')
            if input_vector['bytes_hex'] != raw.hex() or published['bytes_hex'] != raw.hex():
                raise AssertionError(f'{phase}: directly captured native vector bytes differ from canonical callback bytes')
            if input_vector['sha256'] != canonical['sha256'] or published['sha256'] != canonical['sha256']:
                raise AssertionError(f'{phase}: vector hash differs from canonical saved caller')
            handoffs.append({'phase': phase,
                             'worker_task': {'address': task['task'], 'vtable': task['vtable'],
                                             'run': task['run'], 'head_96_bytes': task['task_head_96_bytes']},
                             'rt_object': sent['args'][0],
                             'send_method_entry': {'pc': sent['pc'], 'lr': sent['lr'],
                                                   'rt_slots_before_send_hex': sent['rt_slots_80_120_before_send']},
                             'vector': {'object': input_vector['object'], 'begin': input_vector['begin'],
                                        'end': input_vector['end'], 'capacity': input_vector['capacity'],
                                        'byte_count': input_vector['byte_count'],
                                        'float_count': input_vector['float_count'],
                                        'sha256': input_vector['sha256'],
                                        'bytes_hex': input_vector['bytes_hex']},
                             'rt_slots_after_task_hex': completed['rt_slots_80_120_after_task'],
                             'published_vector_object': published['object']})
            stops = [e for e in events if e.get('kind') == 'host_canvas_boundary' and
                     e.get('phase') == phase and e.get('vtable') == RENDER_TASK_VTABLE]
            if len(stops) != 1:
                raise AssertionError(f'{phase}: PenGLRenderMsg did not stop at host boundary')
            boundaries.append({'phase': phase, 'task': stops[0]['task'],
                               'vtable': stops[0]['vtable'], 'run': stops[0]['run'],
                               'reason': stops[0]['reason'],
                               'task_head_96_bytes': stops[0]['task_head_96_bytes']})
        if not trace['queue_bridge_returns']:
            raise AssertionError('positive route did not return the selected queue through the host canvas stub')
        if any(row['memory_writes'] for row in trace['queue_bridge_returns']):
            raise AssertionError('host queue bridge wrote memory')
    else:
        if send_entries or send_tasks or completions:
            raise AssertionError('negative queue control unexpectedly reached native SendDataToGPU')
        if any(e.get('kind') == 'host_canvas_boundary' for e in events):
            raise AssertionError('negative queue control dispatched PenGLRenderMsg')
        if trace['queue_bridge_returns']:
            raise AssertionError('negative queue control unexpectedly returned the selected queue')
    deletes = [e for e in events if e.get('kind') == 'native_vector_delete_dispatch']
    frees = [e for e in events if e.get('kind') == 'host_noop_allocator_delete']
    delete_returns = [e for e in events if e.get('kind') == 'native_vector_delete_complete']
    delete_details = []
    if positive:
        if len(deletes) != 2 or len(frees) != 4 or len(delete_returns) != 2:
            raise AssertionError('positive route must execute two native WorkerTask::Delete<vector> tasks')
        for phase in phases:
            sent = next(e for e in send_entries if e['phase'] == phase)
            completed = next(e for e in completions if e['phase'] == phase)
            boundary = next(e for e in events if e.get('kind') == 'host_canvas_boundary' and e.get('phase') == phase)
            delete = next(e for e in deletes if e['phase'] == phase)
            phase_frees = [e for e in frees if e['phase'] == phase]
            backing_free = next(e for e in phase_frees if e['vector_allocation'] == sent['input_vector']['begin'])
            object_free = next(e for e in phase_frees if e['vector_allocation'] == sent['input_vector']['object'])
            returned = next(e for e in delete_returns if e['phase'] == phase)
            ordered = [events.index(e) for e in (completed, boundary, delete, backing_free, object_free, returned)]
            if ordered != sorted(ordered):
                raise AssertionError(f'{phase}: vector delete did not follow send completion and host renderer boundary')
            if delete['vector_object'] != sent['input_vector']['object']:
                raise AssertionError(f'{phase}: Delete<vector> task targets a different vector object')
            if returned['task'] != delete['task']:
                raise AssertionError(f'{phase}: Delete<vector> completion task identity changed')
            delete_details.append({'phase': phase, 'task': delete['task'], 'vtable': delete['vtable'],
                                   'task_type': VECTOR_DELETE_TASK_TYPE,
                                   'vtable_symbol_offset': '0x77518 + address-point 0x10 = 0x77528',
                                   'run': delete['run'], 'vector_object': delete['vector_object'],
                                   'vector_before_delete': delete['vector_before_delete'],
                                   'allocator_delete_callbacks': [
                                       {'allocation': f['vector_allocation'], 'behavior': f['behavior']}
                                       for f in phase_frees],
                                   'event_order': {'send_task_complete': events.index(completed),
                                                   'PenGLRenderMsg_host_boundary': events.index(boundary),
                                                   'Delete_vector_dispatch': events.index(delete),
                                                   'operator_delete_backing': events.index(backing_free),
                                                   'operator_delete_vector_object': events.index(object_free),
                                                   'Delete_vector_return': events.index(returned)}})
    elif deletes or frees or delete_returns:
        raise AssertionError('negative control unexpectedly ran native Delete<vector>')
    return {'send_data_call_count': len(send_entries),
            'send_worker_task_count': len(send_tasks),
            'task_handoffs': handoffs,
            'renderer_host_boundaries': boundaries,
            'vector_delete_after_renderer': delete_details,
            'host_rt_slot_writes': [],
            'host_queue_stub_returns': trace['queue_bridge_returns'],
            'lifetime_limit': ('vector bytes are captured from native SendDataToGPU and RT +88 immediately after the WorkerTask. The following PenGLRenderMsg is stopped at the host canvas boundary; native Delete<vector> then runs. Host operator delete is a no-op, so physical allocator release is not modeled and post-delete vector validity is not claimed.'
                              if positive else 'The wrong-queue control does not deliver SendDataToGPU, a PenGLRenderMsg, or a Delete<vector> task; it produces no vector lifecycle evidence.'),
            'host_slot_boundary': ('host queue wiring returns the queue but does not write RT slots; native SendDataToGPU publishes RT +88'
                                  if positive else 'wrong-queue host wiring does not return the selected queue; no native RT publication occurs')}


def build():
    validate_source_pins()
    caller_doc = json.loads(CALLER_FIXTURE.read_text())
    case = caller_doc['cases'][SAVED_CASE]
    canonical, raw = vector_from_caller(case)
    dsos = check_dso_pins(case)
    positive = capture_via_subprocess(True)
    negative = capture_via_subprocess(False)
    return {'schema': 'sdocx-marker3-native-queue-handoff-v2',
            'provenance': {'runner_sha256': sha(Path(__file__).resolve()),
                           'source_pins_sha256': SOURCE_PINS,
                           'dsos': dsos,
                           'vector_delete_task_type': VECTOR_DELETE_TASK_TYPE,
                           'case_id': SAVED_CASE,
                           'canonical_vector_sha256': canonical['sha256'],
                           'canonical_vector_byte_count': len(raw),
                           'canonical_vector_float_count': len(raw) // 4},
            'scope': 'The pinned saved caller executes in memory with native Model/Drawing/Marker3 code. Positive and negative differ only in the host canvas stub queue return. Direct SendDataToGPU input and RT +88 publication are copied from Unicorn memory before native Delete<vector>; PenGLRenderMsg execution stops at the host canvas boundary, with no pixel claim.',
            'positive': phase_handoffs(positive, canonical, raw, True),
            'wrong_queue_negative_control': phase_handoffs(negative, canonical, raw, False)}


def capture_via_subprocess(queue_enabled):
    mode = 'positive' if queue_enabled else 'negative'
    env = dict(os.environ)
    parts = [str(ROOT / 'scratch/apk-analysis-runtime/python'), str(ROOT / 'conformance')]
    if env.get('PYTHONPATH'):
        parts.append(env['PYTHONPATH'])
    env['PYTHONPATH'] = os.pathsep.join(parts)
    result = subprocess.run([sys.executable, str(Path(__file__).resolve()), '--capture', mode],
                            cwd=ROOT, env=env, check=True, capture_output=True, text=True)
    try:
        return json.loads(result.stdout)
    except json.JSONDecodeError as exc:
        raise RuntimeError(f'{mode} capture did not emit one JSON object: {exc}') from exc


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    group = parser.add_mutually_exclusive_group(required=True)
    group.add_argument('--write-fixture', action='store_true')
    group.add_argument('--check-fixture', action='store_true')
    group.add_argument('--capture', choices=('positive', 'negative'))
    parser.add_argument('--fixture', type=Path, default=FIXTURE)
    args = parser.parse_args()
    if args.capture:
        print(json.dumps(capture_child(args.capture == 'positive'), sort_keys=True))
        return
    actual = build()
    if args.write_fixture:
        args.fixture.write_text(json.dumps(actual, indent=2) + '\n')
    else:
        if json.loads(args.fixture.read_text()) != actual:
            raise SystemExit('Marker3 queue handoff fixture mismatch')
    print('Marker3 queue handoff positive/negative traces matched' if args.check_fixture
          else f'Wrote {args.fixture.relative_to(ROOT)}')


if __name__ == '__main__':
    main()
