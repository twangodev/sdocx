#!/usr/bin/env python3
"""Replay finite-size controls against native Pencil3 V3 drawPoint.

Uses the project-local pinned Unicorn/AArch64 runtime. The native particle
method executes on a Drawing-created drawable; its RTV1 AddPoint import is
intercepted to capture arguments. No native buffer or pixel claim is made.
"""
import sys, pathlib, struct, json, contextlib, io, hashlib, ctypes, argparse
ROOT=pathlib.Path(__file__).resolve().parents[1]
FIXTURE=ROOT/'conformance/pencil3-v3-particle-controls-native.json'
BASE=ROOT/'conformance/pencil3_v3_native.py'
BASE_SHA256='4445fc15a87ee64c9a39c6fb55481328f5931e5a7b48d8da220189b633111405'
if hashlib.sha256(BASE.read_bytes()).hexdigest()!=BASE_SHA256:
 raise SystemExit('pinned Pencil V3 caller wrapper changed')
parser=argparse.ArgumentParser(description=__doc__)
parser.add_argument('--write-fixture',action='store_true')
args=parser.parse_args()
sys.path.insert(0,str(ROOT/'conformance'))
import pencil3_v3_native as base
capture, ns, _=base.run_native(1920,1080)
p=ns['p']; R=ns['R']
f=lambda x:struct.unpack('<f',struct.pack('<f',x))[0]
bits=lambda x:struct.unpack('<I',struct.pack('<f',x))[0]
libm=ctypes.CDLL('libm.so.6'); libm.fmaf.argtypes=[ctypes.c_float]*3; libm.fmaf.restype=ctypes.c_float
rows=[]
add_args=[]
def capture_addpoint():
 add_args.append({'s0_s4':[hex(p.u.reg_read(getattr(R,f'UC_ARM64_REG_S{i}'))) for i in range(5)]})
p.callbacks['SPen::Pencil3StrokeDrawableRTV1::AddPoint(SPen::Vector3<float>, float, float)']=capture_addpoint
# Small/large sizes and both sides of the size+1 == 5 min branch.
sizes=[0.,.1,.4,1.,3.999,4.,4.001,10.,36.2,799.,800.]
for i,size in enumerate(sizes):
 point=[f(12.125),f(-7.375)]; alpha=f(.37)
 ptr=p.alloc(64); p.u.mem_write(ptr,struct.pack('<ff',*point)+bytes(56)); rect=p.alloc(16)
 p.u.mem_write(ns['actual_drawable']+80,struct.pack('<Q',1))
 before=len(add_args); p.f(0,size); p.f(1,alpha)
 with contextlib.redirect_stdout(io.StringIO()):
  p.call('SPen::Pencil3StrokeDrawableGLV3::drawPoint(SPen::SmPoint*, float, float, SPen::RectF*)',[ns['actual_drawable'],ptr,rect])
 native_args=add_args[before:]; assert len(native_args)==1,native_args
 size1=f(f(size)+1); ran=libm.fmaf(size1,min(size1,5.),30.); divisor=int(ran)
 state=1; states=[]
 for _ in range(3):
  state=(state*0x343fd+0x269ec3)&((1<<64)-1); states.append(state)
 xy=[f(f(point[j]+f(((states[j]>>16)&0x7fffffff)%divisor/20.))-f(divisor/40.)) for j in range(2)]
 expected=[bits(xy[0]),bits(xy[1]),bits(float((state>>16)&15)),bits(size1),bits(alpha)]
 actual=[int(x,16) for x in native_args[0]['s0_s4']]
 assert actual==expected,(size,actual,expected)
 observed=struct.unpack('<Q',p.u.mem_read(ns['actual_drawable']+80,8))[0]; assert observed==state
 rows.append({'size':size,'point':point,'alpha':alpha,'range_f32_bits':hex(bits(ran)),'divisor':divisor,'native_addpoint_u32':actual,'native_final_state':hex(observed),'all_lanes_match':True})
report={'scope':'Direct native V3 drawPoint on Drawing-created object; RTV1 AddPoint intercepted at import boundary (no native buffer access); nonnegative finite size controls, private state reset to 1 by host per case. No saved sampler or pixel claim.','schema':'pencil3-v3-particle-controls-native-v1','runner_sha256':hashlib.sha256(pathlib.Path(__file__).read_bytes()).hexdigest(),'caller_wrapper_sha256':BASE_SHA256,'binary_sha256':capture['libraries'],'cases':rows}
encoded=json.dumps(report,indent=2,sort_keys=True)+'\n'
if args.write_fixture:
 FIXTURE.write_text(encoded)
 print('wrote '+str(FIXTURE.relative_to(ROOT)))
else:
 if FIXTURE.read_text()!=encoded: raise SystemExit('native V3 particle controls fixture mismatch')
 print('verified 11 native V3 particle controls at AddPoint import boundary')
