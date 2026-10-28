import importlib.util,sys
from pathlib import Path
src=Path('tmp/vector-parity-plan/pen-family-audit/pencil2-v14-geometry-v12/capture_candidate.py').resolve()
spec=importlib.util.spec_from_file_location('pencil_capture',src); m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m)
name=sys.argv[1]
settings={'alpha_mid_low':('14;',0x40336699),'alpha_mid_high':('14;',0xc0336699),'color_empty':('empty',0x80336699),'color_second':('14;',0xffdd4411),'alpha_zero':('14;',0x00336699),'alpha_full':('14;',0xff336699),'alpha_zero_large':('14;',0x00336699)}
setting,color=settings[name]
m.CASES[name]={**m._CONTROL_BASELINE,'case_id':name,'advanced_setting_arg':setting,'advanced_setting_state':'controlled '+setting,'color_argb':color}
if name == 'alpha_zero_large': m.CASES[name].update(size=8.0,pressure=[1.0]*4)
m.OUTPUTS=Path('tmp/pencil2-gradient-scalability/captures').resolve()
sys.argv=[str(src),'--capture','--case',name,'--width','1920','--height','1080','--profile-evidence','synthetic host control']
m.main()
