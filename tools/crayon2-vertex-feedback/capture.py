import sys
from pathlib import Path
s=Path('tools/crayon2-transform-control/intervention.py').read_text()
anchor="a='# Keep the native texture-scale uniform alongside the paint calls.'"
probe='''
tf_shader=set()
old_source=p.callbacks['glShaderSource']
def tf_source():
 shader,count,arr,lens=[p.x(i) for i in range(4)]
 raw=p.cstr(p.get(arr,'Q')[0]) if count==1 and not lens else b''
 if b'out vec2 vBgCanvasUV' in raw: tf_shader.add(shader)
 return old_source()
p.callbacks['glShaderSource']=tf_source
fn=lambda n,ret,args:ip.cfunc(p.gles.gl,n,ret,args)
getattached=fn('glGetAttachedShaders',None,[C.c_uint,C.c_int,C.POINTER(C.c_int),C.POINTER(C.c_uint)])
tfvary=fn('glTransformFeedbackVaryings',None,[C.c_uint,C.c_int,C.POINTER(C.c_char_p),C.c_uint])
old_link=p.callbacks['glLinkProgram']
tf_program=set()
def tf_link():
 program=p.x(0); n=C.c_int(); ids=(C.c_uint*16)();getattached(program,16,C.byref(n),ids)
 if any(x in tf_shader for x in ids[:n.value]):
  names=(C.c_char_p*2)(b'gl_Position',b'vBgCanvasUV');tfvary(program,2,names,0x8c8c);tf_program.add(program);tf_shader.clear()
 result=old_link()
 if program in tf_program:
  getiv=fn('glGetProgramiv',None,[C.c_uint,C.c_uint,C.POINTER(C.c_int)]);status=C.c_int();getiv(program,0x8b82,C.byref(status));assert status.value == 1;getlog=fn('glGetProgramInfoLog',None,[C.c_uint,C.c_int,C.POINTER(C.c_int),C.c_char_p]); buf=C.create_string_buffer(4096); n=C.c_int();getlog(program,4096,C.byref(n),buf);print('TF_LINK',buf.value,file=__import__('sys').stderr)
 return result
p.callbacks['glLinkProgram']=tf_link
'''
a2='# Split the native helper\'s instanced draw from its diagnostic mask readback so'
post='''
old_draw=p.callbacks['glDrawArraysInstanced']
def tf_draw():
 vals=[p.x(i) for i in range(4)]
 old_draw()
 state=p.gl_state()
 if state['program'] not in tf_program:return
 gen=fn('glGenBuffers',None,[C.c_int,C.POINTER(C.c_uint)]);bind=fn('glBindBuffer',None,[C.c_uint,C.c_uint]);data=fn('glBufferData',None,[C.c_uint,C.c_ssize_t,C.c_void_p,C.c_uint]);base=fn('glBindBufferBase',None,[C.c_uint,C.c_uint,C.c_uint]);begin=fn('glBeginTransformFeedback',None,[C.c_uint]);end=fn('glEndTransformFeedback',None,[]);mapbuf=fn('glMapBufferRange',C.c_void_p,[C.c_uint,C.c_ssize_t,C.c_ssize_t,C.c_uint]);unmap=fn('glUnmapBuffer',C.c_ubyte,[C.c_uint]);delete=fn('glDeleteBuffers',None,[C.c_int,C.POINTER(C.c_uint)])
 getint=fn('glGetIntegerv',None,[C.c_uint,C.POINTER(C.c_int)]);getidx=fn('glGetIntegeri_v',None,[C.c_uint,C.c_uint,C.POINTER(C.c_int)]);isenabled=fn('glIsEnabled',C.c_ubyte,[C.c_uint]);generic=C.c_int();indexed=C.c_int();getint(0x8c8f,C.byref(generic));getidx(0x8c8f,0,C.byref(indexed));assert generic.value == indexed.value == 0 and not isenabled(0x8c89);assert vals == [5,0,4,1];assert p.gles.f['glGetError']() == 0
 b=C.c_uint();gen(1,C.byref(b));bind(0x8c8e,b);data(0x8c8e,144,None,0x88e1);base(0x8c8e,0,b)
 p.gles.f['glEnable'](0x8c89);begin(4);p.gles.f['glDrawArraysInstanced'](*vals);end();p.gles.f['glDisable'](0x8c89)
 ptr=mapbuf(0x8c8e,0,144,1)
 print('TF_CAPTURE',C.string_at(ptr,144).hex() if ptr else 'NULL','error',p.gles.f['glGetError'](),file=__import__('sys').stderr)
 if ptr:unmap(0x8c8e)
 base(0x8c8e,0,0);bind(0x8c8e,0);delete(1,C.byref(b));getint(0x8c8f,C.byref(generic));getidx(0x8c8f,0,C.byref(indexed));assert generic.value == indexed.value == 0 and not isenabled(0x8c89);assert p.gles.f['glGetError']() == 0
p.callbacks['glDrawArraysInstanced']=tf_draw
'''
a3="p.callbacks['glDrawArraysInstanced']=split_instanced_draw"
inject=("source=source.replace('translation=(13.0,9.0)', 'translation=(0.0,0.0)')\n" if __import__('os').environ.get('CRAYON_IDENTITY') else '')+'assert source.count('+repr(a2)+') == 1\nsource=source.replace('+repr(a2)+','+repr(probe+'\n'+a2)+')\nassert source.count('+repr(a3)+') == 1\nsource=source.replace('+repr(a3)+','+repr(a3+'\n'+post)+')\n'
s=s.replace(anchor,anchor+'\ns=s.replace(a, '+repr(inject)+'+a)')
sys.argv=['tools/crayon2-transform-control/intervention.py','139','paint']
exec(compile(s,'tools/crayon2-transform-control/intervention.py','exec'),{'__name__':'__main__','__file__':str(Path('tools/crayon2-transform-control/intervention.py').resolve())})
