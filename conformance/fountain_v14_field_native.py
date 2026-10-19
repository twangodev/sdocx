"""Replay embedded real Fountain V14 AddPoint stamps and shader fields.

Native RTV4::AddPoint executes from the hash-pinned ARM64 DSO. The recovered
V4 shaders then execute in Chromium SwiftShader with explicit GL_MAX composition.
This is a bounded vector-field oracle, not an Android/device pixel capture.
"""
from __future__ import annotations

import argparse
import base64
import copy
import hashlib
import json
import os
import shutil
import struct
import subprocess
import sys
import tempfile
import zlib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
FIXTURE = ROOT / 'conformance/fountain-v14-field-native.json'
LIBRARY_DIR = ROOT / 'scratch/apk-analysis-native/arm64-v8a'
OBJDUMP = ROOT / 'scratch/apk-analysis-runtime/binutils-aarch64/usr/bin/aarch64-linux-gnu-objdump'
PLAYWRIGHT = ROOT / 'web/node_modules/@playwright/test/index.mjs'
EXPECTED_INPUTS_SHA256 = '8f07c48d455305a24b3d788b0648d1c771d4dd80bddf91b0c6c2d38fd706fd88'
EXPECTED_SHADER_SHA256 = '26d7ac797b9a65f5e40ae6fe09007aefbbc98148314f9c9b336af4301cb05147'
EXPECTED_HELPER_SHA256 = '40cb917c7f286302d8519e9b953c3c21b7fe6ea0b6b0a80ccc13e2e724cfad30'
ADDPOINT = 'SPen::FountainPenStrokeDrawableRTV4::AddPoint(SPen::Vector3<float>, SPen::Vector2<float>, float)'
SHADER_SYMBOL_RVAS = {
    'vertex': 0x40c58,
    'fragment': 0x40f68,
}
SHADER_SYMBOLS = {
    'vertex': '_ZN4SPen25FountainPenStrokeShaderV414szVertexShaderE',
    'fragment': '_ZN4SPen25FountainPenStrokeShaderV416szFragmentShaderE',
}


def canonical_hash(value: object) -> str:
    return hashlib.sha256(json.dumps(value, sort_keys=True, separators=(',', ':')).encode()).hexdigest()


def load_native():
    sys.path.insert(0, str(ROOT / 'conformance'))
    sys.path.insert(0, str(ROOT / 'scratch/apk-analysis-runtime/python'))
    import fountain_native
    helper = ROOT / 'conformance/fountain_native.py'
    actual_helper_hash = hashlib.sha256(helper.read_bytes()).hexdigest()
    if actual_helper_hash != EXPECTED_HELPER_SHA256:
        raise RuntimeError(f'fountain_native.py helper changed: {actual_helper_hash}')
    return fountain_native.NativeFountain(LIBRARY_DIR, OBJDUMP), fountain_native, actual_helper_hash


def elf_virtual_bytes(path: Path, address: int, length: int) -> bytes:
    """Read initialized bytes at an ELF64 little-endian virtual address."""
    data = path.read_bytes()
    if data[:6] != b'\x7fELF\x02\x01':
        raise ValueError(f'unsupported ELF encoding: {path}')
    phoff = struct.unpack_from('<Q', data, 32)[0]
    phentsize, phnum = struct.unpack_from('<HH', data, 54)
    for index in range(phnum):
        header = struct.unpack_from('<IIQQQQQQ', data, phoff + index * phentsize)
        kind, _flags, file_offset, vaddr, _paddr, file_size, _mem_size, _align = header
        if kind == 1 and vaddr <= address and address + length <= vaddr + file_size:
            start = file_offset + address - vaddr
            return data[start:start + length]
    raise ValueError(f'ELF address {address:#x} is not file-backed in {path}')


def verify_shader_sources(fixture: dict) -> dict:
    """Bind embedded GLSL to exported DSO C strings and their constructor route."""
    path = LIBRARY_DIR / 'libSPenFountainPen.so'
    raw = path.read_bytes()
    symbol_lines = subprocess.check_output(['nm', '-D', str(path)], text=True).splitlines()
    exported_symbols = {}
    for line in symbol_lines:
        fields = line.split()
        if len(fields) == 3:
            exported_symbols[fields[2]] = (int(fields[0], 16), fields[1])
    sources = fixture['shader_sources']
    result = {}
    for name, rva in SHADER_SYMBOL_RVAS.items():
        if exported_symbols.get(SHADER_SYMBOLS[name]) != (rva, 'R'):
            raise ValueError(f'{name} V4 shader ELF symbol is absent or moved')
        # The symbol points at a NUL-terminated C string in the read-only segment.
        prefix = elf_virtual_bytes(path, rva, len(sources[name].encode('utf-8')) + 1)
        expected = sources[name].encode('utf-8') + b'\0'
        if prefix != expected:
            raise ValueError(f'{name} shader source does not match pinned DSO C string at {rva:#x}')
        # Map the RVA back to a file offset for an auditable byte-level locator.
        phoff = struct.unpack_from('<Q', raw, 32)[0]
        phentsize, phnum = struct.unpack_from('<HH', raw, 54)
        file_offset = None
        for index in range(phnum):
            kind, _flags, off, va, _pa, filesz, _memsz, _align = struct.unpack_from('<IIQQQQQQ', raw, phoff + index * phentsize)
            if kind == 1 and va <= rva < va + filesz:
                file_offset = off + rva - va
                break
        if file_offset is None:
            raise ValueError(f'{name} shader symbol RVA is not file-backed')
        result[name] = {
            'symbol': f'SPen::FountainPenStrokeShaderV4::sz{name.title()}Shader',
            'mangled_symbol': SHADER_SYMBOLS[name],
            'rva': hex(rva),
            'file_offset': hex(file_offset),
            'nul_terminated_byte_count': len(expected),
            'cstring_sha256': hashlib.sha256(expected).hexdigest(),
        }
    # V4 constructor loads these two GLOB_DAT-backed pointers then calls the
    # renderer factory with them as its first two source arguments.
    route = elf_virtual_bytes(path, 0xa34d0, 0x1c)
    if route.hex() != '800100f0810100f01f2003d502d8cc1000c840f921cc40f932a20094':
        raise ValueError('V4 shader constructor source-load/CreateShaderProgram sequence changed')
    result['native_constructor_route'] = {
        'constructor': 'SPen::FountainPenStrokeShaderV4::FountainPenStrokeShaderV4()',
        'constructor_rva': '0xa34ac',
        'source_load_instructions_rva': '0xa34d0',
        'source_got_slots_rva': ['0xd6190', '0xd6198'],
        'create_shader_program_call_rva': '0xa34e8',
        'call_target': 'SPen::RendererFactory::CreateShaderProgram(char const*, char const*, char const*)',
        'evidence': 'ELF bytes show two GOT loads and a direct PLT call; runtime replay verifies source C-string bytes, while the host WebGL compile is not a native CreateShader interception.',
    }
    return result


def native_addpoint(native, stamp: dict, scale: int) -> list[float]:
    native.heap = 0x10000000
    obj, vector, buffer = native.alloc(512), native.alloc(24), native.alloc(128)
    native.put(obj + 80, 'Q', vector)
    native.put(vector, 'QQQ', buffer, buffer, buffer + 128)
    inverse = 1.0 / scale
    native.put(obj + 264, 'ff', inverse, inverse)
    x, y = stamp['point']
    dx, dy = stamp['direction']
    native.call(ADDPOINT, [obj], [x, y, stamp['radius'], dx, dy, 1.0])
    start, end, capacity = native.get(vector, 'QQQ')
    if end - start != 9 * 4 or capacity < end:
        raise AssertionError(f'AddPoint emitted {end - start} bytes, expected one 9-float record')
    record = list(native.get(start, '9f'))
    if not all(map(lambda value: value == value and abs(value) != float('inf'), record)):
        raise AssertionError('AddPoint emitted non-finite record')
    return record


NODE_HARNESS = r'''import { readFile, writeFile } from 'node:fs/promises';
import { chromium } from __PLAYWRIGHT_URL__;
const input = JSON.parse(await readFile(process.argv[2], 'utf8'));
const browser = await chromium.launch({ args: ['--use-angle=swiftshader', '--enable-unsafe-swiftshader'] });
try {
  const result = await browser.newPage().then(page => page.evaluate(({ cases, runs, shader }) => {
    const canvas = document.createElement('canvas'); canvas.width = canvas.height = 64;
    const gl = canvas.getContext('webgl2', { antialias: false, premultipliedAlpha: false, preserveDrawingBuffer: true });
    if (!gl) throw new Error('WebGL2 unavailable');
    const compile = (type, source) => { const s = gl.createShader(type); gl.shaderSource(s, source); gl.compileShader(s); if (!gl.getShaderParameter(s, gl.COMPILE_STATUS)) throw new Error(gl.getShaderInfoLog(s)); return s; };
    const program = gl.createProgram(); gl.attachShader(program, compile(gl.VERTEX_SHADER, shader.vertex)); gl.attachShader(program, compile(gl.FRAGMENT_SHADER, shader.fragment)); gl.linkProgram(program);
    if (!gl.getProgramParameter(program, gl.LINK_STATUS)) throw new Error(gl.getProgramInfoLog(program));
    gl.bindVertexArray(gl.createVertexArray()); gl.useProgram(program);
    const attr = (loc, width, data) => { gl.bindBuffer(gl.ARRAY_BUFFER, gl.createBuffer()); gl.bufferData(gl.ARRAY_BUFFER, new Float32Array(data), gl.STATIC_DRAW); gl.enableVertexAttribArray(loc); gl.vertexAttribPointer(loc, width, gl.FLOAT, false, 0, 0); };
    attr(0, 2, [0,1, 0,0, 1,1, 1,0]); attr(1, 2, [1,-1, -1,1, 1,1, -1,-1]);
    gl.enable(gl.BLEND); gl.blendEquation(gl.MAX); gl.blendFunc(gl.ONE, gl.ONE);
    gl.uniform4f(gl.getUniformLocation(program, 'inputColor'), 1,1,1,1);
    const clamp = v => Math.max(0, Math.min(1, v)), f = Math.fround;
    const subpixelBits = gl.getParameter(gl.SUBPIXEL_BITS), grid = 2 ** subpixelBits, snap = v => Math.round(v * grid) / grid;
    const interp = (quad, x, y) => {
      const uv = [[0,1],[0,0],[1,1],[1,0]];
      for (const ids of [[0,1,2],[2,1,3]]) {
        const [i,j,k] = ids, [ax,ay] = quad[i], [bx,by] = quad[j], [cx,cy] = quad[k];
        const den = (by-cy)*(ax-cx)+(cx-bx)*(ay-cy); if (!den) continue;
        const wa = ((by-cy)*(x-cx)+(cx-bx)*(y-cy))/den, wb = ((cy-ay)*(x-cx)+(ax-cx)*(y-cy))/den, wc = 1-wa-wb;
        if (Math.min(wa,wb,wc) < -1e-9) continue;
        return [wa*uv[i][0]+wb*uv[j][0]+wc*uv[k][0]-.5, wa*uv[i][1]+wb*uv[j][1]+wc*uv[k][1]-.5];
      }
      return [1,1];
    };
    const captures = [], scaleRuns = runs;
    const rendererInfo = gl.getExtension('WEBGL_debug_renderer_info');
    const host = { vendor: gl.getParameter(gl.VENDOR), renderer: gl.getParameter(gl.RENDERER), version: gl.getParameter(gl.VERSION), shading_language: gl.getParameter(gl.SHADING_LANGUAGE_VERSION), unmasked_vendor: rendererInfo ? gl.getParameter(rendererInfo.UNMASKED_VENDOR_WEBGL) : null, unmasked_renderer: rendererInfo ? gl.getParameter(rendererInfo.UNMASKED_RENDERER_WEBGL) : null, subpixel_bits: subpixelBits, context_attributes: gl.getContextAttributes() };
    if (!String(host.unmasked_renderer || host.renderer).toLowerCase().includes('swiftshader')) throw new Error('renderer is not SwiftShader: ' + JSON.stringify(host));
    for (const run of scaleRuns) {
      const c = cases.find(x => x.name === run.case); if (!c) throw new Error('unknown case');
      const scale = run.scale, anchor = c.stamps[0].point, quads = [];
      gl.viewport(0,0,64,64); gl.clearColor(0,0,0,0); gl.clear(gl.COLOR_BUFFER_BIT);
      const matrix = [scale/32,0,0,0, 0,-scale/32,0,0, 0,0,1,0, -scale,scale,0,1];
      gl.uniformMatrix4fv(gl.getUniformLocation(program, 'ProjectionMatrix'), false, new Float32Array(matrix));
      for (let i=0;i<c.stamps.length;i++) {
        const stamp = c.stamps[i], a = run.records[i], [x,y] = stamp.point, [dx,dy] = stamp.direction;
        const docX=32+(x-anchor[0]), docY=32+(y-anchor[1]);
        gl.vertexAttrib4f(2, docX, docY, a[2], a[3]); gl.vertexAttrib2f(3, a[4], a[5]); gl.vertexAttrib1f(4, a[6]); gl.vertexAttrib2f(5, a[7], a[8]);
        gl.drawArrays(gl.TRIANGLE_STRIP, 0, 4);
        const px=32+(x-anchor[0])*scale, py=32-(y-anchor[1])*scale;
        const quad=[[0,1],[0,0],[1,1],[1,0]].map(([u,v])=>{
          const ex=f(f((2*v-1)*a[4])+f((2*u-1)*a[5])), ey=f(f((2*v-1)*a[5])-f((2*u-1)*a[4]));
          return [snap(f(px+f(ex*a[2]*scale))), snap(f(py-f(ey*a[3]*scale)))];
        });
        quads.push({a,quad});
      }
      const gpuBottomUp=new Uint8Array(64*64*4); gl.readPixels(0,0,64,64,gl.RGBA,gl.UNSIGNED_BYTE,gpuBottomUp);
      const glError=gl.getError(); if(glError!==gl.NO_ERROR) throw new Error('GL error '+glError);
      const rgba=new Uint8Array(64*64*4), scalarAlphaBottomUp=new Uint8Array(64*64);
      for(let y=0;y<64;y++) for(let x=0;x<64;x++) {
        let alpha=0;
        for(const{a,quad}of quads){
          const[u,v]=interp(quad,x+.5,y+.5),dist=Math.hypot(u,v); if(Math.abs(u)>=.5||Math.abs(v)>=.5)continue;
          const distY=Math.abs(v), grad=clamp(1-.93/.25*(.25-(.5-distY)));
          const edge=dist>a[7]?1-(dist-a[7])/(.5-a[7]):1;
          alpha=Math.max(alpha,clamp(a[6]*grad*edge*a[8]));
        }
        const index=y*64+x, dest=((63-y)*64+x), source=index*4, byte=Math.round(alpha*255); scalarAlphaBottomUp[index]=byte;
        rgba.set(gpuBottomUp.subarray(source,source+4),dest*4);
      }
      const scalarRgba=new Uint8Array(64*64*4); for(let y=0;y<64;y++)for(let x=0;x<64;x++){const out=((63-y)*64+x)*4,a=scalarAlphaBottomUp[y*64+x];scalarRgba.set([a,a,a,a],out);}
      const gpuAlpha=Array.from({length:4096},(_,i)=>gpuBottomUp[i*4+3]), scalarBottom=Array.from(scalarAlphaBottomUp), errors=gpuAlpha.map((v,i)=>Math.abs(v-scalarBottom[i]));
      captures.push({case:run.case,scale,rgba8:Array.from(rgba),scalar_rgba8:Array.from(scalarRgba),max_shader_vs_scalar_byte_error:Math.max(...errors),mean_shader_vs_scalar_byte_error:errors.reduce((a,b)=>a+b,0)/errors.length,active_shader_vs_scalar_mean_abs_error:errors.filter((_,i)=>gpuAlpha[i]||scalarBottom[i]).reduce((a,b)=>a+b,0)/(errors.filter((_,i)=>gpuAlpha[i]||scalarBottom[i]).length||1),gl_error:glError});
    }
  return {host,captures};
  }, input));
  result.browser_version = browser.version();
  result.browser_executable = chromium.executablePath();
  result.node_version = process.versions.node;
  await writeFile(process.argv[3], JSON.stringify(result));
} finally { await browser.close(); }
'''


def run_shader(cases: list[dict], native_runs: list[dict], shaders: dict) -> dict:
    payload = {'cases': cases, 'runs': native_runs, 'shader': {'vertex': shaders['vertex'], 'fragment': shaders['fragment']}}
    with tempfile.TemporaryDirectory(prefix='sdocx-fountain-v14-field-') as directory:
        td = Path(directory)
        inp, out, js = td / 'input.json', td / 'output.json', td / 'runner.mjs'
        inp.write_text(json.dumps(payload))
        module_url = PLAYWRIGHT.resolve().as_uri()
        js.write_text(NODE_HARNESS.replace('__PLAYWRIGHT_URL__', json.dumps(module_url)))
        completed = subprocess.run(['node', str(js), str(inp), str(out)], cwd=ROOT, text=True, capture_output=True)
        if completed.returncode:
            raise RuntimeError(f'SwiftShader replay failed ({completed.returncode}): {completed.stderr[-5000:]}')
        return json.loads(out.read_text())


def compressed_payload(raw: bytes) -> dict:
    return {'byte_count': len(raw), 'sha256': hashlib.sha256(raw).hexdigest(), 'zlib_base64': base64.b64encode(zlib.compress(raw, 9)).decode('ascii')}


def file_identity(path: str | Path | None) -> dict | None:
    if path is None:
        return None
    resolved = Path(path).resolve()
    return {'path': str(resolved), 'sha256': hashlib.sha256(resolved.read_bytes()).hexdigest()}


def host_toolchain(rendered: dict) -> dict:
    node_path = shutil.which('node')
    nm_path = shutil.which('nm')
    playwright_package = ROOT / 'web/node_modules/@playwright/test/package.json'
    playwright_meta = json.loads(playwright_package.read_text())
    runtime = ROOT / 'scratch/apk-analysis-runtime/binutils-aarch64/usr/lib/x86_64-linux-gnu'
    env = dict(os.environ)
    if runtime.is_dir():
        env['LD_LIBRARY_PATH'] = str(runtime.resolve())
    objdump_version = subprocess.check_output([str(OBJDUMP), '--version'], text=True, env=env).splitlines()[0]
    nm_version = subprocess.check_output([nm_path, '--version'], text=True).splitlines()[0] if nm_path else None
    return {
        'node_version': rendered['node_version'],
        'node_executable': file_identity(node_path),
        'chromium_version': rendered['browser_version'],
        'chromium_executable': file_identity(rendered['browser_executable']),
        'playwright_test_version': playwright_meta['version'],
        'playwright_entrypoint': file_identity(PLAYWRIGHT),
        'playwright_package_metadata': file_identity(playwright_package),
        'nm_version': nm_version,
        'nm_executable': file_identity(nm_path),
        'objdump_version': objdump_version,
        'objdump_executable': file_identity(OBJDUMP),
    }


def replay(fixture: dict, verify_saved_evidence: bool = True) -> tuple[list[dict], dict, dict]:
    if fixture.get('schema') != 'sdocx-fountain-v14-field-native-v1':
        raise ValueError('unexpected fixture schema')
    if canonical_hash(fixture['source_cases']) != EXPECTED_INPUTS_SHA256:
        raise ValueError('embedded real-stroke input triples changed')
    shaders = fixture['shader_sources']
    if canonical_hash(shaders) != EXPECTED_SHADER_SHA256:
        raise ValueError('embedded V4 shaders do not match the pinned shader payload')
    shader_evidence = verify_shader_sources(fixture)
    if verify_saved_evidence and fixture.get('shader_sources_pinned_dso_evidence') not in (None, shader_evidence):
        raise ValueError('shader source DSO byte evidence differs from fixture')
    native, fountain_module, helper_hash = load_native()
    for name, digest in fixture['native_library_sha256'].items():
        if fountain_module.HASHES.get(name) != digest:
            raise ValueError(f'unexpected pinned DSO hash for {name}')
    if native.syms.get(ADDPOINT) != 0x9de50:
        raise ValueError(f'RTV4::AddPoint RVA changed: {native.syms.get(ADDPOINT)!r}')
    runs = []
    for case in fixture['source_cases']:
        for scale in (1, 2, 4):
            recs = [native_addpoint(native, stamp, scale) for stamp in case['stamps']]
            packed = b''.join(struct.pack('<9f', *record) for record in recs)
            diagnostics = []
            for stamp, record in zip(case['stamps'], recs):
                outer_px = record[2] * scale
                inner_px = 2 * record[7] * record[2] * scale
                diagnostics.append({'outer_extent_device_px': outer_px, 'inner_radius_device_px': inner_px, 'radial_transition_width_device_px': outer_px-inner_px})
            runs.append({'case': case['name'], 'scale': scale, 'records': recs, 'packed_records_sha256': hashlib.sha256(packed).hexdigest(), 'radial_extent_diagnostics': diagnostics})
    rendered = run_shader(fixture['source_cases'], runs, shaders)
    if len(rendered['captures']) != len(runs):
        raise AssertionError('SwiftShader did not return every case/scale capture')
    captures=[]
    for run, pixels in zip(runs, rendered['captures']):
        if pixels['case'] != run['case'] or pixels['scale'] != run['scale']:
            raise AssertionError('SwiftShader capture order does not match native AddPoint runs')
        if pixels['max_shader_vs_scalar_byte_error'] > 1:
            rgba=bytes(pixels['rgba8']); scalar=bytes(pixels['scalar_rgba8']); errs=sorted(range(4096),key=lambda i:abs(rgba[i*4+3]-scalar[i*4+3]),reverse=True)[:5]
            top=[{'xy':[i%64,i//64],'gpu':rgba[i*4+3],'scalar':scalar[i*4+3]} for i in errs]
            raise AssertionError(f"{run['case']} scale {run['scale']} shader/scalar mismatch: {pixels['max_shader_vs_scalar_byte_error']} {top}")
        rgba=bytes(pixels['rgba8']); scalar_rgba=bytes(pixels['scalar_rgba8'])
        if len(rgba)!=64*64*4 or len(scalar_rgba)!=64*64*4:
            raise AssertionError('shader/scalar readback is not a full RGBA8 viewport')
        if any(rgba[i] != rgba[i+1] or rgba[i] != rgba[i+2] or rgba[i] != rgba[i+3] for i in range(0,len(rgba),4)):
            raise AssertionError('white shader and GL_MAX replay did not preserve equal RGBA channels')
        captures.append({'case':run['case'],'scale':run['scale'],'inverse_scale_document_units_per_pixel':[1/run['scale'],1/run['scale']], 'native_AddPoint_record_f32_le_hex_words':[[struct.pack('<f',v).hex() for v in rec] for rec in run['records']], 'native_packed_records_sha256':run['packed_records_sha256'], 'record_count':len(run['records']), 'radial_extent_diagnostics':run['radial_extent_diagnostics'], 'shader_rgba8':compressed_payload(rgba), 'scalar_rgba8':compressed_payload(scalar_rgba), 'max_shader_vs_scalar_byte_error':pixels['max_shader_vs_scalar_byte_error'], 'mean_shader_vs_scalar_byte_error':pixels['mean_shader_vs_scalar_byte_error'], 'active_shader_vs_scalar_mean_abs_error':pixels['active_shader_vs_scalar_mean_abs_error'], 'gl_error':pixels['gl_error']})
    host = {'webgl': rendered['host'], 'toolchain': host_toolchain(rendered)}
    return captures, host, shader_evidence


def main() -> None:
    parser=argparse.ArgumentParser(description=__doc__)
    mode=parser.add_mutually_exclusive_group()
    mode.add_argument('--write-fixture',action='store_true',help='replay and explicitly replace the fixture captures')
    mode.add_argument('--check-fixture',action='store_true',help='replay and compare with the checked-in fixture')
    parser.add_argument('--fixture',type=Path,default=FIXTURE)
    args=parser.parse_args()
    fixture=json.loads(args.fixture.read_text())
    if canonical_hash(fixture['source_cases'])!=EXPECTED_INPUTS_SHA256 or canonical_hash(fixture['shader_sources'])!=EXPECTED_SHADER_SHA256:
        raise SystemExit('fixture source inputs or shaders differ from the runner-pinned values')
    captures, host, shader_evidence=replay(fixture, verify_saved_evidence=not args.write_fixture)
    if args.write_fixture:
        fixture['host_gl']=host
        fixture['shader_sources_pinned_dso_evidence']=shader_evidence
        fixture['runner_sha256']=hashlib.sha256(Path(__file__).read_bytes()).hexdigest()
        fixture['captures']=captures
        fixture['host_boundaries']=['Native RTV4::AddPoint executes from hash-pinned ARM64 libSPenFountainPen.so through the tracked fountain_native Unicorn loader; source point/radius/direction triples are embedded prepared real V14 stamps.','Recovered unmodified V4 vertex/fragment shaders execute in Chromium SwiftShader WebGL2. GL_MAX with ONE/ONE, white input color, 64x64 target and viewport projection are explicit host replay settings.','A scalar fragment-center reference models the native GLSL and GL_MAX composite; the WebGL target is not Samsung device GLES and does not claim device pixels, app canvas projection, stroke-wide public export parity, or PDF backend parity.']
        args.fixture.write_text(json.dumps(fixture,indent=2)+'\n')
    else:
        if fixture.get('host_gl')!=host:
            raise SystemExit('host SwiftShader identity differs from fixture')
        if fixture.get('shader_sources_pinned_dso_evidence')!=shader_evidence:
            raise SystemExit('embedded shader source no longer matches pinned DSO symbols/constructor route')
        if fixture.get('runner_sha256')!=hashlib.sha256(Path(__file__).read_bytes()).hexdigest():
            raise SystemExit('runner hash differs from fixture')
        if fixture.get('captures')!=captures:
            raise SystemExit('native AddPoint or SwiftShader field readback differs from fixture')
    print(json.dumps({'mode':'write' if args.write_fixture else 'check','cases':len(fixture['source_cases']),'runs':len(captures),'host_gl':host,'max_scalar_shader_error':max(row['max_shader_vs_scalar_byte_error'] for row in captures)},indent=2))


if __name__=='__main__':
    main()
