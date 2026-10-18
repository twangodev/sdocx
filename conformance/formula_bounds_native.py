#!/usr/bin/env python3
"""Capture hash-pinned Model stroke drawn bounds and Formula list unions.

Run from the repository root with extracted APK libraries and local Unicorn:
  PYTHONPATH=scratch/apk-analysis-runtime/python python3 conformance/formula_bounds_native.py --check-fixture
"""
import argparse, hashlib, json, math, struct
from pathlib import Path
from unicorn import Uc, UC_ARCH_ARM64, UC_MODE_ARM, UC_HOOK_CODE
from unicorn import arm64_const as R
ROOT=Path(__file__).resolve().parents[1]
MODEL=ROOT/'scratch/apk-analysis-native/arm64-v8a/libSPenModel.so'
BASE=ROOT/'scratch/apk-analysis-native/arm64-v8a/libSPenBase.so'
FIXTURE=ROOT/'conformance/formula-drawn-bounds.json'
BMODEL=0x40000000; BBASE=0x50000000; HEAP=0x10000000; STACK=0x30000000; STOP=STACK+0x1000; HOST=0x60000000; VTABLE=HEAP+0x1000; OWNER=HEAP+0x2000; IMPL=HEAP+0x3000; RAW=HEAP+0x4000; TLS=HEAP+0x8000

def mapelf(uc,path,base):
 b=path.read_bytes(); phoff=struct.unpack_from('<Q',b,32)[0]; es,n=struct.unpack_from('<HH',b,54)
 for i in range(n):
  typ,flags,off,va,pa,fs,ms,al=struct.unpack_from('<II6Q',b,phoff+i*es)
  if typ==1 and ms:
   uc.mem_write(base+va,b[off:off+fs]);
   if ms>fs: uc.mem_write(base+va+fs,bytes(ms-fs))
 return b

def sha(path): return hashlib.sha256(path.read_bytes()).hexdigest()

def w64(uc,a,x): uc.mem_write(a,struct.pack('<Q',x))
def f32(uc,a,x): uc.mem_write(a,struct.pack('<f',x))
def u32(uc,a): return struct.unpack('<I',uc.mem_read(a,4))[0]
def vals(uc,regs): return [struct.unpack('<f',struct.pack('<I',uc.reg_read(r)))[0] for r in regs]
def call(uc,fn,args=()):
 for i,a in enumerate(args): uc.reg_write(R.UC_ARM64_REG_X0+i,a)
 uc.reg_write(R.UC_ARM64_REG_SP,STACK+0x8000); uc.reg_write(R.UC_ARM64_REG_LR,STOP)
 uc.emu_start(fn,STOP,count=500000)
 if uc.reg_read(R.UC_ARM64_REG_PC)!=STOP: raise RuntimeError(f'call did not return {fn:x} pc={uc.reg_read(R.UC_ARM64_REG_PC):x}')
 return uc.reg_read(R.UC_ARM64_REG_X0)

def cstr(uc,p):
 out=bytearray()
 for i in range(256):
  c=uc.mem_read(p+i,1)[0]
  if not c: break
  out.append(c)
 return out.decode(errors='replace')

def run(name,size,rot,rect):
 if sha(MODEL)!='4fbcf6d4213e929f1535d32abb487743643fd5d0dfc366e50dfeb2e7d8015b7a': raise ValueError('Model hash mismatch')
 if sha(BASE)!='e10da0116946691cf68302437ef261282e1dfe0eec15bf2dfa66093286985deb': raise ValueError('Base hash mismatch')
 uc=Uc(UC_ARCH_ARM64,UC_MODE_ARM); uc.mem_map(BMODEL,0x600000); uc.mem_map(BBASE,0x200000); uc.mem_map(HEAP,0x10000); uc.mem_map(STACK,0x10000); uc.mem_map(HOST,0x1000)
 mapelf(uc,MODEL,BMODEL); mapelf(uc,BASE,BBASE)
 uc.reg_write(R.UC_ARM64_REG_TPIDR_EL0,TLS); w64(uc,TLS+40,0x1234)
 w64(uc,OWNER,VTABLE); w64(uc,VTABLE+136,HOST+0x100); w64(uc,VTABLE+168,HOST+0x200); w64(uc,IMPL,OWNER)
 for i,v in enumerate(rect): f32(uc,RAW+i*4,v)
 calls=[]
 def hook(u,a,sz,data):
  if a==BMODEL+0x48b3f0: # memset
   ptr=u.reg_read(R.UC_ARM64_REG_X0); val=u.reg_read(R.UC_ARM64_REG_X1)&255; n=u.reg_read(R.UC_ARM64_REG_X2); u.mem_write(ptr,bytes([val])*n); u.reg_write(R.UC_ARM64_REG_X0,ptr); u.reg_write(R.UC_ARM64_REG_PC,u.reg_read(R.UC_ARM64_REG_LR))
  elif a==BMODEL+0x47d540: # String::CompareTo(char*) explicit bounded test-input comparator
   lit=cstr(u,u.reg_read(R.UC_ARM64_REG_X1)); calls.append(lit); u.reg_write(R.UC_ARM64_REG_W0,0 if lit==name else 1); u.reg_write(R.UC_ARM64_REG_PC,u.reg_read(R.UC_ARM64_REG_LR))
  elif a in (BMODEL+0x47dda0,BMODEL+0x47e1d0,BMODEL+0x47e1e0,BMODEL+0x47db80,BMODEL+0x47e1f0):
   u.reg_write(R.UC_ARM64_REG_W0,1 if a==BMODEL+0x47e1f0 else 0); u.reg_write(R.UC_ARM64_REG_PC,u.reg_read(R.UC_ARM64_REG_LR))
  elif a==BMODEL+0x47f0d0: u.reg_write(R.UC_ARM64_REG_PC,BBASE+0xb15c0)
  elif a==BMODEL+0x47e3d0: u.reg_write(R.UC_ARM64_REG_PC,BBASE+0xb1850)
  elif a==BBASE+0xe66b0: u.reg_write(R.UC_ARM64_REG_PC,BBASE+0xb0f10)
  elif a==BBASE+0xe7c50:
   angle=struct.unpack('<d',struct.pack('<Q',u.reg_read(R.UC_ARM64_REG_D0)))[0]
   u.mem_write(u.reg_read(R.UC_ARM64_REG_X0),struct.pack('<d',math.sin(angle)))
   u.mem_write(u.reg_read(R.UC_ARM64_REG_X1),struct.pack('<d',math.cos(angle)))
   u.reg_write(R.UC_ARM64_REG_PC,u.reg_read(R.UC_ARM64_REG_LR))
  elif a==BBASE+0xb0f10: # Base PointF::GetRotatedPoint called by rotated bound; rely on libc math via Python not hooked yet
   pass
  elif a==HOST+0x100: u.reg_write(R.UC_ARM64_REG_S0,struct.unpack('<I',u.mem_read(HEAP+0x5000,4))[0]); u.reg_write(R.UC_ARM64_REG_PC,u.reg_read(R.UC_ARM64_REG_LR))
  elif a==HOST+0x200:
   u.reg_write(R.UC_ARM64_REG_S0,u32(u,RAW)); u.reg_write(R.UC_ARM64_REG_S1,u32(u,RAW+4)); u.reg_write(R.UC_ARM64_REG_S2,u32(u,RAW+8)); u.reg_write(R.UC_ARM64_REG_S3,u32(u,RAW+12)); u.reg_write(R.UC_ARM64_REG_PC,u.reg_read(R.UC_ARM64_REG_LR))
 uc.hook_add(UC_HOOK_CODE,hook)
 # run native impl constructor, then setters
 call(uc,BMODEL+0x2e8788,[IMPL,OWNER])
 # supplied string receiver is only read through the explicitly stubbed CompareTo boundary
 call(uc,BMODEL+0x2ea3c0,[IMPL,HEAP+0x6000])
 category=struct.unpack('<I',uc.mem_read(IMPL+328,4))[0]
 f32(uc,HEAP+0x5000,rot)
 uc.reg_write(R.UC_ARM64_REG_S0,struct.unpack('<I',struct.pack('<f',size))[0]); call(uc,BMODEL+0x2ea300,[IMPL])
 out=call(uc,BMODEL+0x2e9488,[IMPL,0])
 out=vals(uc,[R.UC_ARM64_REG_S0,R.UC_ARM64_REG_S1,R.UC_ARM64_REG_S2,R.UC_ARM64_REG_S3])
 return {'name':name.rsplit('.',1)[-1], 'category':category,'size_f32_bits':struct.pack('<f',size).hex(),'rotation_f32_bits':struct.pack('<f',rot).hex(),'rect_f32_bits':[struct.pack('<f',v).hex() for v in out],'rect':out,'native_comparison_literals_seen':calls}

def run_formula_lists(source,answer):
 uc=Uc(UC_ARCH_ARM64,UC_MODE_ARM); uc.mem_map(BMODEL,0x600000); uc.mem_map(BBASE,0x200000); uc.mem_map(HEAP,0x10000); uc.mem_map(STACK,0x10000); uc.mem_map(HOST,0x1000)
 mapelf(uc,MODEL,BMODEL); mapelf(uc,BASE,BBASE); uc.reg_write(R.UC_ARM64_REG_TPIDR_EL0,TLS); w64(uc,TLS+40,0x1234)
 formula=HEAP+0x9000; src=formula+88; ans=formula+104; temp1=HEAP+0xa200; temp2=HEAP+0xa300; child_base=HEAP+0xc000; vt_base=HEAP+0xd000
 child_lists={src:source,ans:answer,temp1:[],temp2:[]}; raw_rects={}; visibility={}; next_temp={src:0,ans:0}; events=[]
 for listptr,children in ((src,source),(ans,answer)):
  w64(uc,listptr+8,HEAP+0xb000 if listptr==src else HEAP+0xb100)
 for ix,child in enumerate(source+answer):
  ptr=child_base+ix*0x100; vt=vt_base+ix*0x100; data=HEAP+0xe000+ix*0x100
  w64(uc,ptr,vt); w64(uc,vt+160,HOST+0x300); w64(uc,vt+168,HOST+0x400)
  for j,v in enumerate(child['rect']): f32(uc,data+j*4,v)
  w64(uc,ptr+8,data); raw_rects[ptr]=data; visibility[ptr]=child.get('visible',True)
  child['ptr']=ptr
 child_lists[src]=[x['ptr'] for x in source]; child_lists[ans]=[x['ptr'] for x in answer]
 count_map={}; cursor_map={}
 def host_return(u): u.reg_write(R.UC_ARM64_REG_PC,u.reg_read(R.UC_ARM64_REG_LR))
 def hook(u,a,sz,data):
  if a==BMODEL+0x489200: u.reg_write(R.UC_ARM64_REG_PC,BMODEL+0x4324a4)
  elif a==BMODEL+0x29f7b0:
   out=u.reg_read(R.UC_ARM64_REG_X0); orig=u.reg_read(R.UC_ARM64_REG_X1); n=next_temp.get(orig,0); next_temp[orig]=n+1
   tmp=temp1 if n%2==0 else temp2; children=child_lists.get(orig,[]); total=len(children)
   if n%2: children=[]
   child_lists[tmp]=children; count_map[tmp]=len(children); cursor_map[tmp]=0
   w64(u,out,tmp); w64(u,out+8,0); u.mem_write(out+16,struct.pack('<I',total if n%2 else 0))
   events.append({'detached_helper_pass':n,'source_list':hex(orig),'range_start':0 if n%2==0 else total,'range_end':total})
   host_return(u)
  elif a==BMODEL+0x47c110:
   p=u.reg_read(R.UC_ARM64_REG_X0); n=count_map.get(p,len(child_lists.get(p,[]))); events.append({'get_count':hex(p),'count':n}); u.reg_write(R.UC_ARM64_REG_W0,n); host_return(u)
  elif a==BMODEL+0x47c0f0:
   p=u.reg_read(R.UC_ARM64_REG_X0); i=cursor_map.get(p,0); rows=child_lists.get(p,[]); events.append({'get_data':hex(p),'cursor':i,'rows':len(rows)}); u.reg_write(R.UC_ARM64_REG_X0,rows[i] if i<len(rows) else 0); host_return(u)
  elif a==BMODEL+0x47c120:
   p=u.reg_read(R.UC_ARM64_REG_X0); i=cursor_map.get(p,0); events.append({'next':hex(p),'cursor':i}); cursor_map[p]=i+1; u.reg_write(R.UC_ARM64_REG_X0,i+1); host_return(u)
  elif a==BMODEL+0x47c190 or a==BMODEL+0x47ac10: host_return(u)
  elif a==BMODEL+0x47d9a0:
   p=u.reg_read(R.UC_ARM64_REG_X0); u.reg_write(R.UC_ARM64_REG_W0,int(visibility.get(p,False))); host_return(u)
  elif a==BMODEL+0x47aca0: u.reg_write(R.UC_ARM64_REG_PC,BMODEL+0x275630)
  elif a==HOST+0x300 or a==HOST+0x400:
   p=u.reg_read(R.UC_ARM64_REG_X0); dat=raw_rects[p]
   for j,r in enumerate((R.UC_ARM64_REG_S0,R.UC_ARM64_REG_S1,R.UC_ARM64_REG_S2,R.UC_ARM64_REG_S3)): u.reg_write(r,u32(u,dat+j*4))
   host_return(u)
 uc.hook_add(UC_HOOK_CODE,hook)
 try: call(uc,BMODEL+0x431e4c,[formula,1])
 except Exception as e: raise RuntimeError(f'formula lists failed PC={uc.reg_read(R.UC_ARM64_REG_PC):x} LR={uc.reg_read(R.UC_ARM64_REG_LR):x} regs={[hex(uc.reg_read(R.UC_ARM64_REG_X0+i)) for i in range(4)]} bytes={bytes(uc.mem_read(uc.reg_read(R.UC_ARM64_REG_PC),4)).hex()} events={events}') from e
 rect=vals(uc,[R.UC_ARM64_REG_S0,R.UC_ARM64_REG_S1,R.UC_ARM64_REG_S2,R.UC_ARM64_REG_S3])
 return {'source_count':len(source),'answer_count':len(answer),'source_visible':[x.get('visible',True) for x in source],'answer_visible':[x.get('visible',True) for x in answer],'rect':rect,'rect_f32_bits':[struct.pack('<f',v).hex() for v in rect],'helper_events':events}

def capture():
 names=['Marker','Beautify','Beautify2','unknown','Crayon','Crayon2','BrushPen','Pencil2','Pencil3','PatternPen','InkPen','ObliquePen','OilBrush3','ColoredPencil','TapePen']
 categories=[]
 for name in names:
  for rotation in (0.0,37.0):
   rect=[10.0,20.0,110.0,70.0]
   sample=run('com.samsung.android.sdk.pen.pen.preload.'+name,4.0,rotation,rect)
   categories.append({'pen_name':name,'category':sample['category'],'size_f32_bits':sample['size_f32_bits'],
    'rotation_f32_bits':sample['rotation_f32_bits'],'raw_rect_f32_bits':[struct.pack('<f',v).hex() for v in rect],
    'drawn_rect_f32_bits':sample['rect_f32_bits']})
 union_inputs=[
  ('one_source_visible',([{'rect':[2,12,118,78]}],[])),
  ('one_answer_visible',([],[{'rect':[3,13,117,77]}])),
  ('two_source_union',([{'rect':[2,12,118,78]},{'rect':[202,212,318,278]}],[])),
  ('source_hidden_answer_visible',([{'rect':[2,12,118,78],'visible':False}],[{'rect':[303,313,417,377]}])),
  ('source_visible_answer_hidden',([{'rect':[2,12,118,78]}],[{'rect':[303,313,417,377],'visible':False}])),
  ('both_empty',([],[])),
  ('both_hidden',([{'rect':[2,12,118,78],'visible':False}],[{'rect':[303,313,417,377],'visible':False}])),
 ]
 unions=[]
 for name,(source,answer) in union_inputs:
  actual=run_formula_lists(source,answer)
  unions.append({'name':name,'source_rects_f32_bits':[[struct.pack('<f',v).hex() for v in row['rect']] for row in source],
   'source_visible':[row.get('visible',True) for row in source],
   'answer_rects_f32_bits':[[struct.pack('<f',v).hex() for v in row['rect']] for row in answer],
   'answer_visible':[row.get('visible',True) for row in answer],
   'native_union_f32_bits':actual['rect_f32_bits']})
 return {'schema':'samsung-notes-formula-drawn-bounds-native-v1',
  'libraries':{'Model':sha(MODEL),'Base':sha(BASE)},
  'boundary':['Native Model ObjectStrokeImpl constructor 0x2e8788, SetPenType 0x2ea3c0, SetPenSize 0x2ea300 and GetDrawnRect 0x2e9488',
   'Native Model FormulaImpl GetRectByStrokeList 0x431e4c, getUnionStrokeListRect 0x4324a4 and UnionF 0x275630',
   'Native Base RectF::IncreaseRect 0xb15c0, GetRotatedBound 0xb1850 and PointF::GetRotatedPoint 0xb0f10',
   'Host String::CompareTo compares exact supplied pen name with native literals; owner refresh/raw-rect/rotation callbacks supply explicit state; Base sincos callback uses host libm',
   'Host HistoryManager detached-list and ObjectList iterator callbacks supply the explicitly listed children; child visibility and GetDrawnRect responses are supplied callbacks'],
  'stroke_rect_cases':categories,'formula_union_cases':unions}

def main():
 global MODEL,BASE
 parser=argparse.ArgumentParser(description=__doc__)
 group=parser.add_mutually_exclusive_group(required=True)
 group.add_argument('--write-fixture',action='store_true')
 group.add_argument('--check-fixture',action='store_true')
 parser.add_argument('--model',type=Path,default=MODEL)
 parser.add_argument('--base',type=Path,default=BASE)
 args=parser.parse_args(); MODEL=args.model; BASE=args.base
 encoded=json.dumps(capture(),indent=2,separators=(',',': '))+'\n'
 if args.write_fixture:
  FIXTURE.write_text(encoded); print(f'wrote {FIXTURE.relative_to(ROOT)}')
 elif FIXTURE.read_text()!=encoded:
  raise SystemExit('native Formula bounds capture differs from conformance/formula-drawn-bounds.json')
 else: print(f'verified {FIXTURE.relative_to(ROOT)}')

if __name__=='__main__': main()
