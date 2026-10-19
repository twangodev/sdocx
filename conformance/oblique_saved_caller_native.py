"""Bounded native Oblique caller and saved-redraw capture.

Run with the project research runtime:
  PYTHONPATH=scratch/apk-analysis-runtime/python python3 conformance/oblique_saved_caller_native.py --check-fixture
  PYTHONPATH=scratch/apk-analysis-runtime/python python3 conformance/oblique_saved_caller_native.py --check-profile-invariance --width 2560 --height 1600
"""
import sys,pathlib,hashlib,struct,subprocess,re,os,argparse,json
ROOT=pathlib.Path(__file__).resolve().parents[1]
os.chdir(ROOT)
sys.path.insert(0,str(ROOT/'conformance'))
import oblique_canvas_native as oc
import elf_plt
from unicorn import arm64_const as R
from unicorn import UC_HOOK_CODE
LIBS=ROOT/'scratch/apk-analysis-native/arm64-v8a'; MODEL=LIBS/'libSPenModel.so'; BASE=0x7000000; OBLIQUE_BASE=0x5000000
FIXTURE=ROOT/'conformance/oblique-saved-caller-native.json'
SELECTED_VERSION_FIXTURE=ROOT/'conformance/oblique-saved-caller-selected-native.json'
ADVANCED4_FIXTURE=ROOT/'conformance/oblique-saved-caller-advanced4-native.json'
REUSE_FIXTURE=ROOT/'conformance/oblique-saved-caller-reuse-native.json'
parser=argparse.ArgumentParser(description=__doc__)
mode=parser.add_mutually_exclusive_group(required=True)
mode.add_argument('--write-fixture',action='store_true')
mode.add_argument('--check-fixture',action='store_true')
mode.add_argument('--print-capture',action='store_true')
mode.add_argument('--check-profile-invariance',action='store_true')
parser.add_argument('--case',choices=('two_point_curve','two_point_near','two_point_far','two_point_offset','three_points_curve','three_points_no_curve','four_points_curve','alternating_terminal_curve','stress_six_curve','stress_six_linear','stress_gate_alternation','stress_tool2','stress_size4','stress_size8','stress_size16','stress_size56','two_point_below5','two_point_at5','two_point_above5','two_point_offset_at5','edge_linear_duplicates_sub5','edge_curve_duplicates_sub5','edge_both_axis_gate','edge_curve_exact5','edge_alternating_short_long','edge_tool1_same_mesh','edge_tool3_same_mesh','tool1_linear_same_mesh','tool1_curve_duplicates_sub5','tool1_curve_alternating_short_long','tool1_curve_49_999','tool1_curve_50_000','tool1_curve_50_001','tool1_linear_short_long','tool3_curve_short_long','tool3_linear_same_mesh','reuse_second','color_contract'),default='two_point_curve')
parser.add_argument('--redraw-version',choices=('selected','v4'),default='selected',help='use the selected drawable vtable entry, or the explicit GLV4 diagnostic probe')
parser.add_argument('--capture-transitions',action='store_true',help='include selected V1 redrawLine/endPen return snapshots in diagnostic output')
parser.add_argument('--advanced-setting',choices=('none','4'),default='none',help='apply saved ObjectStroke advanced setting before Drawing selects the drawable')
parser.add_argument('--omit-size',action='store_true',help='leave ObjectStroke pen size at its native constructor value')
parser.add_argument('--omit-color',action='store_true',help='leave ObjectStroke ARGB at its native constructor value')
parser.add_argument('--width',type=int,default=1920)
parser.add_argument('--height',type=int,default=1080)
parser.add_argument('--verbose',action='store_true',help='print native setup and instruction-boundary diagnostics')
parser.add_argument('--two-stroke',action='store_true',help='draw three_points_curve then the fixed short reuse_second input in the same Drawing session')
args=parser.parse_args()
import builtins
_console_print=builtins.print
def print(*values,**kwargs):
 always=kwargs.pop('_always',False)
 if args.verbose or always:_console_print(*values,**kwargs)
FIXTURE=SELECTED_VERSION_FIXTURE if args.redraw_version=='selected' else FIXTURE
if args.redraw_version=='selected' and args.advanced_setting=='4': FIXTURE=ADVANCED4_FIXTURE
if args.two_stroke or args.case=='reuse_second': FIXTURE=REUSE_FIXTURE
if args.case in {'two_point_below5','two_point_at5','two_point_above5','two_point_offset_at5'} and (args.redraw_version!='selected' or args.advanced_setting!='4'):
 raise SystemExit('threshold witnesses are scoped to the Drawing-selected V4 route with saved setting 4;')
INPUT_CASES={
 'two_point_curve': {'points':[[0.0,0.0],[2.0,0.0]],'size':1.0,'curve_enabled':True,'tool_type':0,'stroke_type':0,'pressure':[0.5,0.75],'timestamps':[100,110],'tilt':[0.1,0.2],'orientation':[0.0,0.3]},
 'two_point_near': {'points':[[0.0,0.0],[0.5,0.0]],'size':1.0,'curve_enabled':True,'tool_type':0,'stroke_type':0,'pressure':[0.5,0.75],'timestamps':[100,110],'tilt':[0.1,0.2],'orientation':[0.0,0.3]},
 'two_point_far': {'points':[[0.0,0.0],[20.0,0.0]],'size':1.0,'curve_enabled':True,'tool_type':0,'stroke_type':0,'pressure':[0.5,0.75],'timestamps':[100,110],'tilt':[0.1,0.2],'orientation':[0.0,0.3]},
 'two_point_offset': {'points':[[10.0,20.0],[12.0,20.0]],'size':1.0,'curve_enabled':True,'tool_type':0,'stroke_type':0,'pressure':[0.5,0.75],'timestamps':[100,110],'tilt':[0.1,0.2],'orientation':[0.0,0.3]},
 'two_point_below5': {'points':[[0.0,0.0],[4.999,0.0]],'size':1.0,'curve_enabled':True,'tool_type':0,'stroke_type':0,'pressure':[0.5,0.75],'timestamps':[100,110],'tilt':[0.1,0.2],'orientation':[0.0,0.3]},
 'two_point_at5': {'points':[[0.0,0.0],[5.0,0.0]],'size':1.0,'curve_enabled':True,'tool_type':0,'stroke_type':0,'pressure':[0.5,0.75],'timestamps':[100,110],'tilt':[0.1,0.2],'orientation':[0.0,0.3]},
 'two_point_above5': {'points':[[0.0,0.0],[5.001,0.0]],'size':1.0,'curve_enabled':True,'tool_type':0,'stroke_type':0,'pressure':[0.5,0.75],'timestamps':[100,110],'tilt':[0.1,0.2],'orientation':[0.0,0.3]},
 'two_point_offset_at5': {'points':[[10.0,20.0],[15.0,20.0]],'size':1.0,'curve_enabled':True,'tool_type':0,'stroke_type':0,'pressure':[0.5,0.75],'timestamps':[100,110],'tilt':[0.1,0.2],'orientation':[0.0,0.3]},
 'three_points_curve': {'points':[[10.0,20.0],[18.0,25.0],[26.0,20.0]],'size':4.0,'curve_enabled':True,'tool_type':0,'stroke_type':0,'pressure':[0.5,0.5,0.5],'timestamps':[0,8,16],'tilt':[0.0,0.0,0.0],'orientation':[0.0,0.0,0.0]},
 'three_points_no_curve': {'points':[[10.0,20.0],[18.0,25.0],[26.0,20.0]],'size':4.0,'curve_enabled':False,'tool_type':0,'stroke_type':0,'pressure':[0.5,0.5,0.5],'timestamps':[0,8,16],'tilt':[0.0,0.0,0.0],'orientation':[0.0,0.0,0.0]},
 'four_points_curve': {'points':[[10.0,20.0],[18.0,25.0],[26.0,20.0],[34.0,25.0]],'size':4.0,'curve_enabled':True,'tool_type':0,'stroke_type':0,'pressure':[0.5,0.5,0.5,0.5],'timestamps':[0,8,16,24],'tilt':[0.0,0.0,0.0,0.0],'orientation':[0.0,0.0,0.0,0.0]},
 'alternating_terminal_curve': {'points':[[0.0,0.0],[10.0,0.0],[12.0,0.0]],'size':1.0,'curve_enabled':True,'tool_type':0,'stroke_type':0,'pressure':[0.5,0.5,0.5],'timestamps':[0,10,20],'tilt':[0.0,0.0,0.0],'orientation':[0.0,0.0,0.0]},
 'stress_six_curve': {'points':[[10.0,20.0],[18.0,25.0],[26.0,20.0],[34.0,25.0],[42.0,20.0],[50.0,25.0]],'size':4.0,'curve_enabled':True,'tool_type':0,'stroke_type':0,'pressure':[0.5]*6,'timestamps':[0,8,16,24,32,40],'tilt':[0.0]*6,'orientation':[0.0]*6},
 'stress_six_linear': {'points':[[10.0,20.0],[18.0,25.0],[26.0,20.0],[34.0,25.0],[42.0,20.0],[50.0,25.0]],'size':4.0,'curve_enabled':False,'tool_type':0,'stroke_type':0,'pressure':[0.5]*6,'timestamps':[0,8,16,24,32,40],'tilt':[0.0]*6,'orientation':[0.0]*6},
 'stress_gate_alternation': {'points':[[0.0,0.0],[1.25,1.25],[2.25,2.25],[3.25,3.25],[4.25,4.25],[11.0,11.0],[11.75,11.75],[12.5,12.5],[22.0,22.0]],'size':1.0,'curve_enabled':True,'tool_type':0,'stroke_type':0,'pressure':[0.5]*9,'timestamps':[0,8,16,24,32,40,48,56,64],'tilt':[0.0]*9,'orientation':[0.0]*9},
 'stress_tool2': {'points':[[10.0,20.0],[18.0,25.0],[26.0,20.0],[34.0,25.0]],'size':4.0,'curve_enabled':True,'tool_type':2,'stroke_type':0,'pressure':[0.5]*4,'timestamps':[0,8,16,24],'tilt':[0.0]*4,'orientation':[0.0]*4},
 'stress_size4': {'points':[[0.0,0.0],[100.0,0.0]],'size':4.0,'curve_enabled':True,'tool_type':0,'stroke_type':0,'pressure':[0.5,0.5],'timestamps':[0,10],'tilt':[0.0,0.0],'orientation':[0.0,0.0]},
 'stress_size8': {'points':[[0.0,0.0],[100.0,0.0]],'size':8.0,'curve_enabled':True,'tool_type':0,'stroke_type':0,'pressure':[0.5,0.5],'timestamps':[0,10],'tilt':[0.0,0.0],'orientation':[0.0,0.0]},
 'stress_size16': {'points':[[0.0,0.0],[100.0,0.0]],'size':16.0,'curve_enabled':True,'tool_type':0,'stroke_type':0,'pressure':[0.5,0.5],'timestamps':[0,10],'tilt':[0.0,0.0],'orientation':[0.0,0.0]},
 'stress_size56': {'points':[[0.0,0.0],[100.0,0.0]],'size':56.0,'curve_enabled':True,'tool_type':0,'stroke_type':0,'pressure':[0.5,0.5],'timestamps':[0,10],'tilt':[0.0,0.0],'orientation':[0.0,0.0]},
 'edge_linear_duplicates_sub5': {'points':[[10.0,20.0],[10.0,20.0],[11.25,21.25],[12.5,22.5],[14.0,22.5],[18.0,22.5]],'size':4.0,'curve_enabled':False,'tool_type':0,'stroke_type':0,'pressure':[0.5]*6,'timestamps':[0,8,16,24,32,40],'tilt':[0.0]*6,'orientation':[0.0]*6},
 'edge_curve_duplicates_sub5': {'points':[[10.0,20.0],[10.0,20.0],[11.25,21.25],[12.5,22.5],[14.0,22.5],[18.0,22.5]],'size':4.0,'curve_enabled':True,'tool_type':0,'stroke_type':0,'pressure':[0.5]*6,'timestamps':[0,8,16,24,32,40],'tilt':[0.0]*6,'orientation':[0.0]*6},
 'edge_both_axis_gate': {'points':[[0.0,0.0],[1.49,1.49],[2.99,2.99],[4.49,4.49],[10.0,10.0]],'size':1.0,'curve_enabled':True,'tool_type':0,'stroke_type':0,'pressure':[0.5]*5,'timestamps':[0,8,16,24,32],'tilt':[0.0]*5,'orientation':[0.0]*5},
 'edge_curve_exact5': {'points':[[10.0,20.0],[15.0,20.0]],'size':1.0,'curve_enabled':True,'tool_type':0,'stroke_type':0,'pressure':[0.5,0.5],'timestamps':[0,10],'tilt':[0.0,0.0],'orientation':[0.0,0.0]},
 'edge_alternating_short_long': {'points':[[0.0,0.0],[8.0,0.0],[9.0,0.0],[17.0,0.0],[18.0,0.0],[26.0,0.0]],'size':1.0,'curve_enabled':True,'tool_type':0,'stroke_type':0,'pressure':[0.5]*6,'timestamps':[0,8,16,24,32,40],'tilt':[0.0]*6,'orientation':[0.0]*6},
 'edge_tool1_same_mesh': {'points':[[10.0,20.0],[18.0,25.0],[26.0,20.0],[34.0,25.0]],'size':4.0,'curve_enabled':True,'tool_type':1,'stroke_type':0,'pressure':[0.5]*4,'timestamps':[0,8,16,24],'tilt':[0.0]*4,'orientation':[0.0]*4},
 'edge_tool3_same_mesh': {'points':[[10.0,20.0],[18.0,25.0],[26.0,20.0],[34.0,25.0]],'size':4.0,'curve_enabled':True,'tool_type':3,'stroke_type':0,'pressure':[0.5]*4,'timestamps':[0,8,16,24],'tilt':[0.0]*4,'orientation':[0.0]*4},
 'tool1_linear_same_mesh': {'points':[[10.0,20.0],[18.0,25.0],[26.0,20.0],[34.0,25.0]],'size':4.0,'curve_enabled':False,'tool_type':1,'stroke_type':0,'pressure':[0.5]*4,'timestamps':[0,8,16,24],'tilt':[0.0]*4,'orientation':[0.0]*4},
 'tool1_curve_duplicates_sub5': {'points':[[10.0,20.0],[10.0,20.0],[11.25,21.25],[12.5,22.5],[14.0,22.5],[18.0,22.5]],'size':4.0,'curve_enabled':True,'tool_type':1,'stroke_type':0,'pressure':[0.5]*6,'timestamps':[0,8,16,24,32,40],'tilt':[0.0]*6,'orientation':[0.0]*6},
 'tool1_curve_alternating_short_long': {'points':[[0.0,0.0],[60.0,0.0],[61.0,0.0],[121.0,0.0],[122.0,0.0],[182.0,0.0]],'size':4.0,'curve_enabled':True,'tool_type':1,'stroke_type':0,'pressure':[0.5]*6,'timestamps':[0,8,16,24,32,40],'tilt':[0.0]*6,'orientation':[0.0]*6},
 'tool1_curve_49_999': {'points':[[0.0,0.0],[49.999,0.0]],'size':4.0,'curve_enabled':True,'tool_type':1,'stroke_type':0,'pressure':[0.5,0.5],'timestamps':[0,10],'tilt':[0.0,0.0],'orientation':[0.0,0.0]},
 'tool1_curve_50_000': {'points':[[0.0,0.0],[50.0,0.0]],'size':4.0,'curve_enabled':True,'tool_type':1,'stroke_type':0,'pressure':[0.5,0.5],'timestamps':[0,10],'tilt':[0.0,0.0],'orientation':[0.0,0.0]},
 'tool1_curve_50_001': {'points':[[0.0,0.0],[50.001,0.0]],'size':4.0,'curve_enabled':True,'tool_type':1,'stroke_type':0,'pressure':[0.5,0.5],'timestamps':[0,10],'tilt':[0.0,0.0],'orientation':[0.0,0.0]},
 'tool1_linear_short_long': {'points':[[0.0,0.0],[60.0,0.0],[61.0,0.0],[121.0,0.0],[122.0,0.0],[182.0,0.0]],'size':4.0,'curve_enabled':False,'tool_type':1,'stroke_type':0,'pressure':[0.5]*6,'timestamps':[0,8,16,24,32,40],'tilt':[0.0]*6,'orientation':[0.0]*6},
 'tool3_curve_short_long': {'points':[[0.0,0.0],[60.0,0.0],[61.0,0.0],[121.0,0.0],[122.0,0.0],[182.0,0.0]],'size':4.0,'curve_enabled':True,'tool_type':3,'stroke_type':0,'pressure':[0.5]*6,'timestamps':[0,8,16,24,32,40],'tilt':[0.0]*6,'orientation':[0.0]*6},
 'tool3_linear_same_mesh': {'points':[[10.0,20.0],[18.0,25.0],[26.0,20.0],[34.0,25.0]],'size':4.0,'curve_enabled':False,'tool_type':3,'stroke_type':0,'pressure':[0.5]*4,'timestamps':[0,8,16,24],'tilt':[0.0]*4,'orientation':[0.0]*4},
 'tool1_linear_same_mesh': {'points':[[10.0,20.0],[18.0,25.0],[26.0,20.0],[34.0,25.0]],'size':4.0,'curve_enabled':False,'tool_type':1,'stroke_type':0,'pressure':[0.5]*4,'timestamps':[0,8,16,24],'tilt':[0.0]*4,'orientation':[0.0]*4},
 'tool1_curve_duplicates_sub5': {'points':[[10.0,20.0],[10.0,20.0],[11.25,21.25],[12.5,22.5],[14.0,22.5],[18.0,22.5]],'size':4.0,'curve_enabled':True,'tool_type':1,'stroke_type':0,'pressure':[0.5]*6,'timestamps':[0,8,16,24,32,40],'tilt':[0.0]*6,'orientation':[0.0]*6},
 'tool1_curve_alternating_short_long': {'points':[[0.0,0.0],[60.0,0.0],[61.0,0.0],[121.0,0.0],[122.0,0.0],[182.0,0.0]],'size':4.0,'curve_enabled':True,'tool_type':1,'stroke_type':0,'pressure':[0.5]*6,'timestamps':[0,8,16,24,32,40],'tilt':[0.0]*6,'orientation':[0.0]*6},
 'tool1_curve_49_999': {'points':[[0.0,0.0],[49.999,0.0]],'size':4.0,'curve_enabled':True,'tool_type':1,'stroke_type':0,'pressure':[0.5,0.5],'timestamps':[0,10],'tilt':[0.0,0.0],'orientation':[0.0,0.0]},
 'tool1_curve_50_000': {'points':[[0.0,0.0],[50.0,0.0]],'size':4.0,'curve_enabled':True,'tool_type':1,'stroke_type':0,'pressure':[0.5,0.5],'timestamps':[0,10],'tilt':[0.0,0.0],'orientation':[0.0,0.0]},
 'tool1_curve_50_001': {'points':[[0.0,0.0],[50.001,0.0]],'size':4.0,'curve_enabled':True,'tool_type':1,'stroke_type':0,'pressure':[0.5,0.5],'timestamps':[0,10],'tilt':[0.0,0.0],'orientation':[0.0,0.0]},
 'tool1_linear_short_long': {'points':[[0.0,0.0],[60.0,0.0],[61.0,0.0],[121.0,0.0],[122.0,0.0],[182.0,0.0]],'size':4.0,'curve_enabled':False,'tool_type':1,'stroke_type':0,'pressure':[0.5]*6,'timestamps':[0,8,16,24,32,40],'tilt':[0.0]*6,'orientation':[0.0]*6},
 'tool3_curve_short_long': {'points':[[0.0,0.0],[60.0,0.0],[61.0,0.0],[121.0,0.0],[122.0,0.0],[182.0,0.0]],'size':4.0,'curve_enabled':True,'tool_type':3,'stroke_type':0,'pressure':[0.5]*6,'timestamps':[0,8,16,24,32,40],'tilt':[0.0]*6,'orientation':[0.0]*6},
 'tool3_linear_same_mesh': {'points':[[10.0,20.0],[18.0,25.0],[26.0,20.0],[34.0,25.0]],'size':4.0,'curve_enabled':False,'tool_type':3,'stroke_type':0,'pressure':[0.5]*4,'timestamps':[0,8,16,24],'tilt':[0.0]*4,'orientation':[0.0]*4},
 'reuse_second': {'points':[[30.0,2.0],[31.25,2.5]],'size':4.25,'curve_enabled':True,'tool_type':0,'stroke_type':0,'pressure':[0.6,0.8],'timestamps':[33,43],'tilt':[0.1,-0.2],'orientation':[0.3,0.5]},
 'color_contract': {'points':[[0.0,0.0],[2.0,0.0]],'size':1.0,'curve_enabled':True,'tool_type':0,'stroke_type':0,'pressure':[0.5,0.75],'timestamps':[100,110],'tilt':[0.1,0.2],'orientation':[0.0,0.3]},
}
input_case=INPUT_CASES[args.case]
if args.two_stroke and (args.case!='three_points_curve' or args.redraw_version!='selected' or args.advanced_setting!='4'):
 raise SystemExit('--two-stroke requires --case three_points_curve --redraw-version selected --advanced-setting 4')
if not (0 < args.width < 65536 and 0 < args.height < 65536): raise SystemExit('profile dimensions must be positive u16 values')
def mov_w0_ret(value):
 if not 0 <= value <= 0xffff: raise ValueError('screen dimension exceeds MOVZ immediate')
 return struct.pack('<II',0x52800000|(value<<5),0xd65f03c0)
def call_address(address, args, limit=5_000_000):
 for index,value in enumerate(args):p.x(index,value)
 p.u.reg_write(R.UC_ARM64_REG_SP,0x200EF000);p.u.reg_write(R.UC_ARM64_REG_LR,oc.STOP)
 p.u.emu_start(address,oc.STOP,count=limit)
 if p.u.reg_read(R.UC_ARM64_REG_PC)!=oc.STOP:raise RuntimeError(f'address call {address:#x} did not return')
p=oc.CanvasProbe()
# The shared canvas probe installs unsupported/invisible ObjectBase defaults
# to suppress paint in its matrix-only test. This caller must use Model's
# pinned native type/visibility methods: ObjectStroke is explicitly visible.
for inherited in ('SPen::ObjectBase::GetType() const','SPen::ObjectBase::IsVisible() const'):
 p.callbacks.pop(inherited,None)
def trace(u,a,z,d):
 if a==0x2000000+0xc3d88: print('STACKFAIL guard',hex(u.reg_read(R.UC_ARM64_REG_TPIDR_EL0)),u.mem_read(0x20000000+40,8).hex(),'x20',hex(u.reg_read(R.UC_ARM64_REG_X20)),'x8',u.mem_read(u.reg_read(R.UC_ARM64_REG_X29)-8,8).hex())
p.u.hook_add(UC_HOOK_CODE,trace)
raw=MODEL.read_bytes(); digest=hashlib.sha256(raw).hexdigest(); assert digest=='4fbcf6d4213e929f1535d32abb487743643fd5d0dfc366e50dfeb2e7d8015b7a'
phoff=struct.unpack_from('<Q',raw,32)[0]; ent,num=struct.unpack_from('<HH',raw,54); extent=0
for i in range(num):
 typ,fl,off,va,pa,fs,ms,al=struct.unpack_from('<II6Q',raw,phoff+i*ent)
 if typ==1:extent=max(extent,va+ms)
p.u.mem_map(BASE,(extent+0xfff)&~0xfff)
for i in range(num):
 typ,fl,off,va,pa,fs,ms,al=struct.unpack_from('<II6Q',raw,phoff+i*ent)
 if typ==1 and ms:
  p.u.mem_write(BASE+va,raw[off:off+fs])
  if ms>fs:p.u.mem_write(BASE+va+fs,bytes(ms-fs))
env=dict(os.environ); env['LD_LIBRARY_PATH']=str((pathlib.Path('scratch/apk-analysis-runtime/binutils-aarch64/usr/lib/x86_64-linux-gnu')).resolve())
for line in subprocess.check_output(['nm','-D','-C',str(MODEL)],text=True,env=env).splitlines():
 parts=line.strip().split(' ',2)
 if len(parts)==3 and parts[1] in ('T','W'):p.syms[parts[2]]=BASE+int(parts[0],16)
dis=subprocess.check_output([str(oc.OBJDUMP),'-d','-C','-j','.plt',str(MODEL)],text=True,env=env)
for addr,name in re.findall(r'^([0-9a-f]+) <(.+)@plt>:',dis,re.M):p.plt[BASE+int(addr,16)]=name
# Map the pinned Android libc++ runtime needed by Base String/locale methods.
CPP=LIBS/'libc++_shared.so'; CPP_BASE=0x8000000; cb=CPP.read_bytes(); assert hashlib.sha256(cb).hexdigest()=='4397241b4bd20a8e579bfb41d21107857e12985f6a01ca0c2a5f83380d1270b4'
coff=struct.unpack_from('<Q',cb,32)[0]; cent,cnum=struct.unpack_from('<HH',cb,54); cextent=0
for i in range(cnum):
 typ,fl,off,va,pa,fs,ms,al=struct.unpack_from('<II6Q',cb,coff+i*cent)
 if typ==1:cextent=max(cextent,va+ms)
p.u.mem_map(CPP_BASE,(cextent+0xfff)&~0xfff)
for i in range(cnum):
 typ,fl,off,va,pa,fs,ms,al=struct.unpack_from('<II6Q',cb,coff+i*cent)
 if typ==1 and ms:
  p.u.mem_write(CPP_BASE+va,cb[off:off+fs])
  if ms>fs:p.u.mem_write(CPP_BASE+va+fs,bytes(ms-fs))
for line in subprocess.check_output(['nm','-D','-C',str(CPP)],text=True,env=env).splitlines():
 parts=line.strip().split(' ',2)
 if len(parts)==3 and parts[1] in ('T','W'):p.syms[parts[2]]=CPP_BASE+int(parts[0],16)
cdis=subprocess.check_output([str(oc.OBJDUMP),'-d','-C','-j','.plt',str(CPP)],text=True,env=env)
for addr,name in re.findall(r'^([0-9a-f]+) <(.+)@plt>:',cdis,re.M):p.plt[CPP_BASE+int(addr,16)]=name
# Native loader relocation for Model references; code imports continue through bounded PLT resolution.
shoff=struct.unpack_from('<Q',raw,40)[0]; se,sn,ss=struct.unpack_from('<HHH',raw,58); secs=[struct.unpack_from('<IIQQQQIIQQ',raw,shoff+i*se) for i in range(sn)]; names=raw[secs[ss][4]:secs[ss][4]+secs[ss][5]]; by={names[s[0]:names.find(b'\0',s[0])].decode():s for s in secs}; ds,st=by['.dynsym'],by['.dynstr']; strings=raw[st[4]:st[4]+st[5]]; symbols=[]
for off in range(ds[4],ds[4]+ds[5],ds[9]):
 no,info,other,shndx,val,size=struct.unpack_from('<IBBHQQ',raw,off); symbols.append((strings[no:strings.find(b'\0',no)].decode() if no else '',val))
for secname in ('.rela.dyn','.rela.plt'):
 sec=by.get(secname)
 if not sec:continue
 for off in range(sec[4],sec[4]+sec[5],sec[9]):
  where,info,add=struct.unpack_from('<QQq',raw,off); typ,idx=info&0xffffffff,info>>32
  if typ==0x403:p.u.mem_write(BASE+where,struct.pack('<Q',BASE+add))
  elif typ in (0x402,0x401,0x1025,0x101):
   name,val=symbols[idx]; target=BASE+val if val else p.syms.get(name,0)
   if target:p.u.mem_write(BASE+where,struct.pack('<Q',target+add if typ in (0x401,0x101) else target))

# Rebase dynamic relocations for mapped DSOs used by the actual constructors.
paths={
 'FountainPen':LIBS/'libSPenFountainPen.so','PenCommon':LIBS/'libSPenPenCommon.so',
 'Base':LIBS/'libSPenBase.so','Drawing':LIBS/'libSPenDrawing.so',
 'Graphics':LIBS/'libSPenGraphics.so','ObliquePen':LIBS/'libSPenObliquePen.so',
 'Composer':LIBS/'libSPenComposer.so','Model':MODEL}
bases={'FountainPen':0,'PenCommon':0x1000000,'Base':0x2000000,'Drawing':0x3000000,
 'Graphics':0x4000000,'ObliquePen':0x5000000,'Composer':0x6000000,'Model':BASE}
rawsyms={}
for lib,path in paths.items():
 for line in subprocess.check_output(['nm','-D',str(path)],text=True,env=env).splitlines():
  parts=line.strip().split()
  if len(parts)==3 and parts[1].upper() in ('T','W','D','B','R','V'): rawsyms.setdefault(parts[2],bases[lib]+int(parts[0],16))
for lib,path in paths.items():
 b=path.read_bytes(); base=bases[lib]; shoff=struct.unpack_from('<Q',b,40)[0]; se,sn,ss=struct.unpack_from('<HHH',b,58); secs=[struct.unpack_from('<IIQQQQIIQQ',b,shoff+i*se) for i in range(sn)]; ns=b[secs[ss][4]:secs[ss][4]+secs[ss][5]]; by={ns[x[0]:ns.find(b'\0',x[0])].decode():x for x in secs}; ds,st=by['.dynsym'],by['.dynstr']; strings=b[st[4]:st[4]+st[5]]; sy=[]
 for off in range(ds[4],ds[4]+ds[5],ds[9]):
  no,info,other,ndx,val,size=struct.unpack_from('<IBBHQQ',b,off); sy.append((strings[no:strings.find(b'\0',no)].decode() if no else '',val))
 for snm in ('.rela.dyn','.rela.plt'):
  sec=by.get(snm)
  if not sec:continue
  for off in range(sec[4],sec[4]+sec[5],sec[9]):
   where,info,add=struct.unpack_from('<QQq',b,off); typ,idx=info&0xffffffff,info>>32
   if typ==0x403:p.u.mem_write(base+where,struct.pack('<Q',base+add))
   elif typ in (0x401,0x402,0x403,0x101,0x1025,0x1026):
    name,val=sy[idx]; target=base+val if val else rawsyms.get(name,0)
    if not target:target=p.syms.get(name,0)
    if target:p.u.mem_write(base+where,struct.pack('<Q',target+(add if typ in (0x401,0x101) else 0)))
    elif lib=='ObliquePen' and where in (0x6c278,0x6c280):print('unresolved',hex(where),name,hex(val),hex(typ))
    if lib=='ObliquePen' and where in (0x6c278,0x6c280):print('reloc',hex(where),name,hex(target),hex(base+where),p.u.mem_read(base+where,8).hex())

def bind_exact_plt(path,base):
 for offset,raw_symbol in elf_plt.resolve(path).items():
  symbol=subprocess.check_output(['c++filt','-n',raw_symbol],text=True).strip() if raw_symbol.startswith('_Z') else raw_symbol
  p.plt[base+offset]=symbol
for _lib,_path in paths.items(): bind_exact_plt(_path,bases[_lib])
bind_exact_plt(CPP,CPP_BASE)

for name in ['std::__ndk1::recursive_mutex::lock()','std::__ndk1::recursive_mutex::unlock()','std::__ndk1::recursive_mutex::recursive_mutex()','std::__ndk1::recursive_mutex::~recursive_mutex()','std::__ndk1::mutex::lock()','std::__ndk1::mutex::unlock()','std::__ndk1::mutex::mutex()','std::__ndk1::mutex::~mutex()']: p.callbacks[name]=lambda:None
p.callbacks['operator new(unsigned long, std::nothrow_t const&)']=lambda:p.x(0,p.alloc(p.x(0)))
p.callbacks['operator new[](unsigned long, std::nothrow_t const&)']=lambda:p.x(0,p.alloc(p.x(0)))
p.callbacks['strlen']=lambda:p.x(0,len(bytes(p.u.mem_read(p.x(0),256)).split(b'\0',1)[0]))
p.callbacks['atoi']=lambda:p.x(0,int(bytes(p.u.mem_read(p.x(0),64)).split(b'\0',1)[0] or b'0'))
# Native Base defines the MotionEvent constructor and every accessor used by
# RedrawPen. fountain_native normally substitutes endpoint properties; remove
# that whole surface so this saved caller reads the event built from ObjectStroke.
for inherited in [name for name in p.callbacks if name.startswith('SPen::MotionEvent::')]:
 p.callbacks.pop(inherited)
p._newlog=[]
def native_new():
 size=p.x(0); out=p.alloc(size); p._newlog.append((out,size)); p.x(0,out)
p.callbacks['operator new(unsigned long)']=native_new
p.callbacks['SPen::PenGLDataManager::GetMsgQueue()']=lambda:(print('GMSGQ',hex(p.x(0)),hex(queue)),p.x(0,queue)); p.callbacks['__cxa_atexit']=lambda:p.x(0,0)
loader_calls=[]; DSO_HANDLE=0x50000001
def read_cstr(addr): return bytes(p.u.mem_read(addr,256)).split(bytes([0]),1)[0].decode('ascii','replace')
def host_dlopen():
 name=read_cstr(p.u.reg_read(R.UC_ARM64_REG_X0)); flags=p.u.reg_read(R.UC_ARM64_REG_X1); loader_calls.append({'fn':'dlopen','name':name,'flags':flags});
 if name!='libSPenObliquePen.so': raise RuntimeError(f'unapproved dlopen {name!r}')
 p.u.reg_write(R.UC_ARM64_REG_X0,DSO_HANDLE)
def host_dlsym():
 handle=p.u.reg_read(R.UC_ARM64_REG_X0); name=read_cstr(p.u.reg_read(R.UC_ARM64_REG_X1)); loader_calls.append({'fn':'dlsym','handle':hex(handle),'name':name})
 if handle!=DSO_HANDLE: raise RuntimeError(f'unknown dso handle {handle:#x}')
 result=p.syms.get(name,0)
 if not result: raise RuntimeError(f'unresolved dlsym {name!r}')
 p.u.reg_write(R.UC_ARM64_REG_X0,result)
p.callbacks['dlopen']=host_dlopen; p.callbacks['dlsym']=host_dlsym; p.callbacks['dlerror']=lambda:p.x(0,0)

def strncmp():
 a,b,n=p.x(0),p.x(1),p.x(2);x=bytes(p.u.mem_read(a,n));y=bytes(p.u.mem_read(b,n));out=0
 for aa,bb in zip(x,y):
  if aa!=bb:out=aa-bb;break
  if aa==0:break
 p.x(0,out)
p.callbacks['strncmp']=strncmp
p.callbacks['getauxval']=lambda:p.x(0,{6:4096,16:256,26:0}.get(p.x(0),0))
p.callbacks['__system_property_get']=lambda:(p.u.mem_write(p.x(1),b'arm64-v8a0'),p.x(0,9))
p.callbacks['__cxa_thread_atexit']=lambda:p.x(0,0)
p._emutls={}
def emutls():
 key=p.x(0); size,align,existing,template=struct.unpack('<QQQQ',p.u.mem_read(key,32))
 if not 0<size<=0x100000 or not 0<align<=0x10000 or align&(align-1): raise RuntimeError(f'invalid emutls descriptor {size} {align}')
 template_bytes=bytes(p.u.mem_read(template,size)) if template else bytes(size)
 value=p._emutls.get(key)
 if value is None:
  if existing: raise RuntimeError(f'unowned preexisting emutls pointer {existing:#x}')
  rawp=p.alloc(size+align); value=(rawp+align-1)&-align; p.u.mem_write(value,template_bytes); p.u.mem_write(key+16,struct.pack('<Q',value)); p._emutls[key]=value
 p.x(0,value)
p.callbacks['__emutls_get_address']=emutls
p._guards=set()
def guard_acquire():
 key=p.x(0); value=struct.unpack('<Q',p.u.mem_read(key,8))[0]
 p.x(0,int(not value&1 and key not in p._guards));
 if p.x(0):p._guards.add(key)
def guard_release():
 key=p.x(0)
 if key not in p._guards:raise RuntimeError(f'guard release without acquire {key:#x}')
 p._guards.remove(key);p.u.mem_write(key,b'\1')
def guard_abort():p._guards.discard(p.x(0))
p.callbacks['__cxa_guard_acquire']=guard_acquire;p.callbacks['__cxa_guard_release']=guard_release;p.callbacks['__cxa_guard_abort']=guard_abort
def host_malloc():
 n=p.u.reg_read(R.UC_ARM64_REG_X0)
 if n>0x1000000: raise RuntimeError(f'malloc bound {n:#x}')
 p.u.reg_write(R.UC_ARM64_REG_X0,p.alloc(max(n,1)))
def host_calloc():
 n=p.u.reg_read(R.UC_ARM64_REG_X0)*p.u.reg_read(R.UC_ARM64_REG_X1)
 if n>0x1000000: raise RuntimeError(f'calloc bound {n:#x}')
 p.u.reg_write(R.UC_ARM64_REG_X0,p.alloc(max(n,1)))
for name in ['malloc','__libc_malloc']:p.callbacks[name]=host_malloc
p.callbacks['calloc']=host_calloc
for name in ['free','__libc_free']:p.callbacks[name]=lambda:p.x(0,0)
p.callbacks['realloc']=lambda:p.x(0,p.alloc(p.x(1)))
for name in ['pthread_mutex_init','pthread_mutexattr_init','pthread_mutexattr_destroy','pthread_mutexattr_settype','pthread_mutex_destroy','pthread_mutex_lock','pthread_mutex_unlock','pthread_cond_init','pthread_cond_destroy','pthread_cond_wait','pthread_cond_signal','pthread_cond_broadcast']:
 p.callbacks[name]=lambda:p.x(0,0)
# Single-thread pthread_once with actual init callback execution thunk.
ONCE=0x200f2080
p.u.mem_write(ONCE,bytes.fromhex('080040f9a80000b5fd7bbfa9fd030091280080d2080000f920003fd6fd7bc1a8000080d2c0035fd6'))
p.syms['pthread_once']=ONCE
p._newlog=[]
def native_new():
 size=p.x(0); out=p.alloc(size); p._newlog.append((out,size)); p.x(0,out)
p.callbacks['operator new(unsigned long)']=native_new
# Run every mapped non-Composer dependency initializer; any failure aborts the probe.
def run_init(path,base):
 b=path.read_bytes(); sh=struct.unpack_from('<Q',b,40)[0]; es,n,si=struct.unpack_from('<HHH',b,58); secs=[struct.unpack_from('<IIQQQQIIQQ',b,sh+i*es) for i in range(n)]; names=b[secs[si][4]:secs[si][4]+secs[si][5]]; sec=next(x for x in secs if names[x[0]:names.find(b'\0',x[0])]==b'.init_array');
 for i in range(0,sec[5],8):
  target=struct.unpack('<Q',p.u.mem_read(base+sec[3]+i,8))[0]
  if not target:continue
  for j in range(4):p.x(j,0)
  p.u.reg_write(R.UC_ARM64_REG_SP,0x200ef000);p.u.reg_write(R.UC_ARM64_REG_LR,oc.STOP)
  try:p.u.emu_start(target,oc.STOP,count=3000000)
  except Exception as e: print('INIT EXC',path.name,hex(target),'pc',hex(p.u.reg_read(R.UC_ARM64_REG_PC)),'lr',hex(p.u.reg_read(R.UC_ARM64_REG_LR)),type(e).__name__,e); raise
  if p.u.reg_read(R.UC_ARM64_REG_PC)!=oc.STOP:raise RuntimeError(f'init failed {path.name} {target:#x} PC={p.u.reg_read(R.UC_ARM64_REG_PC):#x}')
print('symadvanced',p.syms.get('SPen::Pen::SetAdvancedSetting(SPen::String const*)'))
for path,base in [(CPP,CPP_BASE),(LIBS/'libSPenBase.so',0x2000000),(LIBS/'libSPenPenCommon.so',0x1000000),(MODEL,BASE),(LIBS/'libSPenObliquePen.so',0x5000000),(LIBS/'libSPenDrawing.so',0x3000000),(LIBS/'libSPenGraphics.so',0x4000000)]:run_init(path,base)
def emulate_lse(u,a,z,d):
 if a==0x2098c60:
  x1=u.reg_read(R.UC_ARM64_REG_X1); old=struct.unpack('<I',u.mem_read(x1,4))[0]; new=(old+u.reg_read(R.UC_ARM64_REG_W0))&0xffffffff; u.mem_write(x1,struct.pack('<I',new)); u.reg_write(R.UC_ARM64_REG_W0,old); u.reg_write(R.UC_ARM64_REG_PC,a+4)
 if a in (0x104caf0,0x104c400):
  x1=u.reg_read(R.UC_ARM64_REG_X1); old=struct.unpack('<Q',u.mem_read(x1,8))[0]; u.mem_write(x1,struct.pack('<Q',(old+u.reg_read(R.UC_ARM64_REG_X0))&0xffffffffffffffff)); u.reg_write(R.UC_ARM64_REG_X0,old); u.reg_write(R.UC_ARM64_REG_PC,a+4)
p.u.hook_add(UC_HOOK_CODE,emulate_lse)
def dltrace(u,a,z,d):
 if a==0x104ece4:
  x0=u.reg_read(R.UC_ARM64_REG_X0); x1=u.reg_read(R.UC_ARM64_REG_X1); data=bytes(u.mem_read(x0,200)).split(b'\\0',1)[0] if x0 else b''; print('DLOPEN args',hex(x0),data,hex(x1))
p.u.hook_add(UC_HOOK_CODE,dltrace)
pending_add_returns={}
captured_callbacks=[]
captured_final_vectors={}
object_stroke_snapshots=[]
factory_call_returns={}
factory_created_pens=[]
phase_rt_addresses={}
rt_identity_by_address={}
selected_rt_id=None
native_draw_entries=[]
native_motion_getter_returns=[]
pending_motion_getters={}
v1_tool_dispatch_sites=[]
v1_redrawline_threshold_sites=[]
v1_redrawline_axis_gate_sites=[]
last_v1_tool_dispatch={}
method_trace_names_by_pc={}
for _name,_address in p.syms.items():
 oblique_method='ObliquePenStrokeDrawableGLV' in _name and any(token in _name for token in ('::Draw(','::RedrawPen(','::drawPoint(','::startPen(','::endPen('))
 motion_getter=_name.startswith('SPen::MotionEvent::Get') and ('()' in _name or '(int)' in _name)
 if oblique_method or motion_getter:
  method_trace_names_by_pc.setdefault(_address,[]).append(_name)
phase='setup'
screen_resolution_calls=[]
source_size_bits=None
transition_boundaries=[]
pending_transition_boundaries={}
transition_targets_by_pc={}
sampler_repeats=[]
pending_sampler_repeats={}
sampler_repeat_targets_by_pc={}
for _name,_address in p.syms.items():
 if _name.startswith(('SPen::ObliquePenStrokeDrawableGLV1::redrawLine(', 'SPen::ObliquePenStrokeDrawableGLV1::redrawLineNoCurve(', 'SPen::ObliquePenStrokeDrawableGLV1::endPen(')):
  transition_targets_by_pc.setdefault(_address,[]).append(_name)
 if _name=='SPen::ObliquePenStrokeDrawableGLV1::getRepeat(float, float)':
  sampler_repeat_targets_by_pc.setdefault(_address,[]).append(_name)
def rt_identity(address):
 if address not in rt_identity_by_address:
  rt_identity_by_address[address]=f'rt{len(rt_identity_by_address)}'
 return rt_identity_by_address[address]

def transition_drawable_snapshot(u, address):
 if not address:return None
 rt=struct.unpack('<Q',u.mem_read(address+16,8))[0]
 state={'receiver_80_176':bytes(u.mem_read(address+80,96)).hex(),'rt_addr':hex(rt) if rt else None}
 if rt:
  state['rt_344_360']=bytes(u.mem_read(rt+344,16)).hex()
  vector=struct.unpack('<Q',u.mem_read(rt+264,8))[0]
  state['vector_count_bytes']=0
  if vector:
   start,end,capacity=struct.unpack('<3Q',u.mem_read(vector,24)); size=end-start
   if size>1_000_000:raise RuntimeError(f'invalid transition RT vector size {size}')
   state['vector_count_bytes']=size
 return state

def phase_callback_count(target_phase):
 return sum(row['phase']==target_phase for row in captured_callbacks)

def snapshot_object_stroke(label, obj):
 p.call('SPen::ObjectStroke::GetPenSize() const',[obj]); size=struct.pack('<I',p.u.reg_read(R.UC_ARM64_REG_S0)).hex()
 p.call('SPen::ObjectStroke::GetPointCount() const',[obj]); count=p.x(0)
 p.call('SPen::ObjectStroke::IsCurveEnabled() const',[obj]); curve=p.x(0)
 p.call('SPen::ObjectStroke::GetToolType() const',[obj]); tool=p.x(0)
 p.call('SPen::ObjectStroke::GetStrokeType() const',[obj]); stroke=p.x(0)
 state={'label':label,'size_bits':size,'point_count':count,'curve_enabled':curve,'tool_type':tool,'stroke_type':stroke}
 object_stroke_snapshots.append(state)
 print('OBJECT_STROKE_STATE',state)
 return state

def snapshot_source_arrays(obj):
 p.call('SPen::ObjectStroke::GetPointCount() const',[obj]); count=p.x(0)
 arrays={}
 for name,signature,element_size in (
  ('points','SPen::ObjectStroke::GetPoint() const',8),
  ('pressures','SPen::ObjectStroke::GetPressure() const',4),
  ('timestamps','SPen::ObjectStroke::GetTimeStamp() const',4),
  ('tilts','SPen::ObjectStroke::GetTilt() const',4),
  ('orientations','SPen::ObjectStroke::GetOrientation() const',4),
 ):
  p.call(signature,[obj]); data=p.x(0); size=count*element_size
  if size>1_000_000 or (count and not data): raise RuntimeError(f'invalid {signature} data pointer/count {data:#x}/{count}')
  arrays[name]={'count':count,'element_size':element_size,'data_hex':p.u.mem_read(data,size).hex() if size else ''}
 return arrays

def make_stroke(data):
 obj2=p.alloc(0x100); impl2=p.alloc(0x400)
 p.call('SPen::ObjectStroke::ObjectStroke()',[obj2]); p.call('SPen::ObjectStrokeImpl::ObjectStrokeImpl(SPen::ObjectStroke*)',[impl2,obj2]); p.put(obj2+32,'Q',impl2)
 p.f(0,data['size']); p.call('SPen::ObjectStroke::SetPenSize(float)',[obj2]); p.call('SPen::ObjectStroke::SetToolType(SPen::ToolType)',[obj2,data['tool_type']]); p.call('SPen::ObjectStroke::SetStrokeType(SPen::StrokeType)',[obj2,data['stroke_type']]); p.call('SPen::ObjectStroke::SetCurveEnabled(bool)',[obj2,int(data['curve_enabled'])])
 count=len(data['points']); pts2=p.alloc(count*8); prs2=p.alloc(count*4); ts2=p.alloc(count*4); tilt2=p.alloc(count*4); orient2=p.alloc(count*4)
 p.u.mem_write(pts2,struct.pack('<'+'f'*(count*2),*(v for xy in data['points'] for v in xy))); p.u.mem_write(prs2,struct.pack('<'+'f'*count,*data['pressure'])); p.u.mem_write(ts2,struct.pack('<'+'i'*count,*data['timestamps'])); p.u.mem_write(tilt2,struct.pack('<'+'f'*count,*data['tilt'])); p.u.mem_write(orient2,struct.pack('<'+'f'*count,*data['orientation']))
 p.call('SPen::ObjectStroke::SetPoint(SPen::PointF const*, float const*, int const*, float const*, float const*, int)',[obj2,pts2,prs2,ts2,tilt2,orient2,count]); p.call('SPen::ObjectBase::SetVisibility(bool)',[obj2,1])
 pname2=p.alloc(32); p.call('SPen::String::String()',[pname2]); p.call('SPen::String::Construct()',[pname2]); cstr2=p.alloc(80); p.u.mem_write(cstr2,b'com.samsung.android.sdk.pen.pen.preload.ObliquePen\0'); p.call('SPen::String::Set(char const*)',[pname2,cstr2]); p.call('SPen::ObjectStroke::SetPenName(SPen::String const*)',[obj2,pname2])
 advanced2=p.alloc(32); p.call('SPen::String::String()',[advanced2]); p.call('SPen::String::Construct()',[advanced2]); advstr2=p.alloc(8); p.u.mem_write(advstr2,b'4;\0'); p.call('SPen::String::Set(char const*)',[advanced2,advstr2]); p.call('SPen::ObjectStroke::SetAdvancedPenSetting(SPen::String const*)',[obj2,advanced2])
 return obj2,p.alloc(32)

def snapshot_rt(label,drawable):
 rt=struct.unpack('<Q',p.u.mem_read(drawable+16,8))[0]; vector=struct.unpack('<Q',p.u.mem_read(rt+264,8))[0] if rt else 0
 bounds=struct.unpack('<3Q',p.u.mem_read(vector,24)) if vector else (0,0,0); n=bounds[1]-bounds[0]
 if n>1_000_000 or n%12: raise RuntimeError(f'invalid {label} RT vector length {n}')
 return {'label':label,'drawable':hex(drawable),'drawable_vptr':hex(struct.unpack('<Q',p.u.mem_read(drawable,8))[0]),'rt':hex(rt),'rt_id':rt_identity(rt),'drawable_fields_44_92':p.u.mem_read(drawable+44,49).hex(),'rt_previous_xy_344_356':p.u.mem_read(rt+344,16).hex(),'vector_bytes':n,'vector_sha256':hashlib.sha256(p.u.mem_read(bounds[0],n)).hexdigest() if n else hashlib.sha256(b'').hexdigest()}

def color_contract_projection(result):
 argb=int(result['input']['color_argb_u32'],16)
 native=result['native_calls']['color']
 if int(native['ObjectStroke_GetColor_argb_u32'],16)!=argb:
  raise RuntimeError('native ObjectStroke color getter changed the explicit ARGB input')
 if result['native_calls']['route']['drawing_selected_drawable_version']!='1':
  raise RuntimeError('color contract fixture must exercise the Drawing-selected V1 drawable')
 expected_channels=((argb>>16)&0xff,(argb>>8)&0xff,argb&0xff,(argb>>24)&0xff)
 expected_bits=[struct.pack('<f',channel/255.0).hex() for channel in expected_channels]
 if native['drawing_selected_rt_rgba_f32_bits']!=expected_bits:
  raise RuntimeError(f'native RT color does not equal normalized source RGBA: {native["drawing_selected_rt_rgba_f32_bits"]} != {expected_bits}')
 libraries=result['libraries']
 if not libraries or any(len(value)!=64 for value in libraries.values()):
  raise RuntimeError('color contract requires complete pinned native library hashes')
 return {
  'schema':'sdocx-oblique-color-contract-native-v1',
  'input':result['input'],
  'profile':result['profile'],
  'native':{
   'route':result['native_calls']['route'],
   'drawing_selected_rt_id':result['native_calls']['drawing_selected_rt_id'],
   'phase_rt_ids':result['native_calls']['phase_rt_ids'],
   'ObjectDrawing_drawObjectStroke_return':result['native_calls']['ObjectDrawing::drawObjectStroke_return'],
   'color':native,
  },
  'libraries':libraries,
  'host_boundaries':result['host_boundaries'],
  'scope':result['scope'],
 }

def log_vcall(u,a,z,d):
 if args.capture_transitions and a==OBLIQUE_BASE+0x39454:
  row={'phase':phase,'pc':hex(a),'site':'redraw(ObjectStroke): ObjectStroke::GetToolType return','tool_type_return_w0':u.reg_read(R.UC_ARM64_REG_W0),'source_ObjectStroke_x25':hex(u.reg_read(R.UC_ARM64_REG_X25))}
  last_v1_tool_dispatch[phase]=row
  v1_tool_dispatch_sites.append(row)
 if args.capture_transitions and a==OBLIQUE_BASE+0x3946c:
  v1_tool_dispatch_sites.append({'phase':phase,'pc':hex(a),'site':'redraw(ObjectStroke): post-fcsel threshold before receiver+96 store','selected_threshold_f32_bits':struct.pack('<I',u.reg_read(R.UC_ARM64_REG_S0)).hex(),'alternate_threshold_f32_bits':struct.pack('<I',u.reg_read(R.UC_ARM64_REG_S1)).hex(),'nzcv':hex(u.reg_read(R.UC_ARM64_REG_NZCV)),'receiver_x21':hex(u.reg_read(R.UC_ARM64_REG_X21)),'preceding_tool_dispatch':last_v1_tool_dispatch.get(phase)})
 if args.capture_transitions and a==OBLIQUE_BASE+0x39470:
  receiver=u.reg_read(R.UC_ARM64_REG_X21)
  v1_tool_dispatch_sites.append({'phase':phase,'pc':hex(a),'site':'redraw(ObjectStroke): receiver+96 after threshold store','receiver_threshold_f32_bits':bytes(u.mem_read(receiver+96,4)).hex(),'receiver_x21':hex(receiver)})
 if args.capture_transitions and a==OBLIQUE_BASE+0x38c70:
  receiver=u.reg_read(R.UC_ARM64_REG_X19)
  nzcv=u.reg_read(R.UC_ARM64_REG_NZCV)
  v1_redrawline_threshold_sites.append({'phase':phase,'pc':hex(a),'receiver':hex(receiver),'distance_f32_bits':struct.pack('<I',u.reg_read(R.UC_ARM64_REG_S2)).hex(),'threshold_f32_bits':struct.pack('<I',u.reg_read(R.UC_ARM64_REG_S3)).hex(),'pre_gate_flags_byte_88':u.mem_read(receiver+88,1).hex(),'pre_alternation_byte_89':u.mem_read(receiver+89,1).hex(),'nzcv':hex(nzcv),'b_pl_taken':not bool(nzcv & 0x80000000)})
 if args.capture_transitions and a==OBLIQUE_BASE+0x38c44:
  nzcv=u.reg_read(R.UC_ARM64_REG_NZCV)
  v1_redrawline_axis_gate_sites.append({'phase':phase,'pc':hex(a),'dx_f32_bits':struct.pack('<I',u.reg_read(R.UC_ARM64_REG_S3)).hex(),'dy_f32_bits':struct.pack('<I',u.reg_read(R.UC_ARM64_REG_S6)).hex(),'nzcv':hex(nzcv),'both_axis_gate_taken':bool(nzcv & 0x80000000)})
 if args.capture_transitions and a in pending_sampler_repeats:
  row=pending_sampler_repeats.pop(a)
  row['return_i32']=u.reg_read(R.UC_ARM64_REG_X0)
  row['phase']=phase
  sampler_repeats.append(row)
 if args.capture_transitions and a in sampler_repeat_targets_by_pc:
  name=sampler_repeat_targets_by_pc[a][0]
  lr=u.reg_read(R.UC_ARM64_REG_LR)
  length=struct.unpack('<f',struct.pack('<I',u.reg_read(R.UC_ARM64_REG_S0)))[0]
  size=struct.unpack('<f',struct.pack('<I',u.reg_read(R.UC_ARM64_REG_S1)))[0]
  pending_sampler_repeats[lr]={'method':name,'pc':hex(a),'return_pc_expected':hex(lr),'length_f32_bits':struct.pack('<I',u.reg_read(R.UC_ARM64_REG_S0)).hex(),'length_f32':length,'size_f32_bits':struct.pack('<I',u.reg_read(R.UC_ARM64_REG_S1)).hex(),'size_f32':size}
 if args.capture_transitions and pending_transition_boundaries.get(a):
  row=pending_transition_boundaries[a].pop(0)
  row['callback_index_end']=phase_callback_count(row['phase'])
  row['receiver_after']=transition_drawable_snapshot(u,row['receiver'])
  row['return_pc']=hex(a)
  transition_boundaries.append(row)
 if args.capture_transitions and a in transition_targets_by_pc:
  names=transition_targets_by_pc[a]
  receiver=u.reg_read(R.UC_ARM64_REG_X0)
  lr=u.reg_read(R.UC_ARM64_REG_LR)
  method_match=re.search(r'::(redrawLineNoCurve|redrawLine|endPen)\(',names[0])
  row={'method':method_match.group(1) if method_match else names[0], 'signature':names[0], 'phase':phase,
       'pc':hex(a), 'return_pc_expected':hex(lr), 'callback_index_start':phase_callback_count(phase),
       'receiver':receiver, 'receiver_before':transition_drawable_snapshot(u,receiver),
       'integer_args_x0_x3':[hex(u.reg_read(getattr(R,f'UC_ARM64_REG_X{i}'))) for i in range(4)],
       'float_args_s0_s3':[hex(u.reg_read(getattr(R,f'UC_ARM64_REG_S{i}'))) for i in range(4)]}
  if '::endPen(' in names[0]:
   event=u.reg_read(R.UC_ARM64_REG_X1)
   row['motion_event_entry_bytes']=bytes(u.mem_read(event,64)).hex() if event else None
  pending_transition_boundaries.setdefault(lr,[]).append(row)
 if a in pending_motion_getters and pending_motion_getters[a]:
  call=pending_motion_getters[a].pop(0)
  call['return']={'x0':hex(u.reg_read(R.UC_ARM64_REG_X0)),'s0':hex(u.reg_read(R.UC_ARM64_REG_S0)),'d0':hex(u.reg_read(R.UC_ARM64_REG_D0))}
  call['return_pc']=hex(a); call['phase_return']=phase
  native_motion_getter_returns.append(call)
 if a==p.syms.get('createPenInst'):
  factory_call_returns[u.reg_read(R.UC_ARM64_REG_LR)]=True
 if a in factory_call_returns:
  factory_call_returns.pop(a)
  factory_created_pens.append(u.reg_read(R.UC_ARM64_REG_X0))
  print('FACTORY_RETURN pen',hex(factory_created_pens[-1]))
 if a in method_trace_names_by_pc:
  regs={f'x{i}':hex(u.reg_read(getattr(R,f'UC_ARM64_REG_X{i}'))) for i in range(8)}
  regs.update({f's{i}':hex(u.reg_read(getattr(R,f'UC_ARM64_REG_S{i}'))) for i in range(8)})
  pointers={}
  for i in (0,1,2,3,4):
   ptr=u.reg_read(getattr(R,f'UC_ARM64_REG_X{i}'))
   try:pointers[f'x{i}_head']=u.mem_read(ptr,96).hex() if ptr else None
   except Exception:pointers[f'x{i}_head']='unmapped'
  record={'signatures':method_trace_names_by_pc[a],'phase':phase,'pc':hex(a),'regs':regs,'pointer_heads':pointers}
  if any('::endPen(' in name for name in method_trace_names_by_pc[a]):
   event_ptr=u.reg_read(R.UC_ARM64_REG_X1)
   record['motion_event_entry_bytes']=u.mem_read(event_ptr,64).hex() if event_ptr else None
  native_draw_entries.append(record)
  for signature in method_trace_names_by_pc[a]:
   if signature.startswith('SPen::MotionEvent::Get'):
    lr=u.reg_read(R.UC_ARM64_REG_LR)
    pending_motion_getters.setdefault(lr,[]).append({'signature':signature,'phase_entry':phase,'entry_pc':hex(a),'return_pc_expected':hex(lr),'args':{f'x{i}':hex(u.reg_read(getattr(R,f'UC_ARM64_REG_X{i}'))) for i in range(4)}})
  print('NATIVE_METHOD_ENTRY',record)
 if a in pending_add_returns:
  this,record=pending_add_returns.pop(a); vp=struct.unpack('<Q',u.mem_read(this+264,8))[0]; bounds=struct.unpack('<3Q',u.mem_read(vp,24)) if vp else (0,0,0); n=bounds[1]-bounds[0]; before=record['pre_append_count_bytes'];
  if n<before or n>1_000_000 or (n-before)%12: raise RuntimeError(f'invalid AddPoint vector delta {before} -> {n}')
  vector=bytes(u.mem_read(bounds[0],n)); delta=vector[before:n]; record.update({'vector_count_bytes':n,'vector_delta_bytes':delta.hex()});
  captured_callbacks.append(dict(record)); captured_final_vectors[record['phase']]={'vector_count_bytes':n,'vector_bytes':vector.hex(),'sha256':hashlib.sha256(vector).hexdigest(),'k_bits':record['k_bits'],'stored_size_u32':record['stored_size_u32'],'inverse_scale_bits':record['inverse_scale_bits']}
  print('ADDPOINT EXIT',record)
 if a==0x5045288: this=u.reg_read(R.UC_ARM64_REG_X0); caller_lr=u.reg_read(R.UC_ARM64_REG_LR); phase_rt_addresses.setdefault(phase,[]).append(this); vals=[u.reg_read(getattr(R,f'UC_ARM64_REG_S{i}')) for i in range(2)]; vp=struct.unpack('<Q',u.mem_read(this+264,8))[0]; bounds=struct.unpack('<3Q',u.mem_read(vp,24)) if vp else (0,0,0); record={'phase':phase,'rt_id':rt_identity(this),'xy_bits':[hex(v) for v in vals], 'previous_xy_bits':[u.mem_read(this+344,4).hex(),u.mem_read(this+348,4).hex()], 'pre_append_count_bytes':bounds[1]-bounds[0], 'stored_size_u32':struct.unpack('<I',bytes(u.mem_read(this+60,4)))[0], 'k_bits':u.mem_read(this+200,4).hex(), 'inverse_scale_bits':[u.mem_read(this+352,4).hex(),u.mem_read(this+356,4).hex()]}; pending_add_returns[caller_lr]=(this,record); print('ADDPOINT_CALLER',hex(caller_lr)); print('ADDPOINT entry',hex(this),record)
 if a==0x5039f74: print('helper entry x0-x5',[hex(u.reg_read(getattr(R,f'UC_ARM64_REG_X{i}'))) for i in range(6)],'memx0',u.mem_read(u.reg_read(R.UC_ARM64_REG_X0),32).hex())
 if a==0x5039fd4:
  x=lambda i:u.reg_read(getattr(R,f'UC_ARM64_REG_X{i}')); print('vcall getRepeat x0',hex(x(0)),'vptr',u.mem_read(x(0),8).hex(),'x9',hex(x(9)),'slot',u.mem_read(x(9)+16,8).hex(),'x19',hex(x(19)),'x20',hex(x(20)),'x24',hex(x(24)))
p.u.hook_add(UC_HOOK_CODE,log_vcall)
def cpptrace(u,a,z,d):
 if a in (CPP_BASE+0xccc80,CPP_BASE+0xd17fc,CPP_BASE+0xa0058,CPP_BASE+0xa0068,p.syms.get('__cxa_allocate_exception'),p.syms.get('__cxa_throw'),p.syms.get('__cxa_begin_catch')):
  print('CPPTRACE',hex(a),'lr',hex(u.reg_read(R.UC_ARM64_REG_LR)),'x0',hex(u.reg_read(R.UC_ARM64_REG_X0)),'x1',hex(u.reg_read(R.UC_ARM64_REG_X1)),'x2',hex(u.reg_read(R.UC_ARM64_REG_X2)))
p.u.hook_add(UC_HOOK_CODE,cpptrace)
def taskqueue(u,a,z,d):
 if a==0x20001040:
  print('managed register',hex(p.x(0)),hex(p.x(1))); p.x(0,1); u.reg_write(R.UC_ARM64_REG_PC,u.reg_read(R.UC_ARM64_REG_LR))
 elif a==0x20001000:
  task=p.x(1); vt=struct.unpack('<Q',u.mem_read(task,8))[0]; method=struct.unpack('<Q',u.mem_read(vt+16,8))[0]; caller=u.reg_read(R.UC_ARM64_REG_LR); print('queue task',hex(p.x(0)),hex(task),hex(vt),hex(method),'words',u.mem_read(task,64).hex())
  if not method: raise RuntimeError('task has null Run')
  p._task_returns.append(caller); p.x(0,task); u.reg_write(R.UC_ARM64_REG_LR,0x20001020); u.reg_write(R.UC_ARM64_REG_PC,method)
 elif a==0x20001020:
  caller=p._task_returns.pop(); p.x(0,1); u.reg_write(R.UC_ARM64_REG_PC,caller)

def screen_trace(u,a,z,d):
 if a==0x10460b4:
  call={'width':u.reg_read(R.UC_ARM64_REG_W1),'height':u.reg_read(R.UC_ARM64_REG_W2)}; screen_resolution_calls.append(call); print('Pen::SetScreenResolution entry',call)
p.u.hook_add(UC_HOOK_CODE,screen_trace)
p.u.hook_add(UC_HOOK_CODE,taskqueue)
lifecycle_snapshots=[]

obj=p.alloc(0x100); impl=p.alloc(0x400)
try:
 p.call('SPen::ObjectStroke::ObjectStroke()', [obj])
 p.call('SPen::ObjectStrokeImpl::ObjectStrokeImpl(SPen::ObjectStroke*)',[impl,obj])
 p.put(obj+32,'Q',impl)
 print('model ctors ok',hex(obj),hex(impl),bytes(p.u.mem_read(obj,64)).hex())
 pen=p.alloc(0x400)
 p.call('SPen::ObliquePen::ObliquePen()', [pen], limit=5000000)
 manager=p.alloc(256); queue=p.alloc(64); qvt=p.alloc(64); p.put(queue,'Q',qvt); p.u.mem_write(p.context_vtable+16,struct.pack('<Q',0x20001040)); p.u.mem_write(0x20001040,bytes.fromhex('1f2003d5c0035fd6')); p.u.mem_write(0x20001000,bytes.fromhex('1f2003d5c0035fd6')); p.put(qvt+16,'Q',0x20001000); p._task_returns=[]; p.callbacks['SPen::PenGLDataManager::GetMsgQueue()']=lambda:(print('GMSGQ',hex(p.x(0)),hex(queue)),p.x(0,queue))
 p.call('SPen::PenGLDataManager::PenGLDataManager()', [manager]); p.call('SPen::PenGLDataManager::SetMsgQueue(SPen::IGLMsgQueue*)',[manager,queue]); p.put(pen+8,'Q',manager); p.call('SPen::PenGLDataManager::GetMsgQueue()',[manager]); print('manager queue',hex(p.x(0)),hex(queue))
 print('allocs first',p._newlog[:30]); print('canvasprobe addrs',{k:hex(getattr(p,k)) for k in ('drawing','context','bitmap','draw_canvas','pen_canvas','oblique','drawable_vtable','pen','canvas_reference')}); print('oblique ctor ok',hex(pen),bytes(p.u.mem_read(pen,0xa0)).hex()); p.drawing=p.alloc(256); p.u.mem_write(p.drawing,bytes(256)); p.put(p.context,'Q',p.context_vtable); profile=p.alloc(64); profile_vt=p.alloc(64); p.put(profile,'Q',profile_vt); p.put(profile_vt+32,'Q',0x200f20e0); p.put(profile_vt+40,'Q',0x200f20f0); p.u.mem_write(0x200f20d0,bytes.fromhex('40000058c0035fd6')+struct.pack('<Q',profile)); p.u.mem_write(0x200f20e0,mov_w0_ret(args.width)); p.u.mem_write(0x200f20f0,mov_w0_ret(args.height)); p.put(p.context_vtable+16,'Q',0x200f20d0); p.put(p.context_vtable+80,'Q',0x200f20c0); p.u.mem_write(0x200f20c0,bytes.fromhex('e003012ac0035fd6')); p.put(p.context_vtable+144,'Q',0x200f20a0); p.put(p.context_vtable+152,'Q',0x200f20b0); p.u.mem_write(0x200f20a0,bytes.fromhex('20008052c0035fd6')); p.u.mem_write(0x200f20b0,bytes.fromhex('00102e1ec0035fd6')); p.call('SPen::ObjectDrawing::ObjectDrawing()',[p.drawing]); p.call('SPen::ObjectDrawing::Construct(SPen::IContext const*)',[p.drawing,p.context]); print('drawing constructed',p.u.mem_read(p.drawing,128).hex()); p.call('SPen::ObjectDrawing::setIPenBitmap(SPen::ISPPenCanvas*, SPen::IPen*, SPen::StrokeType)',[p.drawing,p.pen_canvas,pen,0],limit=5000000); d=struct.unpack('<Q',p.u.mem_read(pen+112,8))[0]; print('drawing setIPenBitmap drawable',hex(d),bytes(p.u.mem_read(d,0x180)).hex() if d else None);
 if not args.omit_size: p.f(0,input_case['size']); p.call('SPen::ObjectStroke::SetPenSize(float)',[obj])
 p.call('SPen::ObjectStroke::GetPenSize() const',[obj]); source_size_bits=struct.pack('<I',p.u.reg_read(R.UC_ARM64_REG_S0)).hex(); print('native ObjectStroke GetPenSize S0 bits',source_size_bits); p.call('SPen::ObjectStroke::SetToolType(SPen::ToolType)',[obj,input_case['tool_type']]); p.call('SPen::ObjectStroke::SetStrokeType(SPen::StrokeType)',[obj,input_case['stroke_type']]); p.call('SPen::ObjectStroke::SetCurveEnabled(bool)',[obj,int(input_case['curve_enabled'])]); print('SetCurveEnabled post-call x0',p.x(0)); points=input_case['points']; count=len(points); pts=p.alloc(count*8); prs=p.alloc(count*4); ts=p.alloc(count*4); tilt=p.alloc(count*4); orient=p.alloc(count*4); p.u.mem_write(pts,struct.pack('<'+'f'*(count*2),*(v for xy in points for v in xy))); p.u.mem_write(prs,struct.pack('<'+'f'*count,*input_case['pressure'])); p.u.mem_write(ts,struct.pack('<'+'i'*count,*input_case['timestamps'])); p.u.mem_write(tilt,struct.pack('<'+'f'*count,*input_case['tilt'])); p.u.mem_write(orient,struct.pack('<'+'f'*count,*input_case['orientation'])); p.call('SPen::ObjectStroke::SetPoint(SPen::PointF const*, float const*, int const*, float const*, float const*, int)',[obj,pts,prs,ts,tilt,orient,count]); source_argb=native_argb=native_color_type=None
 if args.case=='color_contract' or args.omit_color:
  if args.case=='color_contract' and not args.omit_color:
   source_argb=0x80402010; p.call('SPen::ObjectStroke::SetColor(unsigned int)',[obj,source_argb])
  p.call('SPen::ObjectStroke::GetColor() const',[obj]); native_argb=p.x(0); source_argb=native_argb if source_argb is None else source_argb; p.call('SPen::ObjectStroke::GetColorType() const',[obj]); native_color_type=p.x(0)
 native_pen_setter_calls=[]
 if args.omit_size or args.omit_color:
  size_entry=p.syms['SPen::Pen::SetSize(float)']; color_entry=p.syms['SPen::Pen::SetColor(int)']
  def capture_pen_setter(u,address,size,data):
   if address==size_entry:
    native_pen_setter_calls.append({'method':'SPen::Pen::SetSize(float)','arg_s0_bits':struct.pack('<I',u.reg_read(R.UC_ARM64_REG_S0)).hex(),'caller_lr':hex(u.reg_read(R.UC_ARM64_REG_LR))})
   elif address==color_entry:
    native_pen_setter_calls.append({'method':'SPen::Pen::SetColor(int)','arg_w1':u.reg_read(R.UC_ARM64_REG_W1),'caller_lr':hex(u.reg_read(R.UC_ARM64_REG_LR))})
  p.u.hook_add(UC_HOOK_CODE,capture_pen_setter)
 rect=p.alloc(32); p.call('SPen::ObjectBase::SetVisibility(bool)',[obj,1]); p.call('SPen::ObjectBase::IsVisible() const',[obj]); print('visible',p.x(0)); p.call('SPen::ObjectStroke::GetPenName() const',[obj]); print('penname initial',hex(p.x(0))); pname=p.alloc(32); p.call('SPen::String::String()',[pname]); p.call('SPen::String::Construct()',[pname]); cstr=p.alloc(80); p.u.mem_write(cstr,b'com.samsung.android.sdk.pen.pen.preload.ObliquePen\0'); p.call('SPen::String::Set(char const*)',[pname,cstr]); p.call('SPen::ObjectStroke::SetPenName(SPen::String const*)',[obj,pname]);
 if args.advanced_setting=='4':
  advanced=p.alloc(32); p.call('SPen::String::String()',[advanced]); p.call('SPen::String::Construct()',[advanced]); advanced_cstr=p.alloc(8); p.u.mem_write(advanced_cstr,b'4;\0'); p.call('SPen::String::Set(char const*)',[advanced,advanced_cstr]); p.call('SPen::ObjectStroke::SetAdvancedPenSetting(SPen::String const*)',[obj,advanced]);
 p.call('SPen::ObjectStroke::GetPenName() const',[obj]); print('penname set',hex(p.x(0)),p.u.mem_read(p.x(0),24).hex()); p.call('SPen::ObjectStroke::GetPointCount() const',[obj]); print('count pre',p.x(0)); print('after setcanvas fields',[hex(struct.unpack('<Q',p.u.mem_read(d+i,8))[0]) for i in range(0,128,8)]); p.call('SPen::PenStrokeDataManager::GetInstance()',[]); print('real cache manager',hex(p.x(0)),p.u.mem_read(p.x(0),64).hex()); cachemanager=p.x(0); p.call('SPen::PenStrokeDataManager::GetStrokeData(int)',[cachemanager,0]); print('cache data runtime0',hex(p.x(0)),p.u.mem_read(p.x(0),32).hex() if p.x(0) else None); p.call('SPen::ObjectStroke::GetPointCount() const',[obj]); print('pointcount',p.x(0),'drawable_fields',[hex(struct.unpack('<Q',p.u.mem_read(d+i,8))[0]) for i in range(0,128,8)]); snapshot_object_stroke('before_drawObjectStroke',obj); source_object_arrays=snapshot_source_arrays(obj); phase='draw_object_stroke'; p.call('SPen::ObjectDrawing::drawObjectStroke(SPen::ISPCanvas&, SPen::ISPPenCanvas&, SPen::ObjectStroke const*, bool)',[p.drawing,p.draw_canvas,p.pen_canvas,obj,1],limit=5000000); print('drawObjectStroke result',p.x(0),'loader calls',loader_calls,'drawer',p.u.mem_read(p.drawing,48).hex()); selected_pen=factory_created_pens[-1]; p.call('SPen::ObliquePen::GetStrokeDrawableGL()',[selected_pen]); selected_drawable=p.x(0); selected_vptr=struct.unpack('<Q',p.u.mem_read(selected_drawable,8))[0] if selected_drawable else 0; selected_sub_vptr=struct.unpack('<Q',p.u.mem_read(selected_drawable+8,8))[0] if selected_drawable else 0; selected_object_redraw_target=struct.unpack('<Q',p.u.mem_read(selected_sub_vptr+40,8))[0] if selected_sub_vptr else 0; selected_object_redraw_names=[name for name,address in p.syms.items() if address==selected_object_redraw_target]; selected_drawable_version=next((re.search(r'ObliquePenStrokeDrawableGLV(\d+)',name).group(1) for name in selected_object_redraw_names if re.search(r'ObliquePenStrokeDrawableGLV(\d+)',name)), 'unknown'); print('SELECTED_VTABLE',hex(selected_vptr),hex(selected_sub_vptr),hex(selected_object_redraw_target),selected_object_redraw_names); selected_rt=struct.unpack('<Q',p.u.mem_read(selected_drawable+16,8))[0] if selected_drawable else 0; selected_rt_id=rt_identity(selected_rt) if selected_rt else None; print('DRAW_SELECTED_NATIVE pen/drawable/rt/callbacks',hex(selected_pen),hex(selected_drawable),hex(selected_rt),selected_rt_id,[hex(v) for v in phase_rt_addresses.get('draw_object_stroke',[])]); actual_pen=factory_created_pens[-1]; actual_drawable=selected_drawable; actual_rt=struct.unpack('<Q',p.u.mem_read(actual_drawable+16,8))[0]; rt_color_bytes=bytes(p.u.mem_read(actual_rt+44,16)); rt_color_rgba=struct.unpack('<4f',rt_color_bytes); print('Drawing-selected redraw pen/drawable',hex(actual_pen),hex(actual_drawable));
 if args.redraw_version=='selected':
  expected_version='4' if args.advanced_setting=='4' else '1'
  if selected_drawable_version!=expected_version: raise RuntimeError(f"advanced setting {args.advanced_setting!r} selected V{selected_drawable_version}, expected V{expected_version}")
  drawing_entries=[row for row in native_draw_entries if row['phase']=='draw_object_stroke' and row['pc']==hex(selected_object_redraw_target)]
  if not drawing_entries: raise RuntimeError('Drawing did not enter the selected drawable RedrawPen vtable target')
 if args.two_stroke:
  first_drawable=actual_drawable; first_rt=selected_rt; first_version=selected_drawable_version
  lifecycle_snapshots.append(snapshot_rt('after_first_three_point_draw',first_drawable))
  second_obj,second_rect=make_stroke(INPUT_CASES['reuse_second'])
  snapshot_object_stroke('before_second_short_stroke',second_obj); second_arrays=snapshot_source_arrays(second_obj)
  lifecycle_snapshots.append(snapshot_rt('before_second_short_draw',first_drawable))
  phase='second_draw_object_stroke'
  p.call('SPen::ObjectDrawing::drawObjectStroke(SPen::ISPCanvas&, SPen::ISPPenCanvas&, SPen::ObjectStroke const*, bool)',[p.drawing,p.draw_canvas,p.pen_canvas,second_obj,1],limit=5000000)
  second_pen=factory_created_pens[-1]; p.call('SPen::ObliquePen::GetStrokeDrawableGL()',[second_pen]); actual_drawable=p.x(0); actual_pen=second_pen
  selected_vptr=struct.unpack('<Q',p.u.mem_read(actual_drawable,8))[0]; selected_sub_vptr=struct.unpack('<Q',p.u.mem_read(actual_drawable+8,8))[0]
  selected_object_redraw_target=struct.unpack('<Q',p.u.mem_read(selected_sub_vptr+40,8))[0]
  selected_object_redraw_names=[name for name,address in p.syms.items() if address==selected_object_redraw_target]
  selected_drawable_version=next((re.search(r'ObliquePenStrokeDrawableGLV(\d+)',name).group(1) for name in selected_object_redraw_names if re.search(r'ObliquePenStrokeDrawableGLV(\d+)',name)),'unknown')
  selected_rt=struct.unpack('<Q',p.u.mem_read(actual_drawable+16,8))[0]; selected_rt_id=rt_identity(selected_rt)
  if selected_drawable_version!='4': raise RuntimeError(f'second stroke selected V{selected_drawable_version}')
  if actual_drawable!=first_drawable or selected_rt!=first_rt: raise RuntimeError('second native Drawing call replaced the selected drawable or RT')
  second_entries=[row for row in native_draw_entries if row['phase']=='second_draw_object_stroke' and row['pc']==hex(selected_object_redraw_target)]
  if not second_entries: raise RuntimeError('second Drawing call did not enter the selected drawable RedrawPen vtable target')
  lifecycle_snapshots.append(snapshot_rt('after_second_short_draw',actual_drawable))
  source_object_arrays=second_arrays; obj=second_obj; rect=second_rect
 snapshot_object_stroke('before_saved_RedrawPen',obj); p.call('SPen::Pen::GetSize()', [actual_pen]); pen_size_after_draw_bits=struct.pack('<I',p.u.reg_read(R.UC_ARM64_REG_S0)).hex(); print('native Pen::GetSize after draw S0 bits',pen_size_after_draw_bits); phase='saved_redraw'
 if args.redraw_version=='selected':
  if not selected_object_redraw_target: raise RuntimeError('selected drawable has no ObjectStroke RedrawPen vtable slot')
  call_address(selected_object_redraw_target,[actual_drawable+8,obj,rect,rect])
 else:
  p.call('SPen::ObliquePenStrokeDrawableGLV4::RedrawPen(SPen::ObjectStroke const*, SPen::RectF*)',[actual_drawable,obj,rect],limit=5000000)
 print('redraw result',hex(p.x(0)), 'rect',p.u.mem_read(rect,16).hex()); p.call('SPen::ObliquePenStrokeDrawableGL::getSize()', [actual_drawable]); print('native drawable size after redraw S0',struct.unpack('<f',struct.pack('<I',p.u.reg_read(R.UC_ARM64_REG_S0)))[0]); p.call('SPen::ObliquePenStrokeDrawableGLV4::getInverseScale()', [actual_drawable]); print('native inverse scale after redraw S0S1',struct.unpack('<2f',struct.pack('<II',p.u.reg_read(R.UC_ARM64_REG_S0),p.u.reg_read(R.UC_ARM64_REG_S1)))); rt=struct.unpack('<Q',p.u.mem_read(actual_drawable+16,8))[0]; vp=struct.unpack('<Q',p.u.mem_read(rt+264,8))[0]; print('rt',hex(rt),'vector_ptr',hex(vp),'vector_raw',p.u.mem_read(rt+264,24).hex()); vv=struct.unpack('<3Q',p.u.mem_read(vp,24)) if vp else (0,0,0); print('vector',tuple(hex(v) for v in vv))
 # The native AddPoint return hook copies the complete callback vector before
 # its caller submits/clears it. This is a synthetic explicit context profile.
 import json
 if any(row['vector_count_bytes'] % 12 for row in captured_callbacks): raise RuntimeError('partial native Vector3 callback')
 for row in captured_callbacks:
  if 'vector_delta_bytes' not in row: raise RuntimeError('incomplete callback vector delta')
 callbacks=[{k:v for k,v in row.items() if k not in ('vector_ptr','vector_bounds')} for row in captured_callbacks]
 p.call('SPen::ObliquePenStrokeDrawableGL::getSize()', [actual_drawable]); drawable_size_bits=struct.pack('<I',p.u.reg_read(R.UC_ARM64_REG_S0)).hex()
 p.call('SPen::ObliquePenStrokeDrawableGLV4::getInverseScale()', [actual_drawable]); inverse_scale_bits=[struct.pack('<I',p.u.reg_read(R.UC_ARM64_REG_S0)).hex(),struct.pack('<I',p.u.reg_read(R.UC_ARM64_REG_S1)).hex()]
 expected_profiles=2 if args.two_stroke else 1
 if screen_resolution_calls != [{'width':args.width,'height':args.height}]*expected_profiles: raise RuntimeError(f'unexpected native screen profile calls: {screen_resolution_calls}')
 if not callbacks or not {'draw_object_stroke','saved_redraw'} <= set(r['phase'] for r in callbacks): raise RuntimeError(f'missing callback phases: {[r["phase"] for r in callbacks]}')
 if any(pending_motion_getters.values()): raise RuntimeError(f'MotionEvent getter calls missed native returns: {pending_motion_getters}')
 if not native_motion_getter_returns or not {'draw_object_stroke','saved_redraw'} <= set(r['phase_entry'] for r in native_motion_getter_returns): raise RuntimeError('missing live MotionEvent getter return evidence')
 # Only the original two-point witness has an established byte-for-byte
 # caller/redraw equivalence. Other probes retain both streams independently
 # because their purpose is to expose native lifecycle/history differences.
 draw_phase='second_draw_object_stroke' if args.two_stroke else 'draw_object_stroke'
 if args.redraw_version=='selected' and captured_final_vectors[draw_phase]['vector_bytes']!=captured_final_vectors['saved_redraw']['vector_bytes']: raise RuntimeError('Drawing and selected-vtable saved RedrawPen final callback streams differ')
 native_hashes={name:hashlib.sha256(path.read_bytes()).hexdigest() for name,path in paths.items()}
 native_hashes['libc++_shared.so']=hashlib.sha256(CPP.read_bytes()).hexdigest()
 native_hashes['libSPenComposer.so']=hashlib.sha256((LIBS/'libSPenComposer.so').read_bytes()).hexdigest()
 traced_methods=[{k:v for k,v in row.items() if k in ('signatures','phase','pc','regs','motion_event_entry_bytes')} for row in native_draw_entries if any(token in name for name in row['signatures'] for token in ('::RedrawPen(','::endPen(','::drawPoint('))]
 result={'schema':'sdocx-oblique-saved-caller-native-case-v1','case_id':args.case,'profile':{'width':args.width,'height':args.height,'canvas_model_matrix_f32_bits':[struct.pack('<f',v).hex() for v in (1,0,0,0,1,0,0,0,1)]},'input':{'kind':'synthetic native ObjectStroke setter input; no serialized SDocx parser input','pen_name':'com.samsung.android.sdk.pen.pen.preload.ObliquePen','stroke_type':input_case['stroke_type'],'tool_type':input_case['tool_type'],'curve_enabled':input_case['curve_enabled'],'pen_size_f32_bits':source_size_bits,'points_f32_bits':[[struct.pack('<f',v).hex() for v in xy] for xy in points],'pressure_f32_bits':[struct.pack('<f',v).hex() for v in input_case['pressure']],'timestamps_i32':input_case['timestamps'],'tilt_f32_bits':[struct.pack('<f',v).hex() for v in input_case['tilt']],'orientation_f32_bits':[struct.pack('<f',v).hex() for v in input_case['orientation']],'advanced_setting':('4;' if args.advanced_setting=='4' else None)},'native_calls':{'lifecycle_snapshots':lifecycle_snapshots,'first_drawable_version':first_version if args.two_stroke else None,'second_input':INPUT_CASES['reuse_second'] if args.two_stroke else None,'second_source_ObjectStroke_arrays':second_arrays if args.two_stroke else None,'ObjectDrawing::drawObjectStroke_return':1,'selected_drawable_object_redraw_return':1,'route':{'runner_mode':args.redraw_version,'advanced_setting':('4;' if args.advanced_setting=='4' else None),'drawing_selected_drawable_version':selected_drawable_version,'saved_redraw_invoked_version':('4' if args.redraw_version=='v4' else selected_drawable_version)},'source_ObjectStroke_arrays':source_object_arrays,'motion_event_getter_returns':native_motion_getter_returns,'loader':loader_calls,'pen_screen_resolution':screen_resolution_calls,'source_ObjectStroke_GetPenSize_f32_bits':source_size_bits,'ObjectStroke_getter_snapshots':object_stroke_snapshots,'selected_drawable_redraw_lifecycle_method_entries':traced_methods,'drawing_selected_rt_id':selected_rt_id,'phase_rt_ids':{ph:sorted({rt_identity(v) for v in addresses}) for ph,addresses in phase_rt_addresses.items()},'pen_GetSize_after_draw_f32_bits':pen_size_after_draw_bits,'drawable_getSize_f32_bits':drawable_size_bits,'drawable_getInverseScale_f32_bits':inverse_scale_bits},'callback_add_point_calls':callbacks,'phase_final_vectors':captured_final_vectors,'libraries':native_hashes,'host_boundaries':['ObjectDrawing context vslot +16 supplies the screen-profile object; its vtable +32/+40 returns explicit width/height','context vslot +80 is an explicit identity color-conversion host callback','context vslots +144/+152 return explicit flags/version values 1 and 1.0','canvas/bitmap/context/resource/ownership adapters and managed task queue registration are host; queued task Run methods and Drawing/Oblique geometry execute natively'],'scope':'Native Drawing caller plus direct saved RedrawPen under an explicit synthetic profile. This does not exercise SDK deserialization, Samsung device defaults, renderer pixels, or SVG/PDF export.'}
 if args.omit_size or args.omit_color:
  result['input']['style_setters_omitted']={'pen_size':args.omit_size,'color_argb':args.omit_color}
  result['native_calls']['missing_style_defaults_probe']={'ObjectStroke_GetPenSize_f32_bits':source_size_bits,'ObjectStroke_GetColor_argb_u32':f'0x{native_argb:08x}','ObjectStroke_GetColorType_u32':native_color_type,'Pen_GetSize_after_Drawing_f32_bits':pen_size_after_draw_bits,'selected_drawable_getSize_after_saved_redraw_f32_bits':drawable_size_bits,'drawing_selected_rt_RGBA_f32_bits':[struct.pack('<f',v).hex() for v in rt_color_rgba],'native_Pen_SetSize_SetColor_entries_during_Drawing':native_pen_setter_calls}
 if args.capture_transitions:
  if args.redraw_version!='selected' or selected_drawable_version!='1': raise RuntimeError('transition capture is restricted to selected Oblique V1')
  if any(pending_transition_boundaries.values()): raise RuntimeError('native transition return was not captured')
  if pending_sampler_repeats: raise RuntimeError('native getRepeat return was not captured')
  phase_names={row['phase'] for row in transition_boundaries}
  if not {'draw_object_stroke','saved_redraw'} <= phase_names: raise RuntimeError('transition capture missing native draw phases')
  phase_indices={}
  for row in transition_boundaries:
   row['phase_transition_index']=phase_indices.get(row['phase'],0)
   phase_indices[row['phase']]=row['phase_transition_index']+1
   if row['callback_index_end'] < row['callback_index_start']:
    raise RuntimeError('native transition callback boundary regressed')
  result['native_calls']['v1_transition_boundaries']=transition_boundaries
  result['native_calls']['v1_tool_dispatch_sites']=v1_tool_dispatch_sites
  result['native_calls']['v1_redrawline_threshold_sites']=v1_redrawline_threshold_sites
  result['native_calls']['v1_redrawline_axis_gate_sites']=v1_redrawline_axis_gate_sites
  result['native_calls']['sampler_repeats']=sampler_repeats
 if args.case=='color_contract':
  result['input']['color_argb_u32']=f'0x{source_argb:08x}'
  result['native_calls']['color']={'ObjectStroke_GetColor_argb_u32':f'0x{native_argb:08x}','ObjectStroke_GetColorType_u32':native_color_type,'drawing_selected_rt_rgba_f32_bits':[struct.pack('<f',v).hex() for v in rt_color_rgba],'drawing_selected_rt_rgba_f32':list(rt_color_rgba),'drawing_selected_rt_rgba_bytes_le':rt_color_bytes.hex(),'context_color_conversion':'explicit identity host callback'}
  if not args.omit_size and not args.omit_color: color_oracle=color_contract_projection(result)
 if args.redraw_version=='selected':
  result['native_calls']['selected_drawable_object_redraw_entry']=selected_object_redraw_names[0] if selected_object_redraw_names else None
  result['native_calls']['selected_drawable_primary_vtable_id']='ObliquePenStrokeDrawableGLV'+selected_drawable_version
  result['scope']=f'Native Drawing caller plus direct RedrawPen through the selected GLV{selected_drawable_version} vtable slot under an explicit synthetic profile. This does not exercise SDK deserialization, Samsung device defaults, renderer pixels, or SVG/PDF export.'
 if not args.two_stroke:
  for key in ('lifecycle_snapshots','first_drawable_version','second_input','second_source_ObjectStroke_arrays'):
   result['native_calls'].pop(key)
 encoded=json.dumps(result,indent=2,sort_keys=True)+'\n'
 def assert_reuse_reference(reused,fresh):
  if reused['native_calls']['first_drawable_version']!='4': raise RuntimeError('reuse witness did not begin with selected V4 drawable')
  snapshots=reused['native_calls']['lifecycle_snapshots']
  if [row['label'] for row in snapshots] != ['after_first_three_point_draw','before_second_short_draw','after_second_short_draw']:
   raise RuntimeError('reuse witness is missing native RT lifecycle boundaries')
  if snapshots[0]['drawable']!=snapshots[2]['drawable'] or snapshots[0]['rt']!=snapshots[2]['rt']:
   raise RuntimeError('reuse witness did not preserve selected drawable/RT identity')
  start_xy=[struct.pack('<f',value).hex() for value in INPUT_CASES['reuse_second']['points'][0]]
  if snapshots[0]['rt_previous_xy_344_356'][:16]!=bytes.fromhex('0000d0410000a041').hex():
   raise RuntimeError('first stroke RT did not retain its expected 26,20 endpoint before reuse')
  if snapshots[2]['rt_previous_xy_344_356'][:16]!=bytes.fromhex('0000f04100000040').hex():
   raise RuntimeError('second stroke RT did not retain its observed 30,2 previous point')
  actual=reused['phase_final_vectors']['second_draw_object_stroke']; expected=fresh['phase_final_vectors']['draw_object_stroke']
  for key in ('vector_bytes','sha256','vector_count_bytes','k_bits','stored_size_u32','inverse_scale_bits'):
   if actual.get(key)!=expected.get(key): raise RuntimeError(f'reuse/fresh native output differs in {key}')
  reused_calls=[row for row in reused['callback_add_point_calls'] if row['phase']=='second_draw_object_stroke']
  fresh_calls=[row for row in fresh['callback_add_point_calls'] if row['phase']=='draw_object_stroke']
  callback_fields=('xy_bits','previous_xy_bits','pre_append_count_bytes','stored_size_u32','k_bits','inverse_scale_bits','vector_count_bytes','vector_delta_bytes')
  if len(reused_calls)!=len(fresh_calls): raise RuntimeError('reuse/fresh callback counts differ')
  if not reused_calls or reused_calls[0]['previous_xy_bits']!=start_xy or reused_calls[0]['pre_append_count_bytes']!=0:
   raise RuntimeError('second stroke did not begin with a clean vector and its own start coordinate')
  for index,(actual_call,expected_call) in enumerate(zip(reused_calls,fresh_calls)):
   if any(actual_call.get(key)!=expected_call.get(key) for key in callback_fields): raise RuntimeError(f'reuse/fresh callback {index} differs')

 if args.write_fixture:
  if FIXTURE==REUSE_FIXTURE:
   fixture=json.loads(FIXTURE.read_text()) if FIXTURE.exists() else {'schema':'sdocx-oblique-saved-caller-reuse-native-v1'}
   if fixture.get('schema')!='sdocx-oblique-saved-caller-reuse-native-v1': raise RuntimeError('unexpected reuse fixture schema')
   slot='reused' if args.two_stroke else 'fresh_reference'
   fixture[slot]=result
   if 'reused' in fixture and 'fresh_reference' in fixture: assert_reuse_reference(fixture['reused'],fixture['fresh_reference'])
   FIXTURE.write_text(json.dumps(fixture,indent=2,sort_keys=True)+'\n')
   print(f'wrote {FIXTURE.relative_to(ROOT)} {slot}',_always=True)
  else:
   if FIXTURE.exists():
    old=json.loads(FIXTURE.read_text())
    cases=old.get('cases',{}) if old.get('schema')=='sdocx-oblique-saved-caller-native-v1' else {old.get('case_id','two_point_curve'):old}
   else: cases={}
   if args.case=='color_contract': cases[args.case]=color_oracle
   else: cases[args.case]=result
   FIXTURE.write_text(json.dumps({'schema':'sdocx-oblique-saved-caller-native-v1','cases':cases},indent=2,sort_keys=True)+'\n')
   print(f'wrote {FIXTURE.relative_to(ROOT)} case {args.case}',_always=True)
 elif args.check_profile_invariance:
  baseline=json.loads(FIXTURE.read_text())['cases'][args.case]
  baseline_callbacks=baseline['callback_add_point_calls']
  if len(callbacks)!=len(baseline_callbacks): raise SystemExit('callback count changed across profiles')
  for index,(actual,expected) in enumerate(zip(callbacks,baseline_callbacks)):
   for key in ('phase','xy_bits','previous_xy_bits','pre_append_count_bytes','stored_size_u32','k_bits','inverse_scale_bits','vector_count_bytes','vector_delta_bytes'):
    if actual.get(key)!=expected.get(key): raise SystemExit(f'profile changed callback {index} field {key}')
  if captured_final_vectors != baseline['phase_final_vectors'] or inverse_scale_bits!=baseline['native_calls']['drawable_getInverseScale_f32_bits'] or drawable_size_bits!=baseline['native_calls']['drawable_getSize_f32_bits']:
   raise SystemExit('profile changed native drawable size/inverse scale')
  print(f'verified callback/runtime invariance at {args.width}x{args.height}',_always=True)
 elif args.check_fixture:
  if not FIXTURE.exists(): raise SystemExit('native caller fixture missing')
  fixture=json.loads(FIXTURE.read_text())
  if FIXTURE==REUSE_FIXTURE:
   if 'reused' not in fixture or 'fresh_reference' not in fixture: raise SystemExit('reuse fixture requires both --two-stroke and fresh --case reuse_second captures')
   expected=fixture.get('reused' if args.two_stroke else 'fresh_reference')
   try: assert_reuse_reference(fixture['reused'],fixture['fresh_reference'])
   except RuntimeError as error: raise SystemExit(str(error))
  else: expected=fixture.get('cases',{}).get(args.case)
  if args.case=='color_contract':
   if expected!=color_oracle: raise SystemExit(f'native caller fixture mismatch for {args.case}')
  elif expected!=result: raise SystemExit(f'native caller fixture mismatch for {args.case}')
  print(f'verified {FIXTURE.relative_to(ROOT)} case {args.case}',_always=True)
 else: print(encoded,end='',_always=True)
except Exception:
 raise
