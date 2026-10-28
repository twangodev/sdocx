import sys
import importlib.util,hashlib,json,os,struct
from pathlib import Path

if len(sys.argv) != 2:
    raise SystemExit("usage: python3 " + sys.argv[0] + " NEW_OUTPUT.json")
output = Path(sys.argv[1])
if output.exists():
    raise FileExistsError(output)
root=Path(__file__).resolve().parents[2]; path=root/'tmp/conformance-cleanup/native-capture-a241793/conformance/inkpen_v1_boundaries_native.py'
assert hashlib.sha256(path.read_bytes()).hexdigest() == 'dc2a2ef356eab6dd826673f6cf77ff2269ab74a080463f943ff7d2e97fad4aec'
spec=importlib.util.spec_from_file_location('boundaries',path); h=importlib.util.module_from_spec(spec); spec.loader.exec_module(h)
assert hashlib.sha256(h.WRAPPER.read_bytes()).hexdigest() == h.PINNED_HELPERS['conformance/inkpen_v1_reuse_native.py']
w=h.load('v1reuse',h.WRAPPER); w.verify_pins(); caller=h.load('reuse',w.PINNED_RUNNER)
source=h.add_no_curve_line_returns(h.add_no_curve_trace(w.instrument_v1_trace(caller))); anchor="p.f(0,input_case['size']); p.call('SPen::ObjectStroke::SetPenSize(float)',[obj]);";assert source.count(anchor)==1
base=json.loads(h.BASE_CASES.read_text())['cases']; template=h.case_input(base['five_1920x1080']);results=[]
for width,tool,curve in [(0.5,1,True),(6,1,True),(30,3,True),(6,0,False),(12,4,False)]:
 inp=dict(template,tool_type=tool,curve_enabled=curve); patch=f"p.call('SPen::ObjectStroke::SetFixedWidthEnabled(bool)',[obj,1]); p.f(0,{width}); p.call('SPen::ObjectStroke::SetFixedWidth(float)',[obj]); "+anchor;caller.patched_caller_source=lambda:source.replace(anchor,patch);label=f'w{width}-tool{tool}-curve{curve}';row=h.capture(caller,inp,label,{'width':1920,'height':1080});summary=h.validate_event_ranges(row,inp,label);results.append({'width':width,'tool':tool,'curve':curve,'input':row['input'],'summary':summary,'stream':row['streams']['draw_object_stroke'],'native_model_getters':row['native_model_getters'],'trace':row['reuse']['native_redraw_sample_ranges'],'no_curve_trace':row['reuse'].get('native_no_curve_line_ranges',[])});print(label,summary,flush=True)
with output.open('x') as f: f.write(json.dumps({'schema':'sdocx-inkpen-v1-fixed-width-boundaries-v1','scope':'Synthetic native Drawing event return prefixes; not saved-note or device parity','cases':results,'harness_sha256':hashlib.sha256(path.read_bytes()).hexdigest()},indent=2)+'\n')
