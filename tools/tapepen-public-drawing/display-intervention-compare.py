from pathlib import Path
import subprocess, zlib, hashlib, json
from PIL import Image
import sys
p=Path(sys.argv[1]); repo=Path(__file__).resolve().parent
receipt=json.loads((repo/'display-public-receipt.json').read_text())
assert subprocess.check_output(['resvg', '--version'], text=True).strip() == receipt['resvg']
compressed=(repo/'identity-screen-512-frame.rgba.zlib').read_bytes()
assert hashlib.sha256(compressed).hexdigest() == receipt['compressed_native_sha256']['512']
raw=zlib.decompress(compressed)
assert len(raw) == 512*512*4
native=b''.join(raw[y*2048:y*2048+1024] for y in range(256))
src=(repo/'v173-vector.py').read_text().replace("Path(__file__).parent / 'pattern01-native.rgba.zlib'",('Path(' + repr(str(repo/'pattern01-native.rgba.zlib')) + ')'))
assert (p/'old.mask').read_bytes() == (p/'quad.mask').read_bytes()
assert hashlib.sha256((p/'old.mask').read_bytes()).hexdigest() == '7bf0e2bd0dcf16e2d5805101727e13cacde38048f39165b56a83c7d9cb40ed20'
assert src.count('x-ox, y-oy, 4, True') == 1
for mask in ('old','quad'):
 for period in (64,128):
  code=src.replace('x-ox, y-oy, 4, True', ('x-ox, y-oy, 4, True' if period==64 else '(x-ox)*0.5-0.25, (y-oy)*0.5-0.25, 4, True'))
  (p/'vector.py').write_text(code)
  svg=p/f'{mask}-{period}.svg'; png=svg.with_suffix('.png')
  subprocess.run(['python3',str(p/'vector.py'),str(p/f'{mask}.mask'),str(svg),'77','59'],check=True,stdout=subprocess.DEVNULL)
  subprocess.run(['resvg',str(svg),str(png)],check=True)
  data=Image.open(png).convert('RGBA').tobytes(); pixels=bytes(round(v*data[i-i%4+3]/255) if i%4<3 else v for i,v in enumerate(data)); diffs=[abs(a-b) for a,b in zip(native,pixels,strict=True)]
  result=(sum(diffs),max(diffs),sum(v!=0 for v in diffs),hashlib.sha256(svg.read_bytes()).hexdigest())
  expected={64:(11123,113,258,'591285401d40ee86b0b2cf839d4a6225efd3b6e3322c34efd40bb5d77590e735'),128:(5303,73,272,'b87c41adf45d5a6c472900e96b7dc01c847cab27f30452e1f8d3f011e6bb36fb')}
  assert result == expected[period], (mask,period,result)
  print(mask,period,result)
