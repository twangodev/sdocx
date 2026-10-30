#!/usr/bin/env python3
"""Capture a single stamp using a diagnostic recompiled fragment shader.

This intervention preserves uniforms with tiny sentinel contributions. The
result is not a measurement of the unmodified shader intermediate.
"""
import sys
from pathlib import Path
mode=sys.argv[1]
wrapper=Path(__file__).resolve().with_name('capture-stamp.py')
s=wrapper.read_text()
anchor="source = source.replace(anchor, patch + anchor)"
injection='''needle = "exec(compile(source, str(base), 'exec')"
probe = {probe!r}
anchor2 = "# Split the native helper's instanced draw from its diagnostic mask readback so"
assert source.count(needle)==1
source=source.replace(needle, "source=source.replace("+repr(anchor2)+", "+repr(probe+"\\n"+anchor2)+")\\n"+needle)
'''
probe='''native_source=p.callbacks['glShaderSource']
def diagnostic_source():
 shader,count,arr,lens=[p.x(i) for i in range(4)]
 raw=p.cstr(p.get(arr,'Q')[0]) if count==1 and not lens else b''
 if b'uniform sampler2D pointPattern;' not in raw:
  return native_source()
 text=raw.decode()
 assert text.count('FragColor = vec4(inputColor.xyz, inputColor.w * brushMask * paper * bgTextureAlpha * 5.0);') == 1
 assert text.count('FragColor = vec4(inputColor.xyz, inputColor.w * brushMask * bgTextureAlpha * 5.0);') == 1
 source=text.replace('FragColor = vec4(inputColor.xyz, inputColor.w * brushMask * paper * bgTextureAlpha * 5.0);','FragColor = vec4(vec3(DIAG) + inputColor.xyz * 0.000001, 1.0 - inputColor.w * brushMask * paper * bgTextureAlpha * 0.000001);').replace('FragColor = vec4(inputColor.xyz, inputColor.w * brushMask * bgTextureAlpha * 5.0);','FragColor = vec4(vec3(DIAG) + inputColor.xyz * 0.000001, 1.0 - inputColor.w * brushMask * bgTextureAlpha * 0.000001);').replace('DIAG',EXPR)
 buf=C.create_string_buffer(source.encode()); ptr=C.cast(buf,C.c_char_p); arrc=(C.c_char_p*1)(ptr); length=(C.c_int*1)(len(source));p.gles.f['glShaderSource'](shader,1,arrc,length)
 p.shader_source_calls.append({'shader':shader,'source':[source]})
p.callbacks['glShaderSource']=diagnostic_source
'''.replace('EXPR',repr({'point':'brushMask','background':'bgTextureAlpha','alpha':'inputColor.w * brushMask * bgTextureAlpha * 5.0','xfrac':'fract(vBgCanvasUV.x * uBgTextureScale.x * 150.0)','xcoord':'vBgCanvasUV.x * uBgTextureScale.x','ycoord':'vBgCanvasUV.y * uBgTextureScale.y'}[mode]))
assert s.count(anchor) == 1
s=s.replace(anchor,anchor+'\n'+injection.format(probe=probe))
sys.argv=[str(wrapper),sys.argv[2]];exec(compile(s,str(wrapper),'exec'),{'__name__':'__main__','__file__':str(wrapper)})
