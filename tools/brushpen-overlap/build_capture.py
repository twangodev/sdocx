import os
from pathlib import Path
root=Path(__file__).resolve().parents[2]
s=(root/'tools/brushpen-transform/second_mapdraw.py').read_text().replace("Path(__file__).with_name('long_probe.py')",f"Path({str(root/'tools/brushpen-transform/long_probe.py')!r})")
s=s.replace('.read_text().split("for n in [\'SPen::ObjectDrawing")[0])', '.read_text().split("for n in [\'SPen::ObjectDrawing")[0].replace(\'ROOT=Path(__file__).resolve().parents[2]\', '+repr(f'ROOT=Path({str(root)!r})')+'))')
code='''    print('FIRST_FRAME',sha256(task2_pixels.raw),sum(a!=0 for a in task2_pixels.raw[3::4]),'framebuffer',native_canvas_fbo,'texture',read_attachment_object.value,'first_object',source['object'],'tasks',hex(task1['task']),hex(task2['task']),hex(task3['task']),'vector',hex(task1['words'][5]),flush=True)
    second_start = len(self.queue_submissions)
    second_case = dict(OBJECT_CASE)
    second_case['points'] = [[x+5., y+3.] for x,y in OBJECT_CASE['points']]
    second_source = self.run_object_core(second_case)
    second_tasks = self.queue_submissions[second_start:]
    print('SECOND_SUBMISSION', json.dumps({'source':{k:second_source[k] for k in ('object','drawing','canvas','draw_result','callback_count')}, 'tasks':[{'task':hex(t['task']), 'name':vtable_name(t), 'words':[hex(x) for x in t['words']]} for t in second_tasks]},sort_keys=True),flush=True)
    for t in second_tasks:
        entry = self.get(t['vptr']+16, 'Q')[0]
        print('SECOND_TASK_BEGIN',vtable_name(t),hex(t['task']),flush=True)
        self.x(0,t['task']); self.u.reg_write(R.UC_ARM64_REG_SP,0x200f0000); self.u.reg_write(R.UC_ARM64_REG_LR,0x200f1000)
        self.u.emu_start(entry,0x200f1000,count=2000000)
        bound=positive.C.c_int(); get_integer(0x8CA6,positive.C.byref(bound))
        attachment=positive.C.c_int(-1); get_attachment(0x8D40,0x8CE0,0x8CD1,positive.C.byref(attachment))
        print('SECOND_TASK_END',hex(self.u.reg_read(R.UC_ARM64_REG_PC)),'framebuffer',bound.value,'attachment',attachment.value,'draws',len(self.graphics_draw_calls),flush=True)
        if vtable_name(t) == 'vtable for SPen::PenGLRenderMsg' and (bound.value != native_canvas_fbo or attachment.value != read_attachment_object.value): raise RuntimeError('second draw target changed')
    self.mesa.f['glBindFramebuffer'](0x8D40,native_canvas_fbo)
    frame=positive.C.create_string_buffer(321*228*4)
    self.mesa.f['glReadPixels'](0,0,321,228,0x1908,0x1401,positive.C.cast(frame,positive.C.c_void_p))
    print('SECOND_FRAME',sha256(frame.raw),sum(a!=0 for a in frame.raw[3::4]),'gl_error',self.mesa.f['glGetError'](),flush=True)
    Path('/tmp/brush-overlap-frame.rgba').write_bytes(frame.raw)
    raise RuntimeError('end of two-submission probe')
'''
if os.environ.get('BRUSHPEN_OVERLAP_PRESSURE') == '1':
    code = code.replace("    second_source = self.run_object_core(second_case)", "    second_case['pressures'] = [0.8, 0.7, 0.6, 0.5, 0.4, 0.3, 0.2, 0.1]\n    second_source = self.run_object_core(second_case)")
if os.environ.get('BRUSHPEN_OVERLAP_COLOR') == '1':
    s = s.replace(" n=len(case['points']);pts=self.alloc(n*8)", " self.call('SPen::ObjectStroke::GetColor() const',[obj]); print('SOURCE_COLOR',hex(obj),hex(self.x(0)),flush=True)\n n=len(case['points']);pts=self.alloc(n*8)")
    s = s.replace("[obj,0x806e42d1]", "[obj,case.get('color',0xffd04020)]")
    code = code.replace("    second_source = self.run_object_core(second_case)", "    second_case['color'] = 0xff2080e0\n    second_source = self.run_object_core(second_case)")
    code = code.replace("    for t in second_tasks:", "    print('COLOR_UNIFORMS_BEFORE',json.dumps(matrix_uniforms),flush=True)\n    for t in second_tasks:")
    code = code.replace("    self.mesa.f['glBindFramebuffer'](0x8D40,native_canvas_fbo)\n    frame=", "    print('COLOR_UNIFORMS_AFTER',json.dumps(matrix_uniforms),flush=True)\n    self.mesa.f['glBindFramebuffer'](0x8D40,native_canvas_fbo)\n    frame=")
    s = s.replace('# actual update has multiline dict; insert immediately before task2_pixels allocation', 'source=source.replace("    self.x(0, task2[\'task\'])", "    self.mesa.f[\'glBindFramebuffer\'](0x8D40, native_canvas_fbo)\\n    self.mesa.f[\'glClearColor\'](32/255, 160/255, 96/255, 1.0)\\n    self.mesa.f[\'glClear\'](0x4000)\\n    bg = positive.C.create_string_buffer(321*228*4)\\n    self.mesa.f[\'glReadPixels\'](0,0,321,228,0x1908,0x1401,positive.C.cast(bg,positive.C.c_void_p))\\n    if bg.raw != bytes([32,160,96,255])*(321*228) or self.mesa.f[\'glGetError\'](): raise RuntimeError(\'background clear failed\')\\n    self.mesa.f[\'glBindFramebuffer\'](0x8D40, output)\\n    self.x(0, task2[\'task\'])")\n' + '# actual update has multiline dict; insert immediately before task2_pixels allocation')
anchor="ns=q.__dict__.copy();ns['ROOT']=ROOT;"
if os.environ.get('BRUSHPEN_OVERLAP_COLOR') == '1':
    s = s.replace(anchor, anchor + "ns['matrix_uniforms']=matrix_uniforms;")
s=s.replace(anchor, "source=source.replace(\"    # On success task2 is the native draw route\", "+repr(code)+"+\"    # On success task2 is the native draw route\",1)\n"+anchor)
Path('/tmp/brush-overlap-run.py').write_text(s)
