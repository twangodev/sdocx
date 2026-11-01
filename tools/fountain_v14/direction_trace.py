"""Fresh native Drawing state capture for the 29, 30, 30 lifecycle."""
import state_probe_capture as probe

before = "            before=read.raw\n            native_call(p,'SPen::ObjectDrawing::drawObjectStroke"
after = "            before=read.raw\n            pen_before=bytes(p.u.mem_read(state['pen'],512)).hex() if state['pen'] else None\n            native_call(p,'SPen::ObjectDrawing::drawObjectStroke"
assert probe.SCENE.count(before) == 1
probe.SCENE = probe.SCENE.replace(before, after)
before = "            draws=draw_attribute_joins[first_draw:];stream_key=None;mode=None"
after = "            pen_after=bytes(p.u.mem_read(state['pen'],512)).hex() if state['pen'] else None\n            draws=draw_attribute_joins[first_draw:];stream_key=None;mode=None"
assert probe.SCENE.count(before) == 1
probe.SCENE = probe.SCENE.replace(before, after)
before = "'native_draws':len(draws),'native_counts'"
after = "'native_draws':len(draws),'pen_before_hex':pen_before,'pen_after_hex':pen_after,'native_counts'"
assert probe.SCENE.count(before) == 1
probe.SCENE = probe.SCENE.replace(before, after)
# Trace native writes carrying the terminal direction observed in the retained stream.
old = "    matrix=p.alloc(36);sequence=[29,30,30]"
new = """    from unicorn import UC_HOOK_MEM_WRITE, UC_HOOK_MEM_READ
    trace=[]; active=[False]; direction_address=[None]
    target_bits={struct.unpack('<I',struct.pack('<f',v))[0] for v in [0.2636241018772125,0.9646254777908325]}
    def trace_write(u,access,address,size,value,data):
        if active[0] and any(((value >> (8*i)) & 0xffffffff) in target_bits for i in range(max(0,size-3))):
            pc=u.reg_read(R.UC_ARM64_REG_PC)
            if pc==0x20be154 and (value & 0xffffffff)==0x3e86f9bd: direction_address[0]=address
            trace.append({'kind':'write','pc':hex(pc),'address':hex(address),'size':size,'value':hex(value & ((1 << (size*8))-1))})
    def trace_read(u,access,address,size,value,data):
        base=direction_address[0]
        if active[0] and base is not None and address < base+8 and address+size > base:
            trace.append({'kind':'read','pc':hex(u.reg_read(R.UC_ARM64_REG_PC)),'address':hex(address),'size':size})
    p.u.hook_add(UC_HOOK_MEM_READ,trace_read)
    p.u.hook_add(UC_HOOK_MEM_WRITE,trace_write)
    matrix=p.alloc(36);sequence=[29,30,30]"""
assert probe.SCENE.count(old)==1
probe.SCENE=probe.SCENE.replace(old,new)
probe.SCENE=probe.SCENE.replace("            pen_before=bytes(","            trace.clear();active[0]=True\n            pen_before=bytes(")
probe.SCENE=probe.SCENE.replace("            pen_after=bytes(","            active[0]=False\n            pen_after=bytes(")
probe.SCENE=probe.SCENE.replace("'pen_before_hex':pen_before", "'direction_writes':list(trace),'pen_pointer':hex(state['pen']),'pen_before_hex':pen_before")
original = probe.base.replace
def redirect(body, old, new):
    if new == 'conformance/fountain-v14-state-probe-native.json':
        new = '/tmp/sdocx-fountain-direction-trace.json'
    return original(body, old, new)
probe.base.replace = redirect
probe.main()
