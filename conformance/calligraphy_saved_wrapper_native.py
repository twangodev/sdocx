"""Hash-pinned saved ObjectStroke → Calligraphy V1 MotionEvent wrapper oracle.

Runs native Model getters, the saved RedrawPen thunk, Base array MotionEvent
constructor/accessors, and V1 event redraw. Platform services and mesh submission
remain bounded host shims documented in the fixture.
"""
import argparse
import hashlib, json, struct
import sys
from pathlib import Path
from unicorn import Uc
import calligraphy_saved_native as c
from fountain_native import NativeFountain
ROOT=Path(__file__).resolve().parents[1]
DEFAULT_LIB=ROOT/'scratch/apk-output/samsung-notes-4.4.45.37/lib/arm64-v8a'
LIB=DEFAULT_LIB
OBJdump=ROOT/'scratch/apk-analysis-runtime/binutils-aarch64/usr/bin/aarch64-linux-gnu-objdump'
MODEL_SHA='4fbcf6d4213e929f1535d32abb487743643fd5d0dfc366e50dfeb2e7d8015b7a'
BASE_SHA='e10da0116946691cf68302437ef261282e1dfe0eec15bf2dfa66093286985deb'
CALLIGRAPHY_SHA=c.LIB_SHA
# Reuse the calligraphy oracle's geometry collectors but map the additional Model ELF.
c.fountain_native.HASHES['Model']=MODEL_SHA
orig_map=Uc.mem_map
def map_model(self,address,size,*args,**kwargs):
    if address==4*0x1000000 and size==0x200000: size=0x600000
    return orig_map(self,address,size,*args,**kwargs)
Uc.mem_map=map_model
orig_init=NativeFountain.__init__
def init_with_model(self, library_dir, objdump, additional_libraries=()):
    return orig_init(self, library_dir, objdump, (*additional_libraries, 'Model'))
NativeFountain.__init__=init_with_model
n=None
counts={}
original_calligraphy_hook=c.NativeCalligraphy.hook
def calligraphy_hook(self,u,address,size,data):
    if address in counts and hasattr(self,'native_motion_calls'): self.native_motion_calls.append(counts[address])
    if address==self.redraw_address+0x34c and getattr(self,'wrapper_mode',False):
        from unicorn import arm64_const as R
        u.reg_write(R.UC_ARM64_REG_PC,self.redraw_address+0x354)
        return
    original_calligraphy_hook(self,u,address,size,data)
c.NativeCalligraphy.hook=calligraphy_hook
n=c.NativeCalligraphy(LIB, OBJdump)
if hashlib.sha256((LIB/'libSPenBase.so').read_bytes()).hexdigest()!=BASE_SHA: raise RuntimeError('Base SHA mismatch')
# Let Base construct/read/destroy MotionEvent natively. Keep host allocators and
# calligraphy mesh/queue/bounds interfaces explicit.
for k in ['SPen::MotionEvent::MotionEvent(int, int, SPen::PointF const*, float const*, int, int const*, float const*, float const*, long long const*)', 'SPen::MotionEvent::MotionEvent(long long, int, int, SPen::MotionEvent::PointerProperties*, SPen::MotionEvent::PointerCoords*, int, int, int)', 'SPen::MotionEvent::~MotionEvent()',
          'SPen::MotionEvent::GetHistorySize() const','SPen::MotionEvent::GetHistoricalX(int, int) const','SPen::MotionEvent::GetHistoricalY(int, int) const','SPen::MotionEvent::GetHistoricalOrientation(int) const',
          'SPen::MotionEvent::GetToolType(int) const','SPen::MotionEvent::GetEventTime() const','SPen::MotionEvent::GetX(int) const','SPen::MotionEvent::GetY(int) const','SPen::MotionEvent::GetPressure(int) const','SPen::MotionEvent::GetOrientation(int) const','SPen::MotionEvent::GetTilt(int) const','SPen::MotionEvent::GetDownTime() const','SPen::MotionEvent::GetButtonState() const','SPen::MotionEvent::GetSource() const','SPen::MotionEvent::GetFlags() const']:
    n.callbacks.pop(k, None)
n.callbacks['std::__ndk1::recursive_mutex::lock()']=lambda:None
n.callbacks['std::__ndk1::recursive_mutex::unlock()']=lambda:None
n.callbacks['std::__ndk1::recursive_mutex::recursive_mutex()']=lambda:None
n.callbacks['std::__ndk1::recursive_mutex::~recursive_mutex()']=lambda:None
n.callbacks['__android_log_print']=lambda:n.x(0,0)
n.callbacks['SPen::Error::SetError(unsigned long, int, char const*)']=lambda:n.x(0,0)
n.callbacks['SPen::Error::SetError(unsigned long)']=lambda:n.x(0,0)
def nothrow_new():
    amount=n.x(0)
    if amount>0x100000: raise RuntimeError(f'unexpected nothrow allocation {amount:#x} PC={n.u.reg_read(__import__("unicorn").arm64_const.UC_ARM64_REG_PC):#x}')
    n.x(0,n.alloc(amount))
n.callbacks['operator new(unsigned long, std::nothrow_t const&)']=nothrow_new
n.callbacks['operator delete(void*, std::nothrow_t const&)']=lambda:None
# Ensure Base methods actually execute; count their entries.
base_sym_names=['SPen::MotionEvent::MotionEvent(int, int, SPen::PointF const*, float const*, int, int const*, float const*, float const*, long long const*)','SPen::MotionEvent::MotionEvent(long long, int, int, SPen::MotionEvent::PointerProperties*, SPen::MotionEvent::PointerCoords*, int, int, int)','SPen::MotionEvent::GetHistorySize() const','SPen::MotionEvent::GetHistoricalX(int, int) const','SPen::MotionEvent::GetHistoricalY(int, int) const','SPen::MotionEvent::GetHistoricalOrientation(int) const','SPen::MotionEvent::GetToolType(int) const','SPen::MotionEvent::GetX(int) const','SPen::MotionEvent::GetY(int) const','SPen::MotionEvent::GetPressure(int) const','SPen::MotionEvent::GetOrientation(int) const','SPen::MotionEvent::GetTilt(int) const','SPen::MotionEvent::GetEventTime() const','SPen::MotionEvent::~MotionEvent()']
base_sym_names += ['SPen::ObjectStroke::GetToolType() const','SPen::ObjectStroke::GetPointCount() const','SPen::ObjectStroke::GetPoint() const','SPen::ObjectStroke::GetPressure() const','SPen::ObjectStroke::GetTimeStamp() const','SPen::ObjectStroke::GetTilt() const','SPen::ObjectStroke::GetOrientation() const']
for sym in base_sym_names:
    if sym not in n.syms: raise RuntimeError('missing Base symbol '+sym)
    counts[n.syms[sym]]=sym
n.wrapper_mode=True

# One native Model object; each fixture gets fresh drawable state below.
obj=n.alloc(0x100); impl=n.alloc(0x400); rect=n.alloc(32)
n.queue=n.alloc(64); qvt=n.alloc(32); n.put(n.queue,'Q',qvt); n.put(qvt+16,'Q',0x20001000)
n.call('SPen::ObjectStroke::ObjectStroke()', [obj])
n.call('SPen::ObjectStrokeImpl::ObjectStrokeImpl(SPen::ObjectStroke*)', [impl,obj])
n.put(obj+32,'Q',impl)
# Actual Model setters are used for saved size and tool. Constructor default
# StrokeType remains zero; count and channel vectors model each decoded record.
n.call('SPen::ObjectStroke::SetPenSize(float)', [obj], [6.0])
n.call('SPen::ObjectStroke::SetToolType(SPen::ToolType)', [obj, 2])
n.call('SPen::ObjectStroke::GetToolType() const', [obj])
if n.x(0) != 2: raise RuntimeError(f'ObjectStroke SetToolType/GetToolType ABI mismatch: {n.x(0)}')
saved_tool_type=n.x(0)
n.call('SPen::ObjectStroke::GetStrokeType() const',[obj]); stroke_type=n.x(0)

# Fixture vectors are installed into real ObjectStrokeImpl vector triplets;
# per-channel reads execute actual Model getters.
def run_case(case, obj, impl, rect, stroke_type):
    rows=case['rows']; count=len(rows)
    if not rows: raise ValueError('empty strokes are excluded: saved wrapper dereferences GetTimeStamp()[0] at 0x1ed04 before any zero-count return')
    n.native_motion_calls=[]; n.add_calls=[]; n.build_calls=[]; n.drawable_states=[]; n.orientation_reads=[]; n.sample_ends=[]; n.terminal_start=None; n.position_queries=[]; n.length_queries=[]; n.pending_position_returns={}; n.pending_length_returns={}; n.active_add_index=None; n.output_labels={}; n.current_size=6.0
    pen_data=n.alloc(0x100); settings=n.alloc(0x80); manager=n.alloc(0x80); drawable=n.alloc(0x600); n.drawable=drawable
    n.call('SPen::MontblancCalligraphyPenStrokeDrawableGLV1::MontblancCalligraphyPenStrokeDrawableGLV1(SPen::MontblancCalligraphyPenData*, SPen::PenGLDataManager*)',[drawable,pen_data,manager])
    # Constructor installs native state. Resolve only the relocated primary
    # slot 144 to the real event RedrawPen entry for Unicorn dispatch.
    primary_vt=n.alloc(0x100)
    for off in range(0,0x100,8): n.put(primary_vt+off,'Q',0x20001000)
    n.put(primary_vt+144,'Q',n.syms['SPen::MontblancCalligraphyPenStrokeDrawableGLV1::RedrawPen(SPen::MotionEvent const*, SPen::RectF*)'])
    n.put(drawable,'Q',primary_vt)
    n.put(drawable+72,'Q',pen_data); n.put(pen_data,'Q',settings); n.put(settings,'f',6.0)
    def set_stroke(points_rows, tilt_values, orientation_values):
        count2=len(points_rows)
        n.event_end={'x':points_rows[-1][0],'y':points_rows[-1][1],
                     'orientation':orientation_values[-1] if orientation_values else 0.0,
                     'pressure':points_rows[-1][2], 'tilt':tilt_values[-1] if tilt_values else 0.0,
                     'time':points_rows[-1][3], 'tool':2}
        points=n.alloc(count2*8); pressures=n.alloc(count2*4); times=n.alloc(count2*4)
        tilt=n.alloc(count2*4) if tilt_values is not None else 0
        orient=n.alloc(count2*4) if orientation_values is not None else 0
        for i,row in enumerate(points_rows):
            n.put(points+8*i,'ff',row[0],row[1]); n.put(pressures+4*i,'f',row[2]); n.put(times+4*i,'i',row[3])
            if tilt: n.put(tilt+4*i,'f',tilt_values[i])
            if orient: n.put(orient+4*i,'f',orientation_values[i])
        n.put(impl+36,'I',count2)
        for off,ptr in ((48,points),(72,pressures),(96,times),(120,tilt),(144,orient)):
            n.put(impl+off,'QQQ',ptr,ptr+count2*(8 if off==48 else 4),ptr+count2*(8 if off==48 else 4)) if ptr else n.put(impl+off,'QQQ',0,0,0)
        n.put(impl+292,'f',6.0)
    set_stroke(rows,case['tilt'],case['orientation'])
    # exact Drawing dispatch subobject pointer; thunk adjusts this-8 into GLV1.
    ret=n.call('non-virtual thunk to SPen::MontblancCalligraphyPenStrokeDrawableGLV1::RedrawPen(SPen::ObjectStroke const*, SPen::RectF*)',[drawable+8,obj,rect],instruction_limit=3000000)
    first={'wrapper_return':n.x(0),'add_vectors':[[*point['vector_bits'],point['extent_bits']] for point in n.add_calls],
           'drawable_states':n.drawable_states,
           'sample_ends':n.sample_ends,'orientation_reads':n.orientation_reads,
           'motion_event_call_counts':{name:n.native_motion_calls.count(name) for name in sorted(set(n.native_motion_calls))},
           'build_call_count':len(n.build_calls),
           'direction_bits':[n.get(drawable+276+4*i,'I')[0].to_bytes(4,'little').hex() for i in range(2)],
           'angle_bits':n.get(drawable+284,'I')[0].to_bytes(4,'little').hex(),'state':n.drawable_state(drawable)}
    repeat=None
    if case.get('repeat_rows'):
        n.native_motion_calls=[]; n.add_calls=[]; n.build_calls=[]; n.drawable_states=[]; n.orientation_reads=[]; n.sample_ends=[]; n.terminal_start=None; n.active_add_index=None
        set_stroke(case['repeat_rows'],case.get('repeat_tilt'),case.get('repeat_orientation'))
        ret=n.call('non-virtual thunk to SPen::MontblancCalligraphyPenStrokeDrawableGLV1::RedrawPen(SPen::ObjectStroke const*, SPen::RectF*)',[drawable+8,obj,rect],instruction_limit=3000000)
        repeat={'wrapper_return':n.x(0),'add_vectors':[[*point['vector_bits'],point['extent_bits']] for point in n.add_calls],
                'drawable_states':n.drawable_states,
                'sample_ends':n.sample_ends,'orientation_reads':n.orientation_reads,
                'motion_event_call_counts':{name:n.native_motion_calls.count(name) for name in sorted(set(n.native_motion_calls))},
                'build_call_count':len(n.build_calls),
                'direction_bits':[n.get(drawable+276+4*i,'I')[0].to_bytes(4,'little').hex() for i in range(2)],
                'angle_bits':n.get(drawable+284,'I')[0].to_bytes(4,'little').hex(),'state':n.drawable_state(drawable)}
    return {'case':case['id'],'source_count':count,'stroke_type':stroke_type,'saved_tool_type':saved_tool_type,'wrapper_return':first['wrapper_return'],'same_drawable_repeat':repeat,
            'actual_base_model_call_counts':first['motion_event_call_counts'],
            'native_drawable_states':first['drawable_states'],'sample_ends':first['sample_ends'],'orientation_reads':first['orientation_reads'],
            'native_add_point_vectors':first['add_vectors'],'direction_bits':first['direction_bits'],
            'angle_bits':first['angle_bits'],'state':first['state'],'build_call_count':first['build_call_count']}

def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--fixture',type=Path,default=ROOT/'conformance/calligraphy-saved-wrapper-native.json')
    mode=parser.add_mutually_exclusive_group(required=True)
    mode.add_argument('--write-fixture',action='store_true')
    mode.add_argument('--check-fixture',action='store_true')
    args=parser.parse_args()
    fixture=json.loads(args.fixture.read_text())
    if fixture.get('schema')!='sdocx-calligraphy-saved-wrapper-v1': raise ValueError('unexpected fixture schema')
    expected={'base_sha256':BASE_SHA,'model_sha256':MODEL_SHA,'calligraphy_sha256':CALLIGRAPHY_SHA}
    if fixture.get('source')!=expected: raise ValueError('fixture library provenance differs')
    results=[]; mismatches=[]
    for case in fixture['cases']:
        result=run_case(case,obj,impl,rect,stroke_type)
        if args.check_fixture and case.get('native')!=result: mismatches.append(case['id'])
        if args.write_fixture: case['native']=result
        results.append(result)
        print(f"{case['id']}: source={result['source_count']} AddPoint={len(result['native_add_point_vectors'])} wrapper={result['wrapper_return']}")
    if args.check_fixture and mismatches: raise SystemExit('native fixture mismatch: '+', '.join(mismatches))
    if args.write_fixture: args.fixture.write_text(json.dumps(fixture,indent=2)+'\n')
    print(f"{len(results)} saved-wrapper cases {'matched' if args.check_fixture else 'written'}")

if __name__=='__main__': main()
