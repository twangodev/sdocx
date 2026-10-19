"""Execute the bounded native BrushPen callback/task handoff on one RTV1."""
import argparse
import hashlib
import json
import struct
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / 'conformance/brushpen-handoff-native.json'
GEOMETRY = ROOT / 'conformance/brushpen-saved-native.json'
GEOMETRY_RUNNER = ROOT / 'conformance/brushpen_saved_native.py'
POSITIVE_RUNNER = ROOT / 'conformance/brushpen_paint_positive_native.py'
TEXTURE_RUNNER = ROOT / 'conformance/brushpen_texture_native.py'
sys.path.insert(0, str(ROOT / 'conformance'))
import brushpen_paint_positive_native as positive
import brushpen_saved_native as geometry
from unicorn import UC_HOOK_MEM_WRITE, arm64_const as R

OBJECT_CASE = geometry.OBJECT_CASE


def sha256(raw):
    return hashlib.sha256(raw).hexdigest()


def qwords(raw):
    return [f'0x{x:x}' for x in struct.unpack('<' + 'Q' * (len(raw) // 8), raw)]


class HandoffProbe(positive.PositiveProbe):
    def __init__(self):
        super().__init__()
        self.queue_submissions = []
        self.rt_writes = []
        self.native_call_entries = []
        self.callback_this = 0
        self.watch_rt = 0
        self.paint_state = None
        self.draw_args = None
        self.queue_obj = 0
        self.queue_get_pc, self.queue_submit_pc, self.canvas_bounds_pc = 0x20001040, 0x20001060, 0x20001080
        for pc in (self.queue_get_pc, self.queue_submit_pc, self.canvas_bounds_pc):
            self.u.mem_write(pc, b'\0' * 4)
        setup = self.setup_native_pen

        def setup_once():
            if self.paint_state is None:
                self.paint_state = setup()
                self.watch_rt = self.paint_state['rt']
            return self.paint_state
        self.setup_native_pen = setup_once
        self.u.hook_add(UC_HOOK_MEM_WRITE, self.memwrite)

    def memwrite(self, u, access, address, size, value, data):
        if self.watch_rt and self.watch_rt <= address < self.watch_rt + 128:
            self.rt_writes.append({'offset': address - self.watch_rt, 'size': size,
                                   'value_hex': f'0x{value:x}'})

    def hook(self, u, address, size, data):
        if address == self.queue_get_pc:
            self.x(0, self.queue_obj)
            u.reg_write(R.UC_ARM64_REG_PC, u.reg_read(R.UC_ARM64_REG_LR))
            return
        if address == self.queue_submit_pc:
            task = self.x(1)
            raw = bytes(u.mem_read(task, 96))
            self.queue_submissions.append({'task': task, 'queue': self.x(0),
                'vptr': self.get(task, 'Q')[0], 'raw96': raw,
                'words': struct.unpack('<12Q', raw)})
            self.x(0, 1)
            u.reg_write(R.UC_ARM64_REG_PC, u.reg_read(R.UC_ARM64_REG_LR))
            return
        if address == self.canvas_bounds_pc:
            u.mem_write(u.reg_read(R.UC_ARM64_REG_X8), b'\0' * 32)
            u.reg_write(R.UC_ARM64_REG_PC, u.reg_read(R.UC_ARM64_REG_LR))
            return
        draw = self.syms.get('SPen::BrushPenStrokeDrawableRTV1::Draw(SPen::ISPSubBitmap*, SPen::Matrix4<float>&)')
        if draw and address == draw:
            self.draw_args = [self.x(i) for i in range(3)]
        if address == self.end_pen_plt and self.stop_before_end_pen:
            self.stop_before_end_pen = False
        watched = {
            self.syms.get('SPen::PenReturnCallback::PenReturnCallback(SPen::PenDrawableRT*, SPen::ISPPenCanvas*)'): 'PenReturnCallback::constructor',
            self.syms.get('SPen::PenReturnCallback::GetBuffer()'): 'PenReturnCallback::GetBuffer',
            self.syms.get('SPen::PenDrawableRT::SetBuffer(std::__ndk1::vector<float, std::__ndk1::allocator<float> >*, std::__ndk1::vector<float, std::__ndk1::allocator<float> >*)'): 'PenDrawableRT::SetBuffer',
            self.syms.get('SPen::PenReturnCallback::~PenReturnCallback()'): 'PenReturnCallback::destructor',
            self.syms.get('SPen::PenDrawableRT::SendDataToGPU(std::__ndk1::vector<float, std::__ndk1::allocator<float> >*, std::__ndk1::vector<float, std::__ndk1::allocator<float> >*)'): 'PenDrawableRT::SendDataToGPU',
        }
        if address in watched and address:
            receiver = self.x(0)
            rt = self.watch_rt
            if watched[address] == 'PenReturnCallback::constructor':
                self.callback_this = receiver
                matches = self.x(1) == rt
                role = 'callback_this; RT is argument x1'
            elif watched[address] in ('PenReturnCallback::GetBuffer', 'PenReturnCallback::destructor'):
                matches = receiver == self.callback_this
                role = 'PenReturnCallback this'
            else:
                matches = receiver == rt
                role = 'RT this'
            self.native_call_entries.append({'method': watched[address], 'receiver': hex(receiver),
                'receiver_role': role, 'receiver_matches_expected_object': matches,
                'rt_fields_at_entry_72_to_127_hex': bytes(u.mem_read(rt + 72, 56)).hex() if rt else None,
                'x0_x3': [hex(self.x(i)) for i in range(4)]})
        super().hook(u, address, size, data)

    def run_handoff(self):
        initial = super().run()
        rt = self.paint_state['rt']
        before = bytes(self.u.mem_read(rt + 72, 56))
        self.queue_obj = self.alloc(32)
        queue_vtable = self.alloc(32)
        self.put(self.queue_obj, 'Q', queue_vtable)
        self.put(queue_vtable + 16, 'Q', self.queue_submit_pc)
        canvas = self.get(self.paint_state['drawable'] + 32, 'Q')[0]
        canvas_vtable = self.get(canvas, 'Q')[0]
        self.put(canvas_vtable + 184, 'Q', self.queue_get_pc)
        self.put(canvas_vtable + 216, 'Q', self.canvas_bounds_pc)
        # These paths now execute their native bodies. All other unresolved
        # reached imports continue through the fail-closed inherited loader.
        for name in (
            'SPen::PenDrawableRT::SetBuffer(std::__ndk1::vector<float, std::__ndk1::allocator<float> >*, std::__ndk1::vector<float, std::__ndk1::allocator<float> >*)',
            'SPen::PenReturnCallback::PenReturnCallback(SPen::PenDrawableRT*, SPen::ISPPenCanvas*)',
            'SPen::PenReturnCallback::GetBuffer()',
            'SPen::PenReturnCallback::~PenReturnCallback()',
        ):
            self.callbacks.pop(name, None)
        self.stop_before_end_pen = False
        source = self.run_object_core(OBJECT_CASE)
        after_callback = bytes(self.u.mem_read(rt + 72, 56))
        if len(self.end_pen_records) != 1:
            raise RuntimeError(f'expected one completed native endPen, got {len(self.end_pen_records)}')
        if len(self.queue_submissions) != 3:
            raise RuntimeError(f'expected 3 native callback tasks, got {len(self.queue_submissions)}')
        callback_names = [name for name in self.import_calls
                          if 'PenReturnCallback' in name or 'PenDrawableRT::SetBuffer' in name]
        callback_counts = {name: self.import_calls[name] for name in callback_names}
        if any(callback_counts.get(name, 0) != 1 for name in (
            'SPen::PenDrawableRT::SetBuffer(std::__ndk1::vector<float, std::__ndk1::allocator<float> >*, std::__ndk1::vector<float, std::__ndk1::allocator<float> >*)',
            'SPen::PenReturnCallback::PenReturnCallback(SPen::PenDrawableRT*, SPen::ISPPenCanvas*)',
            'SPen::PenReturnCallback::GetBuffer()',
            'SPen::PenReturnCallback::~PenReturnCallback()',
        )):
            raise RuntimeError(f'native callback import counts unexpected: {callback_counts}')
        callback_rt = sorted(set(row['rt_this'] for row in source['callback_arguments']))
        if callback_rt != [hex(rt)]:
            raise RuntimeError(f'AddPoint callbacks used unexpected RTV1 receiver(s): {callback_rt}')
        if not self.native_call_entries or any(not row['receiver_matches_expected_object']
                                               for row in self.native_call_entries):
            raise RuntimeError(f'native callback/task receiver identity mismatch: {self.native_call_entries}')

        task1 = self.queue_submissions[0]
        task1_vtable_symbol = next((name for name, address in self.syms.items()
                                    if address + 16 == task1['vptr'] and name.startswith('vtable for ')), None)
        expected_task1_vtable = 'vtable for SPen::WorkerTask::Member2<SPen::PenDrawableRT, void (SPen::PenDrawableRT::*)(std::__ndk1::vector<float, std::__ndk1::allocator<float> >*, std::__ndk1::vector<float, std::__ndk1::allocator<float> >*), std::__ndk1::vector<float, std::__ndk1::allocator<float> >*, std::__ndk1::vector<float, std::__ndk1::allocator<float> >*>'
        if task1_vtable_symbol != expected_task1_vtable:
            raise RuntimeError(f'unexpected native task1 vtable: {task1_vtable_symbol}')
        task1_target = task1['words'][3]
        expected_target = self.syms['SPen::PenDrawableRT::SendDataToGPU(std::__ndk1::vector<float, std::__ndk1::allocator<float> >*, std::__ndk1::vector<float, std::__ndk1::allocator<float> >*)']
        if task1_target != expected_target or task1['words'][2] != rt:
            raise RuntimeError('task 1 receiver/member target does not match native SendDataToGPU contract')
        entry1 = self.get(task1['vptr'] + 16, 'Q')[0]
        self.x(0, task1['task'])
        self.u.reg_write(R.UC_ARM64_REG_SP, 0x200f0000)
        self.u.reg_write(R.UC_ARM64_REG_LR, 0x200f1000)
        self.u.emu_start(entry1, 0x200f1000, count=2_000_000)
        if self.u.reg_read(R.UC_ARM64_REG_PC) != 0x200f1000:
            raise RuntimeError('native task 1 trampoline did not return')
        after_task1 = bytes(self.u.mem_read(rt + 72, 56))
        task1_fields = struct.unpack('<7Q', after_task1)
        if (task1['words'][5] != task1_fields[2] or task1['words'][6] != task1_fields[5]
                or task1_fields[1] != 0 or task1_fields[4] != 0):
            raise RuntimeError('task1 vector arguments do not match the post-task RT fields')

        task2 = self.queue_submissions[1]
        task2_vtable_symbol = next((name for name, address in self.syms.items()
                                    if address + 16 == task2['vptr'] and name.startswith('vtable for ')), None)
        if task2_vtable_symbol != 'vtable for SPen::PenGLRenderMsg':
            raise RuntimeError(f'unexpected native task2 vtable: {task2_vtable_symbol}')
        entry2 = self.get(task2['vptr'] + 16, 'Q')[0]
        canvas_vtable = self.get(canvas, 'Q')[0]
        missing_slot_address = canvas_vtable + 192
        missing_slot_target = self.get(missing_slot_address, 'Q')[0]
        if missing_slot_target != 0 or task2['words'][5] != canvas:
            raise RuntimeError('task2 missing canvas vtable+192 boundary changed')
        self.x(0, task2['task'])
        self.u.reg_write(R.UC_ARM64_REG_SP, 0x200f0000)
        self.u.reg_write(R.UC_ARM64_REG_LR, 0x200f1000)
        try:
            self.u.emu_start(entry2, 0x200f1000, count=2_000_000)
            task2_result = {'returned': self.u.reg_read(R.UC_ARM64_REG_PC) == 0x200f1000}
        except Exception as exc:
            task2_result = {'returned': False, 'error': str(exc),
                'pc': hex(self.u.reg_read(R.UC_ARM64_REG_PC)),
                'lr': hex(self.u.reg_read(R.UC_ARM64_REG_LR)),
                'x0_x3': [hex(self.x(i)) for i in range(4)]}
        if (task2_result.get('pc') != '0x0' or task2_result.get('lr') != '0x10467bc'
                or task2_result.get('x0_x3', [None])[0] != hex(canvas)):
            raise RuntimeError(f'task 2 expected the explicit missing canvas +192 adapter boundary: {task2_result}')
        after_task2 = bytes(self.u.mem_read(rt + 72, 56))
        draw_vector = self.get(rt + 88, 'Q')[0]
        begin, end, capacity = self.get(draw_vector, '3Q')
        raw = bytes(self.u.mem_read(begin, end - begin))
        if len(raw) != 26 * 44 or end > capacity:
            raise RuntimeError(f'invalid native transferred vector bounds/size: {len(raw)}')
        saved = json.loads(GEOMETRY.read_text())
        expected_case = next(c for c in saved['synthetic_cases'] if c['label'] == OBJECT_CASE['name'])
        expected_bytes = b''.join(bytes.fromhex(word) for row in expected_case['buffer_records']
                                  for word in row['instance_bits_le'])
        if raw != expected_bytes:
            raise RuntimeError('native callback/task1 vector differs from pinned saved geometry fixture')

        # Complete the already captured native Update/Draw path explicitly on
        # the host Mesa FBO; task 2 and task 3 remain outside this boundary.
        self._call_gl('glBindFramebuffer', self.mesa.f['glBindFramebuffer'], 0x8D40,
                      initial['output']['framebuffer_id'])
        self._call_gl('glClearColor', self.mesa.f['glClearColor'], 0.0, 0.0, 0.0, 0.0)
        self._call_gl('glClear', self.mesa.f['glClear'], 0x4000)
        self.call('SPen::BrushPenStrokeDrawableRTV1::Update(bool)', [rt, 0])
        if not self.draw_args or self.draw_args[0] != rt:
            raise RuntimeError('missing native Draw receiver for initialized RTV1')
        self.call('SPen::BrushPenStrokeDrawableRTV1::Draw(SPen::ISPSubBitmap*, SPen::Matrix4<float>&)', self.draw_args)
        pixels = positive.C.create_string_buffer(321 * 228 * 4)
        self._call_gl('glPixelStorei', self.mesa.f['glPixelStorei'], 0x0D05, 1)
        self._call_gl('glReadPixels', self.mesa.f['glReadPixels'], 0, 0, 321, 228,
                      0x1908, 0x1401, positive.C.cast(pixels, positive.C.c_void_p))
        if self.gl_errors:
            raise RuntimeError(f'host GLES errors: {self.gl_errors[:8]}')
        after_draw = bytes(self.u.mem_read(rt + 72, 56))

        tasks = []
        for index, row in enumerate(self.queue_submissions):
            words = row['words']
            tasks.append({'index': index + 1, 'object_address': hex(row['task']),
                'queue_address': hex(row['queue']), 'vptr': hex(row['vptr']),
                'raw_96_bytes_hex': row['raw96'].hex(), 'qwords_le_hex': qwords(row['raw96']),
                'worker_trampoline': hex(self.get(row['vptr'] + 16, 'Q')[0]),
                'receiver_word2': hex(words[2]), 'member_target_word3': hex(words[3]),
                'pointer_argument_word5': hex(words[5])})
        task3 = self.queue_submissions[2]
        task3_vtable_symbol = next((name for name, address in self.syms.items()
                                    if address + 16 == task3['vptr'] and name.startswith('vtable for ')), None)
        task3_entry = self.get(task3['vptr'] + 16, 'Q')[0]
        if task3_vtable_symbol != 'vtable for SPen::WorkerTask::Delete<std::__ndk1::vector<float, std::__ndk1::allocator<float> > >':
            raise RuntimeError(f'unexpected deferred task3 vtable symbol: {task3_vtable_symbol}')
        fields = lambda raw: {'rt_plus_72_through_127_raw_hex': raw.hex(),
                              'qwords_at_72_80_88_96_104_112_120': qwords(raw[:56]),
                              'count_u32_at_72': struct.unpack_from('<I', raw, 0)[0]}
        return {
            'schema': 'brushpen-native-callback-handoff-v1',
            'scope': 'Synthetic ObjectStroke on one fresh native RTV1. Native callback constructor/GetBuffer/SetBuffer/destructor, endPen, and task 1 execute. A host canvas/queue adapter captures submissions. Task 2 stops at the absent synthetic canvas vtable+192 method; task 3 is deferred. Update/Draw are then explicitly invoked on host Mesa. This is not natural application queue completion, Android/device execution, or captured-device input.',
            'input': {'kind': 'explicit synthetic ObjectStroke', 'case': OBJECT_CASE['name'],
                      'motion': expected_case['synthetic_input'], 'constructor_selected_version': source['admission']['actual_getVersion']},
            'native_callback': {'native_import_counts': callback_counts,
                'addpoint_callback_count': len(source['callback_arguments']),
                'addpoint_rt_this_values': callback_rt,
                'arguments': source['callback_arguments'],
                'endPen_completed_count': len(self.end_pen_records),
                'terminal_addpoint_count': self.end_pen_records[0]['emitted_addpoint_count']},
            'rt_field_snapshots': {'before_ObjectStroke': fields(before),
                'after_callback_destructor_before_task1': fields(after_callback),
                'after_task1_before_task2_or_Update': fields(after_task1),
                'after_failed_task2': fields(after_task2), 'after_Update_and_Draw': fields(after_draw),
                'observed_native_writes': self.rt_writes,
                'native_method_entry_snapshots': self.native_call_entries},
            'tasks': {'task_count': len(tasks), 'entries': tasks,
                'task1_execution': {'returned': True, 'target_symbol': 'SPen::PenDrawableRT::SendDataToGPU',
                    'target_elf_offset_PenCommon': '0x4a5c4', 'vtable_symbol': task1_vtable_symbol,
                    'receiver_matches_rt': True,
                    'vector_argument': hex(task1['words'][5]), 'second_vector_argument': hex(task1['words'][6])},
                'task2_missing_canvas_plus_192_boundary': {**task2_result,
                    'vtable_symbol': task2_vtable_symbol,
                    'receiver_canvas': hex(canvas), 'canvas_vtable': hex(canvas_vtable),
                    'missing_slot_offset': 192, 'missing_slot_address': hex(missing_slot_address),
                    'missing_slot_target': hex(missing_slot_target),
                    'fault_lr_matches_callsite_after_indirect_slot': task2_result.get('lr') == '0x10467bc'},
                'task3_execution': {'status': 'deferred', 'vtable_symbol': task3_vtable_symbol,
                    'vtable_addresspoint': hex(task3['vptr']),
                    'vtable_symbol_address': hex(task3['vptr'] - 16),
                    'vtable_addresspoint_offset_from_symbol': 16,
                    'entry_address': hex(task3_entry),
                    'entry_elf_offset_PenCommon': hex(task3_entry - 0x01000000),
                    'entry_code_first_16_bytes': bytes(self.u.mem_read(task3_entry, 16)).hex(),
                    'pointer_argument': hex(task3['words'][2]),
                    'meaning': 'The native vtable symbol is WorkerTask::Delete<std::__ndk1::vector<float>>; its slot-0 entry is recorded above. The entry body reads the vector object at task+16, destroys its storage, then frees the object. Deferred and not executed.'},
            },
            'buffer_handoff': {'rt_plus_80_after_task1': hex(task1_fields[1]),
                'rt_plus_88_after_task1': hex(task1_fields[2]),
                'rt_plus_104_after_task1': hex(task1_fields[4]),
                'rt_plus_112_after_task1': hex(task1_fields[5]),
                'task1_vector_argument_matches_rt_plus_88': task1['words'][5] == task1_fields[2],
                'task1_second_vector_argument_matches_rt_plus_112': task1['words'][6] == task1_fields[5],
                'producer_fields_cleared_after_task1': task1_fields[1] == 0 and task1_fields[4] == 0,
                'vector_bytes': len(raw), 'vector_sha256': sha256(raw),
                'same_bytes_as_saved_geometry_fixture': True,
                'native_update_draw_instanced_args': self.graphics_draw_calls[-1]},
            'host_mesa_paint': {'joined_fullstroke_readback_rgba8_321x228_sha256': sha256(pixels.raw),
                'gl_errors': self.gl_errors, 'draw_count_including_initial_paint': len(self.graphics_draw_calls),
                'mesa': initial['mesa'],
                'initial_control_framebuffer_metadata': initial['output']},
            'provenance': {'geometry_fixture_sha256': sha256(GEOMETRY.read_bytes()),
                'geometry_runner_sha256': sha256(GEOMETRY_RUNNER.read_bytes()),
                'positive_runner_sha256': sha256(POSITIVE_RUNNER.read_bytes()),
                'texture_runner_sha256': sha256(TEXTURE_RUNNER.read_bytes()),
                'native_dsos': initial['provenance']['native_elf_sha256'],
                'host_gl_dsos': initial['provenance']['host_gles_dsos'],
                'input_motion_sha256': sha256(json.dumps(expected_case['synthetic_input'], sort_keys=True).encode())},
        }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    modes = parser.add_mutually_exclusive_group(required=True)
    modes.add_argument('--write-fixture', action='store_true')
    modes.add_argument('--check-fixture', action='store_true')
    args = parser.parse_args()
    report = HandoffProbe().run_handoff()
    encoded = json.dumps(report, indent=2, sort_keys=True) + '\n'
    if args.write_fixture:
        OUT.write_text(encoded)
        print(f'wrote {OUT}')
        return
    if not OUT.exists():
        raise SystemExit(f'missing reviewed fixture {OUT}; inspect then run --write-fixture')
    expected = json.loads(OUT.read_text())
    if json.dumps(expected, sort_keys=True) != json.dumps(report, sort_keys=True):
        raise AssertionError('native BrushPen handoff output differs from reviewed fixture')
    print('BrushPen native callback handoff oracle passed')


if __name__ == '__main__':
    main()
