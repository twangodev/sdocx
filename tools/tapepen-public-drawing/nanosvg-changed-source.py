#!/usr/bin/env python3
"""Replay the pinned SVG through the shipped native rasterizer."""
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile

if not __debug__:
    raise SystemExit('Python optimization disables required replay checks')
if sys.argv[1:] != ['--run', '--shape64', '--size12']:
    raise SystemExit('expected --run --shape64 --size12')
probe = Path(os.environ['NSVG_PROBE']).resolve(strict=True)
here = Path(__file__).resolve().parent
receipt = json.loads((here / 'nanosvg-source-receipt.json').read_text())
pins = {'capture_candidate.py': 'f39dcc671e7ae49ca833f7c1273ed05fba6c417f12ae21e742982346e070476f', 'preflight_candidate.py': '1e0985967e9e26c97d311c96979fe7009ad3d741c955fda1bb77436ef916b8a9', 'tapepen_canvas_native.py': 'cf08a72c39f2fa8f8ad463a8ca87596adbfb9649453ad53cb3e763099c385a97', 'preflight.json': 'dec9fcea51fb776d3b0394ea812a80cd8dd12e2964a85c468dde9bfde82d75e9', 'candidate.json': '55a7535e13c9e41900fb98db556b74a2d6996f7ce6423ee59a6e859fefd83747', 'phase-control.patch': '3b6a85f816a4bd71f8151d731b0469a00b614fbac5f7741267228491f48c089f', 'rotation-v113.patch': '9b27f653d7d334ab5154e3a9ad75056d4a6b3d6c1e0f3a650bbe5ad869e0416e'}
for name, expected in pins.items():
    actual = hashlib.sha256((here / name).read_bytes()).hexdigest()
    if actual != expected:
        raise RuntimeError(f"capture input {name} hash mismatch: {actual}")
def replace_once(text, old, new):
    count = text.count(old)
    if count != 1:
        raise RuntimeError(f"expected exactly one replacement, got {count}: {old}")
    return text.replace(old, new, 1)
with tempfile.TemporaryDirectory(prefix='sdocx-tape-phase-') as temp:
    root = Path(temp)
    for name in ('capture_candidate.py', 'preflight_candidate.py', 'tapepen_canvas_native.py', 'preflight.json', 'candidate.json'):
        shutil.copyfile(here / name, root / name)
    subprocess.run(['patch', '-s', str(root / 'capture_candidate.py'), str(here / 'phase-control.patch')], check=True)
    tracing_size = '--trace-size' in sys.argv
    if tracing_size:
        subprocess.run(['patch', '-s', str(root / 'capture_candidate.py'), str(here / 'size-level-trace.patch')], check=True)
    subprocess.run(['patch', '-s', str(root / 'capture_candidate.py'), str(here / 'rotation-v113.patch')], check=True)
    runner = root / 'capture_candidate.py'
    script = runner.read_text()
    script = replace_once(script, '[[0.,0.],[9.,8.],[21.,-3.],[34.,7.]]', '[[0.,0.],[12.,-7.],[24.,6.],[32.,-2.]]')
    script = replace_once(script, '0x8044aaee', '0xc0d74433')
    script = replace_once(script, 'call_address(run_method,[task],limit=5_000_000)', 'call_address(run_method,[task],limit=50_000_000)')
    injection = """
output_dir=OUTPUT_DIR
cases={
 'color':(b'#3F5480',b'#AD2244'),
 'geometry':(b'stroke-width="1.7"',b'stroke-width="2.3"'),
}
for case,(old,new) in cases.items():
 assert tape_pattern_bytes.count(old)==136
 data=tape_pattern_bytes.replace(old,new)
 assert data.count(old)==0 and data.count(new)==136
 ptr=p.alloc(len(data));p.u.mem_write(ptr,data)
 p.call('SPen::SetTapePenPatternSvgUtf8Bytes(int, char const*, unsigned long)',[1,ptr,len(data)])
 open(output_dir+'/nsvg-'+case+'.svg','wb').write(data)
 for dimension in (64,128):
  vec=p.alloc(24);p.u.mem_write(vec,bytes(24))
  p.call('SPen::RasterizeTapePenPatternFromSvg(int, int, int, std::__ndk1::vector<unsigned int, std::__ndk1::allocator<unsigned int> >&)',[1,dimension,dimension,vec],limit=50000000)
  beg,end,cap=struct.unpack('<QQQ',p.u.mem_read(vec,24))
  assert beg and end-beg==dimension*dimension*4 and cap>=end
  open(output_dir+'/nsvg-'+case+'-'+str(dimension)+'.native','wb').write(bytes(p.u.mem_read(beg,end-beg)))
  builtins.print('captured changed source',case,dimension,flush=True)
raise SystemExit(0)
""".replace('OUTPUT_DIR', repr(str(root)))

    script = replace_once(script, "tape_pattern_store_call={'api':", injection + "\ntape_pattern_store_call={'api':")
    runner.write_text(script)
    digest = hashlib.sha256(runner.read_bytes()).hexdigest()

    manifest = root / 'preflight.json'
    data = json.loads(manifest.read_text())
    data['runner_sha256'] = digest
    manifest.write_text(json.dumps(data, indent=2) + '\n')
    manifest_digest = hashlib.sha256(manifest.read_bytes()).hexdigest()

    args = ['--preflight-sha256', manifest_digest, '--translate', '13', '--translate-y', '21', '--width', '512', '--height', '512', '--compact']
    candidate = root / 'candidate.json'
    data = json.loads(candidate.read_text())
    data['run_command'] = subprocess.list2cmdline([sys.executable, str(runner), *args])
    candidate.write_text(json.dumps(data, indent=2) + '\n')
    subprocess.run([sys.executable, str(root / 'preflight_candidate.py')], check=True)
    if '--run' in sys.argv:
        if '--sweep-size' in sys.argv:
            args.extend(['--size', sys.argv[sys.argv.index('--sweep-size') + 1]])
        if '--size4' in sys.argv:
            args.extend(['--size', '4'])
        if '--size12' in sys.argv:
            args.extend(['--size', '12'])
        if '--shape' in sys.argv:
            args.append('--shape-control')
        if '--shape64' in sys.argv:
            args[args.index('13')] = '77'
            args[args.index('21')] = '59'
            args.append('--shape-control')
        subprocess.run([sys.executable, str(runner), *args], check=True)

    for case, expected in receipt['cases'].items():
        source = root / f'nsvg-{case}.svg'
        assert hashlib.sha256(source.read_bytes()).hexdigest() == expected['source_sha256']
        for size, digest in expected['rgba_sha256'].items():
            native = (root / f'nsvg-{case}-{size}.native').read_bytes()
            assert hashlib.sha256(native).hexdigest() == digest
            host = subprocess.check_output([str(probe), str(source), size])
            assert host == native, f'{case} {size} differs from native'
            print(f'{case} {size}: native and upstream match {digest}', flush=True)
