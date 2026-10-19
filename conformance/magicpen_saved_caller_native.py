#!/usr/bin/env python3
"""Hash-gated MagicPen adaptation of the canonical saved-Drawing caller.

This runner does not carry a copied Drawing/Model harness. It verifies the
canonical Fountain caller's exact base hash, applies asserted source patches
in memory, and executes that adapted caller in the same pinned runtime.
"""
from pathlib import Path
import hashlib
import os
import sys

ROOT = Path(__file__).resolve().parents[1]
BASE = ROOT / "conformance/fountain_saved_caller_native.py"
EXPECTED_BASE_SHA256 = "b0837c86483aa438923f9d4d1c6451b1ebc1c5fec4c5f7c447b928a25a8f7b19"
BASE_SHA256 = hashlib.sha256(BASE.read_bytes()).hexdigest()
if BASE_SHA256 != EXPECTED_BASE_SHA256:
    raise SystemExit(f"canonical Fountain caller changed: {BASE_SHA256}")

source = BASE.read_text()

def replace_once(old: str, new: str, label: str) -> None:
    global source
    count = source.count(old)
    if count != 1:
        raise SystemExit(f"patch {label!r}: expected one exact anchor, found {count}")
    source = source.replace(old, new, 1)

def replace_region(start: str, end: str, replacement: str, label: str) -> None:
    global source
    if source.count(start) != 1 or source.count(end) != 1:
        raise SystemExit(f"patch region {label!r}: anchor count changed")
    begin = source.index(start)
    finish = source.index(end)
    if begin >= finish:
        raise SystemExit(f"patch region {label!r}: anchors reversed")
    source = source[:begin] + replacement + source[finish:]

replace_once(
    "FIXTURE=ROOT/'conformance/fountain-saved-caller-native.json'",
    "FIXTURE=ROOT/'conformance/magicpen-saved-caller-native.json'",
    "MagicPen fixture path",
)
replace_once(
    "LIBRARY_SHA256={'FountainPen':'1fed4e071caffcb88a4df4ca345b6df5e3374852c9ef18044793fc2e340ac6ec','PenCommon':'afd39c0d55ec5cf47153be48222c8a9ddc0057fc772dd870af9ced74a59ec33d','DefaultPen':'1b37056c81646c407e9e0697c78e31b50ffab0bdb896966d1e89feb02385544f','Drawing':'788bf413ddeb0b9d352062c5f1b7b8ed11babca911df72691da58ff1a0a5a4bd','Model':'4fbcf6d4213e929f1535d32abb487743643fd5d0dfc366e50dfeb2e7d8015b7a','Base':'e10da0116946691cf68302437ef261282e1dfe0eec15bf2dfa66093286985deb','Graphics':'aac858ce3a9d0353d760b4b0ef09f1e88b0d4a87f5e0906fe8d53936ee8a6621','Composer':'52b83157198368da3a3855a721bfc7d3aafde4e644ce25b5d6eab3b6b510d39f','libc++_shared':'4397241b4bd20a8e579bfb41d21107857e12985f6a01ca0c2a5f83380d1270b4'}",
    "LIBRARY_SHA256={'MagicPen':'e4927da583e96142d072bb3edd977334aa34a58bb723ab1f9aaf531f3bd670a2','FountainPen':'1fed4e071caffcb88a4df4ca345b6df5e3374852c9ef18044793fc2e340ac6ec','ObliquePen':'9b76bdb709db9f56c5d35a1a615fe41c57ceef73c559c91efaf5bf436f90e0ee','PenCommon':'afd39c0d55ec5cf47153be48222c8a9ddc0057fc772dd870af9ced74a59ec33d','DefaultPen':'1b37056c81646c407e9e0697c78e31b50ffab0bdb896966d1e89feb02385544f','Drawing':'788bf413ddeb0b9d352062c5f1b7b8ed11babca911df72691da58ff1a0a5a4bd','Model':'4fbcf6d4213e929f1535d32abb487743643fd5d0dfc366e50dfeb2e7d8015b7a','Base':'e10da0116946691cf68302437ef261282e1dfe0eec15bf2dfa66093286985deb','Graphics':'aac858ce3a9d0353d760b4b0ef09f1e88b0d4a87f5e0906fe8d53936ee8a6621','Renderer':'f38df5db5e64f80c0641b6cee980e14533bd78e8e6eb34f250bd703d28119aed','Composer':'52b83157198368da3a3855a721bfc7d3aafde4e644ce25b5d6eab3b6b510d39f','libc++_shared':'4397241b4bd20a8e579bfb41d21107857e12985f6a01ca0c2a5f83380d1270b4'}",
    "MagicPen DSO and Renderer preflight",
)

replace_once(
    "# Explicitly initialize the mapped FountainPen plugin and its static factory data.\nFP=LIBS/'libSPenFountainPen.so'; fpbase=rt.bases['FountainPen']; assert hashlib.sha256(FP.read_bytes()).hexdigest()=='1fed4e071caffcb88a4df4ca345b6df5e3374852c9ef18044793fc2e340ac6ec'\nrt.run_init(FP,fpbase)",
    """# Map/init the pinned MagicPen DSO at a fresh address, using the runtime's real libc++ and symbol tables.
MP=LIBS/'libSPenMagicPen.so'; mpbase=0xA000000
def map_plugin(path,base):
 raw=path.read_bytes(); phoff=struct.unpack_from('<Q',raw,32)[0]; ent,num=struct.unpack_from('<HH',raw,54); extent=0
 for i in range(num):
  typ,fl,off,va,pa,fs,ms,al=struct.unpack_from('<II6Q',raw,phoff+i*ent)
  if typ==1:extent=max(extent,va+ms)
 p.u.mem_map(base,(extent+0xfff)&~0xfff)
 for i in range(num):
  typ,fl,off,va,pa,fs,ms,al=struct.unpack_from('<II6Q',raw,phoff+i*ent)
  if typ==1 and ms:
   p.u.mem_write(base+va,raw[off:off+fs])
   if ms>fs:p.u.mem_write(base+va+fs,bytes(ms-fs))
 for line in __import__('subprocess').check_output(['nm','-D','-C',str(path)],text=True,env=env).splitlines():
  parts=line.strip().split(' ',2)
  if len(parts)==3 and parts[1] in ('T','W'):p.syms[parts[2]]=base+int(parts[0],16)
 for line in __import__('subprocess').check_output(['nm','-D',str(path)],text=True,env=env).splitlines():
  parts=line.strip().split()
  if len(parts)==3 and parts[1].upper() in ('T','W','D','B','R','V'):rt.rawsyms.setdefault(parts[2],base+int(parts[0],16))
 shoff=struct.unpack_from('<Q',raw,40)[0]; se,sn,ss=struct.unpack_from('<HHH',raw,58); secs=[struct.unpack_from('<IIQQQQIIQQ',raw,shoff+i*se) for i in range(sn)]; names=raw[secs[ss][4]:secs[ss][4]+secs[ss][5]]; by={names[s[0]:names.find(b'\\0',s[0])].decode():s for s in secs}; ds,st=by['.dynsym'],by['.dynstr']; strings=raw[st[4]:st[4]+st[5]]; sy=[]
 for off in range(ds[4],ds[4]+ds[5],ds[9]):
  no,info,other,ndx,val,size=struct.unpack_from('<IBBHQQ',raw,off); sy.append((strings[no:strings.find(b'\\0',no)].decode() if no else '',val))
 for secname in ('.rela.dyn','.rela.plt'):
  sec=by.get(secname)
  if not sec:continue
  for off in range(sec[4],sec[4]+sec[5],sec[9]):
   where,info,add=struct.unpack_from('<QQq',raw,off); typ,idx=info&0xffffffff,info>>32
   if typ==0x403:p.u.mem_write(base+where,struct.pack('<Q',base+add))
   elif typ in (0x401,0x402,0x101,0x1025,0x1026):
    name,val=sy[idx]; target=base+val if val else rt.rawsyms.get(name,0) or p.syms.get(name,0)
    if target:p.u.mem_write(base+where,struct.pack('<Q',target+(add if typ in (0x401,0x101) else 0)))
 for off,sym in __import__('elf_plt').resolve(path).items():
  pretty=__import__('subprocess').check_output(['c++filt','-n',sym],text=True).strip() if sym.startswith('_Z') else sym
  p.plt[base+off]=pretty
 rt.run_init(path,base)
 return extent
map_plugin(MP,mpbase)
# This is a direct NEEDED dependency and is hash-pinned above, but the bounded
# ObjectStroke geometry route does not map/execute Renderer.
""",
    "MagicPen loader/init block",
)

replace_once(
    "pen_set_calls=[]; add_calls=[]; redraws=[]",
    "pen_set_calls=[]; add_calls=[]; redraws=[]; rt_buffer_snapshots=[]; pending_add_receivers=[]",
    "RTV1 return snapshots",
)
replace_once(
    " def trace(u,a,z,d):\n  if a==set_size_addr:",
    """ def trace(u,a,z,d):
  if a==mpbase+0x2f530:
   if not pending_add_receivers:raise RuntimeError('RTV1 AddPoint return without entry receiver')
   receiver=pending_add_receivers.pop(); vecptr=p.get(receiver+80,'Q')[0]
   if not vecptr:raise RuntimeError(f'RTV1 vector pointer null at AddPoint return for {receiver:#x}')
   begin,end,capacity=p.get(vecptr,'QQQ')
   if not(begin<=end<=capacity) or (end-begin)%4:raise RuntimeError(f'invalid native MagicPen RTV1 vector {begin:#x}/{end:#x}/{capacity:#x}')
   raw=bytes(u.mem_read(begin,end-begin)) if end>begin else b''
   rt_buffer_snapshots.append({'receiver':hex(receiver),'vector_object':hex(vecptr),'begin_end_capacity':[hex(begin),hex(end),hex(capacity)],'float_count':(end-begin)//4,'bytes_le_hex':raw.hex(),'sha256':hashlib.sha256(raw).hexdigest()})
  elif a==set_size_addr:""",
    "AddPoint epilogue snapshot hook",
)
replace_once(
    "  elif a in add_addrs:\n   _words=",
    "  elif a in add_addrs:\n   pending_add_receivers.append(p.x(0))\n   _words=",
    "AddPoint entry receiver stack",
)
replace_once(
    " p.u.hook_del(trace_hook)\n records.append(",
    " if pending_add_receivers:raise RuntimeError(f'unmatched native AddPoint entries: {len(pending_add_receivers)}')\n p.u.hook_del(trace_hook)\n native_rtv1_vector=rt_buffer_snapshots[-1] if rt_buffer_snapshots else None\n records.append(",
    "require complete AddPoint returns",
)
replace_once(
    "'redraw_entries':redraws,'rtv_addpoint_entries':add_calls,",
    "'redraw_entries':redraws,'rtv_addpoint_entries':add_calls,'native_rtv1_vector':native_rtv1_vector,'native_rtv1_vector_snapshots':rt_buffer_snapshots,'native_model_metadata':{'tool_type':model_tool,'stroke_type':model_stroke,'curve_enabled':model_curve,'color_argb':model_color},",
    "native post-return buffer fields",
)

replace_once(
    "# Module-loader host adapter resolves only the already-mapped, pinned FountainPen plugin.\ndef cstr(a):return bytes(p.u.mem_read(a,128)).split(b'\\0',1)[0].decode('ascii','replace')\nloader=[]\nFP_HANDLE=0x50000001; DP_HANDLE=0x90000001\ndef dlopen():\n name=cstr(p.x(0));loader.append({'fn':'dlopen','name':name,'lr':hex(p.u.reg_read(R.UC_ARM64_REG_LR))})\n if Path(name).name=='libSPenFountainPen.so':p.x(0,FP_HANDLE)\n elif Path(name).name=='libSPenDefaultPen.so':p.x(0,DP_HANDLE)\n else:raise RuntimeError('unexpected DSO '+name)\ndef dlsym():\n handle=p.x(0); name=cstr(p.x(1));loader.append({'fn':'dlsym','handle':hex(handle),'name':name,'lr':hex(p.u.reg_read(R.UC_ARM64_REG_LR))})\n if name!='createPenInst' or handle not in (FP_HANDLE,DP_HANDLE):raise RuntimeError('unexpected dlsym '+hex(handle)+' '+name)\n plugin='libSPenFountainPen.so' if handle==FP_HANDLE else 'libSPenDefaultPen.so'\n raw=FP.read_bytes() if handle==FP_HANDLE else DP.read_bytes()\n nm=__import__('subprocess').check_output(['nm','-D',str(FP if handle==FP_HANDLE else DP)],text=True,env=env)\n offsets=[int(line.split()[0],16) for line in nm.splitlines() if line.split()[-1:] == ['createPenInst']]\n if not offsets:raise RuntimeError(plugin+' lacks createPenInst')\n p.x(0,(fpbase if handle==FP_HANDLE else dpbase)+offsets[0])\np.callbacks['dlopen']=dlopen;p.callbacks['dlsym']=dlsym;p.callbacks['dlerror']=lambda:p.x(0,0)",
    """# Loader addresses only the already mapped, hash-pinned plugins.
def cstr(a):return bytes(p.u.mem_read(a,128)).split(b'\\0',1)[0].decode('ascii','replace')
loader=[]
MP_HANDLE=0x50000001; DP_HANDLE=0x90000001
def dlopen():
 name=cstr(p.x(0));loader.append({'fn':'dlopen','name':name,'lr':hex(p.u.reg_read(R.UC_ARM64_REG_LR))})
 if Path(name).name=='libSPenMagicPen.so':p.x(0,MP_HANDLE)
 elif Path(name).name=='libSPenDefaultPen.so':p.x(0,DP_HANDLE)
 else:raise RuntimeError('unexpected DSO '+name)
def dlsym():
 handle=p.x(0); name=cstr(p.x(1));loader.append({'fn':'dlsym','handle':hex(handle),'name':name,'lr':hex(p.u.reg_read(R.UC_ARM64_REG_LR))})
 if name!='createPenInst' or handle not in (MP_HANDLE,DP_HANDLE):raise RuntimeError('unexpected dlsym '+hex(handle)+' '+name)
 path=MP if handle==MP_HANDLE else DP
 base=mpbase if handle==MP_HANDLE else dpbase
 nm=__import__('subprocess').check_output(['nm','-D',str(path)],text=True,env=env)
 offsets=[int(line.split()[0],16) for line in nm.splitlines() if line.split()[-1:] == ['createPenInst']]
 if len(offsets)!=1:raise RuntimeError(str(path)+' createPenInst symbol count '+str(len(offsets)))
 p.x(0,base+offsets[0])
p.callbacks['dlopen']=dlopen;p.callbacks['dlsym']=dlsym;p.callbacks['dlerror']=lambda:p.x(0,0)""",
    "MagicPen dynamic loader",
)

replace_once(
    "for label,mode,size,advanced_value in [(f'{v14}_{case}', mode, size, '14;') for v14 in ('v14',) for case,mode,size in [('omitted','omit',None),('zero','set',0.),('small','set',.1),('normal','set',10.)]] + [(f'v16_{case}',mode,size,'18;0;100;') for case,mode,size in [('omitted','omit',None),('zero','set',0.),('small','set',.1),('normal','set',10.)]]:\n loader.clear()",
    "for label,advanced_value,advanced_mode in [('unset',None,'omitted'),('null-arg',None,'null'),('empty','','value'),('v1','1;','value'),('v2','2;','value')]:\n mode='set'; size=10.0\n loader.clear()",
    "five advanced-setting controls",
)
replace_once(
    "call('SPen::ObjectStroke::SetPenName(SPen::String const*)',[obj,string('com.samsung.android.sdk.pen.pen.preload.FountainPen')])",
    "call('SPen::ObjectStroke::SetPenName(SPen::String const*)',[obj,string('com.samsung.android.sdk.pen.pen.preload.MagicPen')])",
    "MagicPen ObjectStroke name",
)
replace_once(
    "call('SPen::ObjectStroke::GetPointCount() const',[obj]); model_count=p.x(0)",
    "call('SPen::ObjectStroke::GetPointCount() const',[obj]); model_count=p.x(0)\n call('SPen::ObjectStroke::GetToolType() const',[obj]); model_tool=p.x(0)\n call('SPen::ObjectStroke::GetStrokeType() const',[obj]); model_stroke=p.x(0)\n call('SPen::ObjectStroke::IsCurveEnabled() const',[obj]); model_curve=bool(p.x(0))\n call('SPen::ObjectStroke::GetColor() const',[obj]); model_color=f'0x{p.x(0)&0xffffffff:08x}'",
    "MagicPen source metadata getters",
)
replace_once(
    "# Native selection contract V14 is exercised with the actual saved advanced setting.\n call('SPen::ObjectStroke::SetAdvancedPenSetting(SPen::String const*)',[obj,string(advanced_value)])",
    """# Exercise setter omitted, null pointer, empty String, and explicit version values.
 if advanced_mode=='null':call('SPen::ObjectStroke::SetAdvancedPenSetting(SPen::String const*)',[obj,0])
 elif advanced_mode=='value':call('SPen::ObjectStroke::SetAdvancedPenSetting(SPen::String const*)',[obj,string(advanced_value)])
 def read_native_string(ptr):
  if not ptr:return None
  call('SPen::String::GetUTF8Size() const',[ptr]); n=p.x(0)
  if n<0 or n>4096:raise RuntimeError(f'invalid native string size {n}')
  buf=p.alloc(n+1);call('SPen::String::GetUTF8(char*, int) const',[ptr,buf,n+1])
  return bytes(p.u.mem_read(buf,n+1)).split(b'\\0',1)[0].decode('ascii','replace')
 call('SPen::ObjectStroke::GetAdvancedPenSetting() const',[obj]); advanced_ptr=p.x(0); advanced_native=read_native_string(advanced_ptr)""",
    "MagicPen advanced-setting controls",
)

replace_once(
    "redraw_names=['SPen::FountainPenStrokeDrawableGLV14::RedrawPen(SPen::ObjectStroke const*, SPen::RectF*)','SPen::FountainPenStrokeDrawableGLV16::RedrawPen(SPen::ObjectStroke const*, SPen::RectF*)']\n redraw_addrs={p.syms[name]:name for name in redraw_names}\n add_names=['SPen::FountainPenStrokeDrawableRTV4::AddPoint(SPen::Vector3<float>, SPen::Vector2<float>, float)','SPen::FountainPenStrokeDrawableRTV5::AddPoint(SPen::Vector3<float>, float)','SPen::FountainPenStrokeDrawableRTV6::AddPoint(SPen::Vector3<float>, float, SPen::Vector3<float>)'];add_addrs={p.syms[name]:name for name in add_names}",
    "redraw_names=['SPen::MagicPenStrokeDrawableGLV1::RedrawPen(SPen::ObjectStroke const*, SPen::RectF*)','SPen::MagicPenStrokeDrawableGLV2::RedrawPen(SPen::ObjectStroke const*, SPen::RectF*)']\n redraw_addrs={p.syms[name]:name for name in redraw_names}\n add_names=['SPen::MagicPenStrokeDrawableRTV1::AddPoint(SPen::Vector2<float>, float)'];add_addrs={p.syms[name]:name for name in add_names}",
    "MagicPen trace symbols",
)
replace_once(
    "  _words=[struct.pack('<I',u.reg_read(getattr(R,f'UC_ARM64_REG_S{i}'))).hex() for i in range(6)]\n   add_calls.append({'entry':add_addrs[a],'receiver':hex(p.x(0)),'float_words_s0_s5':_words,'native_addpoint_channels':{'position_xyz_f32_bits':_words[0:3],'direction_xy_f32_bits':_words[3:5],'transparency_f32_bits':_words[5]}})",
    "  _words=[struct.pack('<I',u.reg_read(getattr(R,f'UC_ARM64_REG_S{i}'))).hex() for i in range(3)]\n   add_calls.append({'entry':add_addrs[a],'entry_address':hex(a),'receiver':hex(p.x(0)),'float_words_s0_s2':_words,'native_addpoint_channels':{'position_xy_f32_bits':_words[0:2],'radius_f32_bits':_words[2]}})",
    "RTV1 float arguments",
)
replace_once(
    " try: drawable=call('SPen::FountainPen::GetStrokeDrawableGL()',[selected_pen]) if selected_pen else 0\n except Exception as e: drawable_error=str(e); drawable=0\n else: drawable_error=None",
    " if not selected_pen:raise RuntimeError('native ObjectDrawing did not select a MagicPen')\n drawable=call('SPen::MagicPen::GetStrokeDrawableGL()',[selected_pen])\n drawable_error=None",
    "MagicPen factory getter",
)
replace_once(
    "records.append({'case':label,'advanced_setting':advanced_value,",
    "records.append({'case':label,'advanced_setting':advanced_value,'advanced_mode':advanced_mode,'advanced_setting_getter':{'is_null':advanced_ptr==0,'utf8':advanced_native},",
    "capture settings metadata",
)

replace_region(
    "for _row in records:\n _versions=",
    "# Source and library pins were checked before runtime setup; include the stable",
    """for _row in records:
 _versions=[f'GLV{v}' for v in (1,2) if any(f'GLV{v}::RedrawPen' in call['entry'] for call in _row['redraw_entries'])]
 if len(_versions)!=1:raise RuntimeError(f'expected one MagicPen drawable class for {_row["case"]}: {_versions}')
 _row['selected_drawable_class']=_versions[0]
 _expected={'unset':'GLV1','null-arg':'GLV1','empty':'GLV2','v1':'GLV1','v2':'GLV2'}[_row['case']]
 if _versions[0]!=_expected:raise RuntimeError(f'wrong selected class for {_row["case"]}: {_versions[0]}')
 _expected_setting={'unset':(True,None),'null-arg':(True,None),'empty':(False,''),'v1':(False,'1;'),'v2':(False,'2;')}[_row['case']]
 if (_row['advanced_setting_getter']['is_null'],_row['advanced_setting_getter']['utf8'])!=_expected_setting:raise RuntimeError(f'advanced getter changed for {_row["case"]}')
 if _row['drawing_return']!=1 or len(_row['redraw_entries'])!=1:raise RuntimeError(f'MagicPen Drawing route did not run once for {_row["case"]}')
 if not any(item['fn']=='dlsym' and item['name']=='createPenInst' and item['handle']=='0x50000001' for item in _row['loader_calls']):raise RuntimeError('MagicPen factory not selected')
 if _row['source_model_get_point_count']!=3:raise RuntimeError('source ObjectStroke point count changed')
 if _row['source_arrays']!={'points_f32_bits':['00000000','00000000','41200000','00000000','41a00000','40000000'],'pressure_f32_bits':['3e99999a','3f19999a','3f800000'],'timestamps_i32':[100,110,120],'tilt_f32_bits':['00000000']*3,'orientation_f32_bits':['00000000']*3}:raise RuntimeError('native ObjectStroke channel getters differ from configured input')
 if _row['native_model_metadata']!={'tool_type':2,'stroke_type':0,'curve_enabled':True,'color_argb':'0xff000000'}:raise RuntimeError('native ObjectStroke metadata getters differ from configured input')
 if not _row['rtv_addpoint_entries'] or len({item['receiver'] for item in _row['rtv_addpoint_entries']})!=1:raise RuntimeError('RTV1 stream is empty or changed receiver')
 _words=''.join(word for row in _row['rtv_addpoint_entries'] for word in row['float_words_s0_s2'])
 _row['rtv_addpoint_argument_stream_hex_le']=_words
 _row['rtv_addpoint_argument_stream_sha256']=hashlib.sha256(bytes.fromhex(_words)).hexdigest()
 _row['rtv_addpoint_record_count']=len(_row['rtv_addpoint_entries'])
 if len(_row['native_rtv1_vector_snapshots'])!=len(_row['rtv_addpoint_entries']):raise RuntimeError('missing native AddPoint epilogue vector snapshots')
 if _row['native_rtv1_vector'] is None or _row['native_rtv1_vector']['float_count']!=3*_row['rtv_addpoint_record_count']:raise RuntimeError('native post-return RTV1 vector count mismatch')
 expected=bytearray()
 for row in _row['rtv_addpoint_entries']:
  words=row['float_words_s0_s2']; expected.extend(bytes.fromhex(words[0])); expected.extend(bytes.fromhex(words[1]))
  expected.extend(struct.pack('<f',struct.unpack('<f',bytes.fromhex(words[2]))[0]+0.5))
 if bytes(expected)!=bytes.fromhex(_row['native_rtv1_vector']['bytes_le_hex']):raise RuntimeError('native vector is not the AddPoint argument stream with native radius +0.5')
 _row['evidence_scope']='synthetic native ObjectStroke setter input; native Drawing selection and RedrawPen/AddPoint; no SDK deserialization or paint output'
""",
    "MagicPen bounded assertions",
)

replace_once(
    "'sdocx-fountain-saved-caller-v1'",
    "'sdocx-magicpen-saved-caller-v1'",
    "MagicPen output schema",
)
replace_once(
    "'pen_name':'com.samsung.android.sdk.pen.pen.preload.FountainPen'",
    "'pen_name':'com.samsung.android.sdk.pen.pen.preload.MagicPen'",
    "MagicPen source metadata",
)
replace_once(
    "'runner_boundary':'Native Model ObjectStroke setters/getters, DrawingFactory, ObjectDrawing::drawObjectStroke, PenManager lookup, and selected FountainPen RedrawPen execute. V14 enters native RTV4 AddPoint; V16 evidence is limited to selected GLV16 class and size handoff, with no V16 geometry capture. IContext dimensions, queue dispatch, allocator, CanvasProbe drawing callbacks, and non-null synthetic dlopen/dlsym handles are host adapters; no raster output is claimed.'",
    "'runner_boundary':'Native Model ObjectStroke setters/getters, DrawingFactory, ObjectDrawing::drawObjectStroke, MagicPen factory, selected GLV1/GLV2 RedrawPen, and RTV1 AddPoint execute. IContext dimensions, queue dispatch, allocator, CanvasProbe callbacks, and dlopen/dlsym handles are host adapters. Renderer is a pinned direct dependency but is not mapped/executed. No serialized Note reader, texture binding, GPU draw, or raster output is claimed.'",
    "bounded evidence boundary",
)

# Distinguish actually mapped libraries from the ELF NEEDED inventory.
needle = "if MODE_ARGS.write_fixture:\n"
if source.count(needle) != 1:
    raise SystemExit("output-mode anchor count changed")
source = source.replace(
    needle,
    "value['provenance']['base_caller_path']='conformance/fountain_saved_caller_native.py'\n"
    "value['provenance']['base_caller_sha256']='b0837c86483aa438923f9d4d1c6451b1ebc1c5fec4c5f7c447b928a25a8f7b19'\n"
    "value['provenance']['adapter_patch_labels']=['MagicPen fixture path','MagicPen DSO and Renderer preflight','MagicPen loader/init block','RTV1 return snapshots','AddPoint epilogue snapshot hook','AddPoint entry receiver stack','require complete AddPoint returns','native post-return buffer fields','MagicPen dynamic loader','five advanced-setting controls','MagicPen ObjectStroke name','MagicPen source metadata getters','MagicPen advanced-setting controls','MagicPen trace symbols','RTV1 float arguments','MagicPen factory getter','capture settings metadata','MagicPen bounded assertions','MagicPen output schema','MagicPen source metadata','bounded evidence boundary']\n"
    "value['provenance']['mapped_dso_names']=['MagicPen','FountainPen','ObliquePen','PenCommon','Base','Drawing','Graphics','DefaultPen','Model','Composer','libc++_shared']\n"
    "value['provenance']['direct_needed_unmapped']={'Renderer':'f38df5db5e64f80c0641b6cee980e14533bd78e8e6eb34f250bd703d28119aed'}\n"
    "value['provenance']['direct_needed_system']=['liblog.so','libm.so','libdl.so','libc.so']\n"
    + needle,
    1,
)

replace_once(
    "print(f'verified {len(records)} Fountain Drawing cases')",
    "print(f'verified {len(records)} MagicPen Drawing cases')",
    "MagicPen check-fixture label",
)

# Keep the canonical script's normal fixture write/check behavior but run its
# transformed bytes with this wrapper's __file__ and the caller's CLI mode.
exec(compile(source, str(BASE), "exec"), {"__name__": "__main__", "__file__": str(Path(__file__).resolve())})
