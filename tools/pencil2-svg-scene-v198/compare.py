import hashlib
import os
import zlib
from pathlib import Path
from PIL import Image, __version__

if not __debug__:
    raise RuntimeError('Run without Python optimization')
assert __version__ == '12.1.1'
work = Path(os.environ['PENCIL198_DIR'])
svg_hashes = [('205c33002617dd6372beb96834f180c11352688562363092b02e9544f67e9a13', '6c3de72b8c45d3e53a52af3b3821388682b532bfdd893092adbe5045df3fdd2b'), ('19cc491867b710d382f90f50c8c68f87e3340ae699c844d932c233201be676a2', '428630d74c89428623a761aeffcbf83e6f8188492b0144516c397d15a5766f3d')]
native_hashes = ['ab11d63776cd76aff35f5365b55cf7e7e11b8eb1efaf5bec5ea51735e15dfddd', '84b5770be474561d848b359da6b4084930412220b28c62080e04254f52d616fd']
expected = [[(153,3,116,44467),(1022,19,233,503)],[(2234,20,532,608273),(38142,140,1154,845)]]
for i,name in enumerate(('first','second')):
    compressed=Path(f'crates/sdocx/src/ink/pencil2/test_assets/drawing/independent-drawing/{name}.zlib').read_bytes()
    assert hashlib.sha256(compressed).hexdigest() == native_hashes[i]
    raw=zlib.decompress(compressed)
    assert len(raw) == 192*192*4
    native=Image.frombytes('RGBA',(192,192),raw).transpose(Image.Transpose.FLIP_TOP_BOTTOM).tobytes()
    for j,typ in enumerate(('vec','generic')):
        svg=(work/f'{typ}{i}.svg').read_bytes()
        assert hashlib.sha256(svg).hexdigest() == svg_hashes[i][j]
        with Image.open(work/f'{typ}{i}.png') as image:
            assert image.size == (192,192)
            pixels=image.convert('RGBA').tobytes()
        assert len(pixels) == len(native) == 192*192*4
        errors=[abs(a-b) for a,b in zip(pixels,native)]
        result=(sum(errors),max(errors),sum(bool(x) for x in errors),len(svg))
        assert result == expected[i][j], (i,typ,result)
        print(i,typ,'sum,max,channels,svg_bytes',result)
