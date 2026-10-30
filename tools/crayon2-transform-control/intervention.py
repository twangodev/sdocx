if not __debug__:
    raise RuntimeError('This evidence check requires assertions; do not use python -O')

import sys
import hashlib
from pathlib import Path
w=Path(__file__).resolve().with_name('capture.py')
expected_capture = '8b4fe4fe953288e45d21484df23e60be1091273e67c0aa3af2f3d1de76900a4a'
if hashlib.sha256(w.read_bytes()).hexdigest() != expected_capture:
    raise SystemExit('translated capture changed; review intervention before updating its hash')
s=w.read_text(); idx=int(sys.argv[1]); mode=sys.argv[2]
a='# Keep the native texture-scale uniform alongside the paint calls.'
inject='\nvector_data=vector_data[%d:%d]\np.u.mem_write(vecbegin,vector_data)\np.put(vecobj+8,"Q",vecbegin+24)'%(idx*24,(idx+1)*24)
patch='anchor = "assert len(vector_data)%24==0 and len(vector_data)>0"\nassert source.count(anchor)==1\nsource=source.replace(anchor,anchor + '+repr(inject)+')\n'
assert s.count(a) == 1
s=s.replace(a,patch+a)
assert s.count("source = source.replace('translation=(0.0,0.0)") == 1
s=s.replace("source = source.replace('translation=(0.0,0.0)","assert source.count(\"if control_argb >> 24:\") == 1\nsource = source.replace(\"if control_argb >> 24:\", \"if False:\")\nassert source.count(\"else:\\n assert bbox is None, f'zero-alpha source unexpectedly changed background: {bbox}'\") == 1\nsource = source.replace(\"else:\\n assert bbox is None, f'zero-alpha source unexpectedly changed background: {bbox}'\", \"else:\\n pass\")\nsource = source.replace('translation=(0.0,0.0)")
if mode!='paint':
 probe='''native_source=p.callbacks['glShaderSource']
def diagnostic_source():
 shader,count,arr,lens=[p.x(i) for i in range(4)]
 raw=p.cstr(p.get(arr,'Q')[0]) if count==1 and not lens else b''
 if b'uniform sampler2D pointPattern;' not in raw:return native_source()
 text=raw.decode()
 assert text.count('FragColor = vec4(inputColor.xyz, inputColor.w * brushMask * paper * bgTextureAlpha * 5.0);') == 1
 assert text.count('FragColor = vec4(inputColor.xyz, inputColor.w * brushMask * bgTextureAlpha * 5.0);') == 1
 source=text.replace('FragColor = vec4(inputColor.xyz, inputColor.w * brushMask * paper * bgTextureAlpha * 5.0);','FragColor = vec4(vec3(DIAG) + inputColor.xyz * 0.000001, 1.0 - inputColor.w * brushMask * paper * bgTextureAlpha * 0.000001);').replace('FragColor = vec4(inputColor.xyz, inputColor.w * brushMask * bgTextureAlpha * 5.0);','FragColor = vec4(vec3(DIAG) + inputColor.xyz * 0.000001, 1.0 - inputColor.w * brushMask * bgTextureAlpha * 0.000001);').replace('DIAG',EXPR)

 if MODE in ('xuv','yuv'):
  expr={'xuv':'vBgCanvasUV.x','yuv':'vBgCanvasUV.y'}[MODE];bits='floatBitsToUint('+expr+')'; packed='vec3(float(('+bits+' >> 16u) & 255u),float(('+bits+' >> 8u) & 255u),float('+bits+' & 255u)) / 255.0';assert source.count('vec3('+expr+')') == 2;source=source.replace('vec3('+expr+')',packed)
 buf=C.create_string_buffer(source.encode());ptr=C.cast(buf,C.c_char_p);arrc=(C.c_char_p*1)(ptr);length=(C.c_int*1)(len(source));p.gles.f['glShaderSource'](shader,1,arrc,length)
 p.shader_source_calls.append({'shader':shader,'source':[source]})
p.callbacks['glShaderSource']=diagnostic_source
'''.replace('EXPR',repr({'point':'brushMask','background':'bgTextureAlpha','xfrac':'fract(vBgCanvasUV.x * uBgTextureScale.x * 150.0)','yfrac':'fract(vBgCanvasUV.y * uBgTextureScale.y * 150.0)','xuv':'vBgCanvasUV.x','yuv':'vBgCanvasUV.y'}[mode]))
 probe=probe.replace('MODE',repr(mode))
 a2='# Split the native helper\'s instanced draw from its diagnostic mask readback so'
 s=s.replace(a, 'assert source.count('+repr(a2)+') == 1\nsource=source.replace('+repr(a2)+','+repr(probe+'\n'+a2)+')\n'+a)
exec(compile(s,str(w),'exec'),{'__name__':'__main__','__file__':str(w)})
