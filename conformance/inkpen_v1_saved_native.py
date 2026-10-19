"""Replayable native InkPen V1 caller matrix. Run with repository-pinned Unicorn and AArch64 binutils."""
import argparse, hashlib, json, os, pathlib, subprocess, sys
ROOT=pathlib.Path(__file__).resolve().parents[1]
SOURCE=ROOT/'conformance/inkpen_saved_caller_native.py'
FIXTURE=ROOT/'conformance/inkpen-v1-saved-native.json'
SOURCE_SHA256='c0b1657cdcf94bcf2c173ce68ed7817e55a4de246883b47265bb7b4d3f50dc04'
CASE_SPECS=json.loads('{"duplicate_1920x1080":{"spec":{"curve_enabled":true,"orientation":[0.0,0.0,0.0,0.0,0.0,0.0],"points":[[0,0],[0,0],[0,0],[5,0],[5,0],[12,2]],"pressure":[0.2,0.3,0.4,0.6,0.7,0.9],"size":10.0,"stroke_type":0,"tilt":[0.0,0.0,0.0,0.0,0.0,0.0],"timestamps":[1,2,3,4,5,6],"tool_type":1},"width":1920,"height":1080,"advanced":"null"},"five_1080x1920":{"spec":{"curve_enabled":true,"orientation":[0.0,0.0,0.0,0.0,0.0],"points":[[0,0],[10,0],[20,10],[28,14],[40,0]],"pressure":[0.2,0.4,0.6,0.8,1.0],"size":10.0,"stroke_type":0,"tilt":[0.0,0.0,0.0,0.0,0.0],"timestamps":[10,20,30,40,50],"tool_type":1},"width":1080,"height":1920,"advanced":"null"},"five_1440x2560":{"spec":{"curve_enabled":true,"orientation":[0.0,0.0,0.0,0.0,0.0],"points":[[0,0],[10,0],[20,10],[28,14],[40,0]],"pressure":[0.2,0.4,0.6,0.8,1.0],"size":10.0,"stroke_type":0,"tilt":[0.0,0.0,0.0,0.0,0.0],"timestamps":[10,20,30,40,50],"tool_type":1},"width":1440,"height":2560,"advanced":"null"},"five_1920x1080":{"spec":{"curve_enabled":true,"orientation":[0.0,0.0,0.0,0.0,0.0],"points":[[0,0],[10,0],[20,10],[28,14],[40,0]],"pressure":[0.2,0.4,0.6,0.8,1.0],"size":10.0,"stroke_type":0,"tilt":[0.0,0.0,0.0,0.0,0.0],"timestamps":[10,20,30,40,50],"tool_type":1},"width":1920,"height":1080,"advanced":"null"},"line_1920x1080":{"spec":{"curve_enabled":false,"orientation":[0.0,0.0,0.0,0.0],"points":[[0,0],[10,0],[20,10],[30,10]],"pressure":[0.25,0.5,0.75,1.0],"size":10.0,"stroke_type":0,"tilt":[0.0,0.0,0.0,0.0],"timestamps":[10,20,30,40],"tool_type":1},"width":1920,"height":1080,"advanced":"null"},"straight_curve_tool1_null_1440x2560":{"spec":{"points":[[0.0,0.0],[10.0,0.0],[20.0,0.0],[30.0,10.0]],"size":10.0,"curve_enabled":true,"tool_type":1,"stroke_type":0,"pressure":[0.25,0.5,0.75,1.0],"timestamps":[10,20,30,40],"tilt":[0.0,0.0,0.0,0.0],"orientation":[0.0,0.0,0.0,0.0]},"width":1440,"height":2560,"advanced":"null"},"three_1080x1920":{"spec":{"curve_enabled":true,"orientation":[0.0,0.0,0.0],"points":[[0,0],[10,0],[20,10]],"pressure":[0.25,0.6,0.95],"size":10.0,"stroke_type":0,"tilt":[0.0,0.0,0.0],"timestamps":[10,20,45],"tool_type":1},"width":1080,"height":1920,"advanced":"null"},"three_1920x1080":{"spec":{"curve_enabled":true,"orientation":[0.0,0.0,0.0],"points":[[0,0],[10,0],[20,10]],"pressure":[0.25,0.6,0.95],"size":10.0,"stroke_type":0,"tilt":[0.0,0.0,0.0],"timestamps":[10,20,45],"tool_type":1},"width":1920,"height":1080,"advanced":"null"},"time_fast_1920x1080":{"spec":{"curve_enabled":true,"orientation":[0.0,0.0,0.0,0.0],"points":[[0,0],[10,0],[20,10],[30,10]],"pressure":[0.25,0.5,0.75,1.0],"size":10.0,"stroke_type":0,"tilt":[0.0,0.0,0.0,0.0],"timestamps":[10,11,12,13],"tool_type":1},"width":1920,"height":1080,"advanced":"null"},"time_slow_1920x1080":{"spec":{"curve_enabled":true,"orientation":[0.0,0.0,0.0,0.0],"points":[[0,0],[10,0],[20,10],[30,10]],"pressure":[0.25,0.5,0.75,1.0],"size":10.0,"stroke_type":0,"tilt":[0.0,0.0,0.0,0.0],"timestamps":[10,1000,2000,3000],"tool_type":1},"width":1920,"height":1080,"advanced":"null"},"tool2_1920x1080":{"spec":{"curve_enabled":true,"orientation":[0.0,0.0,0.0,0.0],"points":[[0,0],[10,0],[20,10],[30,10]],"pressure":[0.25,0.5,0.75,1.0],"size":10.0,"stroke_type":0,"tilt":[0.0,0.0,0.0,0.0],"timestamps":[10,20,30,40],"tool_type":2},"width":1920,"height":1080,"advanced":"null"},"two_1080x1920":{"spec":{"curve_enabled":true,"orientation":[0.0,0.0],"points":[[0,0],[10,0]],"pressure":[0.3,0.9],"size":10.0,"stroke_type":0,"tilt":[0.0,0.0],"timestamps":[10,20],"tool_type":1},"width":1080,"height":1920,"advanced":"null"},"two_1920x1080":{"spec":{"curve_enabled":true,"orientation":[0.0,0.0],"points":[[0,0],[10,0]],"pressure":[0.3,0.9],"size":10.0,"stroke_type":0,"tilt":[0.0,0.0],"timestamps":[10,20],"tool_type":1},"width":1920,"height":1080,"advanced":"null"},"straight_curve_tool1_1920x1080":{"spec":{"points":[[0.0,0.0],[10.0,0.0],[20.0,0.0],[30.0,10.0]],"size":10.0,"curve_enabled":true,"tool_type":1,"stroke_type":0,"pressure":[0.25,0.5,0.75,1.0],"timestamps":[10,20,30,40],"tilt":[0.0,0.0,0.0,0.0],"orientation":[0.0,0.0,0.0,0.0]},"width":1920,"height":1080,"advanced":"null"},"single_rejected_1920x1080":{"spec":{"points":[[0,0]],"size":10.0,"curve_enabled":true,"tool_type":1,"stroke_type":0,"pressure":[0.7],"timestamps":[10],"tilt":[0.0],"orientation":[0.0]},"width":1920,"height":1080,"advanced":"null","expect_rejected":true}}')

def capture(case_id):
    proc=subprocess.run([sys.executable,str(pathlib.Path(__file__).resolve()),'--capture',case_id],cwd=ROOT,env=os.environ.copy(),text=True,capture_output=True,check=True)
    return json.loads(proc.stdout)

def execute_case(case_id):
    source=SOURCE.read_text()
    if hashlib.sha256(source.encode()).hexdigest()!=SOURCE_SHA256: raise SystemExit('pinned InkPen caller changed')
    config=CASE_SPECS[case_id]; spec=config['spec']
    anchor="case_id='straight_curve_tool1_'+args.advanced_setting\ninput_case={'points':[[0.,0.],[10.,0.],[20.,0.],[30.,10.]],'size':10.0,'curve_enabled':True,'tool_type':1,'stroke_type':0,'pressure':[.25,.5,.75,1.0],'timestamps':[10,20,30,40],'tilt':[0.]*4,'orientation':[0.]*4}"
    replacement='case_id='+repr(case_id)+'\ninput_case='+repr(spec)
    if source.count(anchor)!=1: raise SystemExit('caller input patch anchor mismatch')
    source=source.replace(anchor,replacement,1)
    old_case="case_id='straight_curve_tool1_'+args.advanced_setting"
    if source.count(old_case)!=1: raise SystemExit('caller result-case patch anchor mismatch')
    source=source.replace(old_case,'case_id='+repr(case_id),1)
    if config.get('expect_rejected'):
        old=" if draw_result != 1 or not factory_created_pens:\n  raise RuntimeError(f'Drawing failed to select InkPen: result={draw_result}, loader={loader_calls}')"
        block=[
" if draw_result != 1 or not factory_created_pens:",
"  if draw_result == 0 and not factory_created_pens and not loader_calls and len(points)==1:",
"   rejected={'schema':'sdocx-inkpen-v1-admission-result-v1','case_id':case_id,'admission':'rejected_before_pen_creation','profile':{'width':args.width,'height':args.height},'input':{'kind':'synthetic native ObjectStroke setter input; no serialized SDocx parser input','pen_name':'com.samsung.android.sdk.pen.pen.preload.InkPen','stroke_type':input_case['stroke_type'],'tool_type':input_case['tool_type'],'curve_enabled':input_case['curve_enabled'],'size_f32_bits':struct.pack('<f',input_case['size']).hex(),'points_f32_bits':[[struct.pack('<f',v).hex() for v in xy] for xy in input_case['points']],'pressure_f32_bits':[struct.pack('<f',v).hex() for v in input_case['pressure']],'timestamps_i32':input_case['timestamps'],'tilt_f32_bits':[struct.pack('<f',v).hex() for v in input_case['tilt']],'orientation_f32_bits':[struct.pack('<f',v).hex() for v in input_case['orientation']]},'native_model_getters':model_getter_contract,'native_calls':{'ObjectStroke_getter_snapshots':object_stroke_snapshots,'source_ObjectStroke_arrays':source_object_arrays},'route':{'drawing_return':draw_result,'factory_created_pens':0,'loader_calls':[],'selected_drawable_class':None,'stream_emitted':False},'host_boundaries':['Native Model setters/getters and source-array reads run; ObjectDrawing rejects this one-point object before pen creation. No saved redraw or AddPoint stream is claimed.'],'scope':'Caller admission rejection only; not an InkPen V1 stream.'}",
"   print(json.dumps(rejected,indent=2,sort_keys=True),_always=True)",
"   raise SystemExit(0)",
"  raise RuntimeError(f'Drawing failed to select InkPen: result={draw_result}, loader={loader_calls}')"
        ]
        if source.count(old)!=1: raise SystemExit('caller rejection patch anchor mismatch')
        source=source.replace(old,'\n'.join(block),1)
    old_args='args=parser.parse_args()'
    argv=[str(SOURCE),'--print-capture','--advanced-setting',config.get('advanced','null'),'--width',str(config['width']),'--height',str(config['height'])]
    if source.count(old_args)!=1: raise SystemExit('caller argparse patch anchor mismatch')
    source=source.replace(old_args,'args=parser.parse_args('+repr(argv[1:])+')',1)
    ns={'__name__':'__main__','__file__':str(SOURCE)}
    exec(compile(source,str(SOURCE),'exec'),ns,ns)

def main():
    if '--capture' in sys.argv:
        i=sys.argv.index('--capture')
        if i+1>=len(sys.argv) or sys.argv[i+1] not in CASE_SPECS: raise SystemExit('unknown embedded case')
        execute_case(sys.argv[i+1]); return
    p=argparse.ArgumentParser(description=__doc__)
    mode=p.add_mutually_exclusive_group()
    mode.add_argument('--write-fixture',action='store_true',help='explicitly write or refresh the fixture')
    mode.add_argument('--check-fixture',action='store_true',help='explicitly check the fixture')
    args=p.parse_args()
    cases={key:capture(key) for key in CASE_SPECS}
    value={'schema':'sdocx-inkpen-v1-saved-native-v1','provenance':{'caller':SOURCE.name,'caller_sha256':SOURCE_SHA256,'wrapper_sha256':hashlib.sha256(pathlib.Path(__file__).read_bytes()).hexdigest()},'cases':cases}
    if args.write_fixture:
        FIXTURE.write_text(json.dumps(value,indent=2,sort_keys=True)+'\n')
        print(f'wrote {FIXTURE.relative_to(ROOT)} ({len(cases)} cases)')
    else:
        if not FIXTURE.exists(): raise SystemExit('fixture missing; use --write-fixture explicitly')
        if json.loads(FIXTURE.read_text())!=value: raise SystemExit('native InkPen V1 fixture mismatch')
        print(f'verified {len(cases)} native InkPen V1 cases')

if __name__=='__main__': main()
