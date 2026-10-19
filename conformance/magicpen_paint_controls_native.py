#!/usr/bin/env python3
"""Capture and verify native MagicPen RTV1 paint across view and version controls.

The pinned MagicPen Drawing-to-Mesa runner is adapted only in memory. Normal
invocation replays every control and checks this fixture. Updating the fixture
requires the explicit --write-fixture option.
"""
import argparse
import ast
import base64
import contextlib
import hashlib
import io
import json
import math
import os
import struct
import subprocess
import sys
import zlib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
BASE_RUNNER = ROOT / 'conformance/magicpen_drawing_paint_native.py'
BASE_FIXTURE = ROOT / 'conformance/magicpen-drawing-paint-native.json'
FIXTURE = ROOT / 'conformance/magicpen-paint-controls-native.json'
BASE_RUNNER_SHA256 = 'b87400d5508b9717b05a6413d5f1824ab56de9d7156c059a6f69ea3e3b21bbdd'
BASE_FIXTURE_SHA256 = '874f659069c8ab206c049f4e7c7161524146f55a93c44fd4ac378bba6e5c9bc2'
BASE_MATRIX = "mat=p.alloc(64);p.put(mat,'16f',1,0,0,0,0,1,0,0,0,0,1,0,translation[0],translation[1],0,1)"
FIXED_CENTER_ASSERT = "expected_stroke_sample=([26,51,77,128] if background_kind=='transparent' else [58,115,173,255])\nassert rgba_at(64,64)==expected_stroke_sample, f'partial-alpha source-over pixel changed: {rgba_at(64,64)}'"

# This is the same explicit geometry/control set used by the reviewed scratch
# experiment. Version identity is captured at the unit transform.
VIEW_CONTROLS = [
    {'id':'view-s0.5-phase0','scale':[0.5,0.5],'phase':[0.0,0.0]},
    {'id':'view-s1-phase0','scale':[1.0,1.0],'phase':[0.0,0.0]},
    {'id':'view-s1-phase-quarter','scale':[1.0,1.0],'phase':[0.25,0.25]},
    {'id':'view-s1-phase-half-quarter','scale':[1.0,1.0],'phase':[0.5,0.25]},
    {'id':'view-s2-phase0','scale':[2.0,2.0],'phase':[0.0,0.0]},
    {'id':'view-s4-phase-eighth-quarter','scale':[4.0,4.0],'phase':[0.125,0.25]},
    {'id':'view-aniso-x0.5-y2','scale':[0.5,2.0],'phase':[0.0,0.0]},
    {'id':'view-aniso-x2-y0.5','scale':[2.0,0.5],'phase':[0.25,0.125]},
]
CAPTURES = ([{'kind':'view','version':'v2','background':'transparent',**c} for c in VIEW_CONTROLS]
            + [{'kind':'version','version':'v1','background':bg,'id':f'identity-v1-{bg}','scale':[1.0,1.0],'phase':[0.0,0.0]} for bg in ('transparent','opaque')]
            + [{'kind':'version','version':'v2','background':'opaque','id':'identity-v2-opaque','scale':[1.0,1.0],'phase':[0.0,0.0]}])


def sha(raw):
    return hashlib.sha256(raw).hexdigest()


def read_hash_literal(tree, name):
    for node in tree.body:
        if isinstance(node, ast.Assign) and any(isinstance(t, ast.Name) and t.id == name for t in node.targets):
            return ast.literal_eval(node.value)
    raise RuntimeError(f'could not read pinned {name} from base wrapper')


def preflight():
    base_raw = BASE_RUNNER.read_bytes()
    if sha(base_raw) != BASE_RUNNER_SHA256:
        raise SystemExit(f'pinned base wrapper changed: {sha(base_raw)}')
    if sha(BASE_FIXTURE.read_bytes()) != BASE_FIXTURE_SHA256:
        raise SystemExit(f'pinned base fixture changed: {sha(BASE_FIXTURE.read_bytes())}')
    tree = ast.parse(base_raw, filename=str(BASE_RUNNER))
    helper_hashes = read_hash_literal(tree, 'HELPER_HASHES')
    dso_hashes = read_hash_literal(tree, 'DSO_HASHES')
    tool_hashes = read_hash_literal(tree, 'TOOL_HASHES')
    for rel, expected in helper_hashes.items():
        actual = sha((ROOT / rel).read_bytes())
        if actual != expected:
            raise SystemExit(f'pinned helper changed: {rel}: {actual}')
    libdir = ROOT / 'scratch/apk-analysis-native/arm64-v8a'
    for name, expected in dso_hashes.items():
        actual = sha((libdir / name).read_bytes())
        if actual != expected:
            raise SystemExit(f'pinned native library changed: {name}: {actual}')
    for path, expected in tool_hashes.items():
        actual = sha(Path(path).read_bytes())
        if actual != expected:
            raise SystemExit(f'pinned tool/host library changed: {path}: {actual}')
    return {
        'base_runner_path':'conformance/magicpen_drawing_paint_native.py',
        'base_runner_sha256':BASE_RUNNER_SHA256,
        'base_fixture_path':'conformance/magicpen-drawing-paint-native.json',
        'base_fixture_sha256':BASE_FIXTURE_SHA256,
        'helper_sha256':helper_hashes,
        'native_dso_sha256':dso_hashes,
        'tool_and_host_graphics_sha256':tool_hashes,
    }


def patch_once(source, old, new, label):
    count = source.count(old)
    if count != 1:
        raise RuntimeError(f'{label}: expected one exact adaptation anchor, found {count}')
    return source.replace(old, new, 1)


def capture_one(control):
    source = BASE_RUNNER.read_text()
    sx, sy = control['scale']
    px, py = control['phase']
    source = patch_once(source, 'translation=(64.0,64.0)',
                        f'translation=(64.0+{px!r},64.0+{py!r})', 'view phase')
    matrix = (f"view_scale={sx!r};view_scale_y={sy!r};mat=p.alloc(64);"
              "p.put(mat,'16f',view_scale,0,0,0,0,view_scale_y,0,0,0,0,1,0,translation[0],translation[1],0,1)")
    source = patch_once(source, BASE_MATRIX, matrix, 'native Draw view matrix')
    source = patch_once(source, FIXED_CENTER_ASSERT,
                        "# coverage is compared over the full captured RGBA8 buffer", 'fixed center assertion')
    if control['version'] == 'v1':
        source = patch_once(source, 'record=records[-1]',
                            "record=next(row for row in records if row['case']=='v1')", 'GLV1 source selection')
    elif control['version'] != 'v2':
        raise RuntimeError(f"unsupported native drawable version: {control['version']}")

    env = os.environ.copy()
    paths = [str(ROOT/'conformance'), str(ROOT/'scratch/apk-analysis-runtime/python')]
    env['PYTHONPATH'] = os.pathsep.join(paths + [env.get('PYTHONPATH','')]).rstrip(os.pathsep)
    env['LD_LIBRARY_PATH'] = os.pathsep.join([str(ROOT/'scratch/apk-analysis-runtime/binutils-aarch64/usr/lib/x86_64-linux-gnu'), env.get('LD_LIBRARY_PATH','')]).rstrip(os.pathsep)
    old_argv = sys.argv[:]
    sys.argv = [str(BASE_RUNNER), '--_capture-background', control['background']]
    output = io.StringIO()
    try:
        with contextlib.redirect_stdout(output):
            exec(compile(source, str(BASE_RUNNER), 'exec'), {'__name__':'__main__','__file__':str(BASE_RUNNER)})
    finally:
        sys.argv = old_argv
    value = json.loads(output.getvalue())
    source_capture = value['source_capture']
    paint = value['paint']
    row_name = 'v1' if control['version'] == 'v1' else 'v2'
    selected = next(r for r in source_capture['cases'] if r['case'] == row_name)
    if selected['selected_drawable_class'] != ('GLV1' if row_name == 'v1' else 'GLV2'):
        raise RuntimeError(f"native source did not select expected GL drawable for {control['id']}")
    if paint['source_case'] != row_name:
        raise RuntimeError(f"paint joined unexpected native source case for {control['id']}: {paint['source_case']}")
    validate_paint(control, paint, selected)
    return source_capture, paint


def validate_paint(control, paint, selected):
    sx, sy = control['scale']
    px, py = control['phase']
    if paint['target_size'] != [256,256] or paint['readback']['bytes'] != 256*256*4:
        raise RuntimeError(f"unexpected readback shape for {control['id']}")
    if paint['view_translation'] != [64.0+px,64.0+py]:
        raise RuntimeError(f"view translation mismatch for {control['id']}")
    if paint['native_vector_sha256'] != selected['native_rtv1_vector']['sha256']:
        raise RuntimeError(f"native Draw upload source differs from selected Drawing vector for {control['id']}")
    if paint['native_vector_bytes'] != 96:
        raise RuntimeError(f"unexpected native RTV1 vector length for {control['id']}")
    query = paint['framebuffer_bindings_before_readpixels']
    if query['draw_pname_GL_DRAW_FRAMEBUFFER_BINDING'] != 0x8CA6 or query['read_pname_GL_READ_FRAMEBUFFER_BINDING'] != 0x8CAA:
        raise RuntimeError('readback framebuffer query pnames changed')
    if query['draw_value'] != query['read_value'] or query['read_value'] != paint['draw_calls'][-1]['state']['framebuffer']:
        raise RuntimeError(f"actual draw/read framebuffer queries disagree for {control['id']}")
    if not paint['framebuffer_completeness']['all_complete'] or paint['operation_errors'] or paint['gl_errors_final']:
        raise RuntimeError(f"native GLES paint produced an incomplete/error state for {control['id']}")
    if not paint['readback']['rgba8_zlib_base64']:
        raise RuntimeError('full RGBA8 readback payload missing')
    rgba = zlib.decompress(base64.b64decode(paint['readback']['rgba8_zlib_base64']))
    if len(rgba) != 256*256*4 or sha(rgba) != paint['readback']['sha256']:
        raise RuntimeError(f"full readback bytes/hash mismatch for {control['id']}")


def shader_law(source_capture, paint):
    mask = '_ZN4SPen18MagicPenMaskShader16szFragmentShaderE'
    composite = '_ZN4SPen23MagicPenCompositeShader16szFragmentShaderE'
    captured = '\n'.join('\n'.join(x.get('source',[])) for x in paint['shader_source_calls'])
    if 'float pixelDist = fwidth(dist) * 1.0' not in captured:
        raise RuntimeError('captured Mask shader no longer has the pinned fwidth law')
    if '1.0 - clamp((dist - (radius - pixelDist)) / pixelDist, 0.0, 1.0)' not in captured:
        raise RuntimeError('captured Mask shader coverage expression changed')
    if 'strokeAlpha = texture(StrokeTexture, vUV).a' not in captured or 'strokeAlpha * inputColor.a' not in captured:
        raise RuntimeError('captured composite shader no longer applies saved alpha once')
    return {'mask_shader_symbol':mask,'composite_shader_symbol':composite,
            'mask_expression':'clamp((0.5 - d) / fwidth(d), 0, 1)',
            'distance':'d = length(fragCoord - (0.5, 0.5)); stamp dimensions derive from native stored radius and Draw view matrix',
            'fwidth':'GL 2x2 derivative-pair differences, absolute x difference plus absolute y difference',
            'stamp_combine':'GL_MAX across native stamp instances, then sampled mask alpha times saved ObjectStroke alpha in composite',
            'captured_shader_sources_are_runtime_strings_from_pinned_dso':True}


def scalar_compare(control, source_capture, paint):
    """Compare the DSO-derived fwidth law to every final RGBA8 pixel."""
    if control['kind'] != 'view' and control['id'] not in ('identity-v1-transparent','identity-v1-opaque','identity-v2-opaque'):
        raise RuntimeError('unknown scalar comparison control')
    selected = next(r for r in source_capture['cases'] if r['case'] == control['version'])
    raw = bytes.fromhex(selected['native_rtv1_vector']['bytes_le_hex'])
    stamps = [struct.unpack_from('<3f',raw,i) for i in range(0,len(raw),12)]
    pixels_meta=paint['readback']
    actual=zlib.decompress(base64.b64decode(pixels_meta['rgba8_zlib_base64']))
    width,height=pixels_meta['width'],pixels_meta['height']
    sx,sy=control['scale'];tx,ty=paint['view_translation']
    ink=paint['native_rt_rgba_f32'];bg=paint['target_initial_background']['rgba8']
    errors=[];channel_errors=[[],[],[],[]];false_positive=false_negative=0;support=0;max_active=0
    initial=paint['target_initial_background']['rgba8']
    opaque_support_diagnostic=initial[3] == 255
    for y in range(height):
        for x in range(width):
            predicted_coverage=0.0
            for point_x,point_y,radius in stamps:
                cx=tx+point_x*sx;cy=ty+point_y*sy
                if abs((x+0.5)-cx)>radius*sx or abs((y+0.5)-cy)>radius*sy:
                    continue
                px=x+0.5;py=y+0.5
                dxden=2.0*radius*sx;dyden=2.0*radius*sy
                ux=(px-cx)/dxden;uy=(py-cy)/dyden
                d=math.hypot(ux,uy)
                xn=x+(1 if (x&1)==0 else -1)
                yn=y+(1 if (y&1)==0 else -1)
                dx=math.hypot((xn+0.5-cx)/dxden,uy)
                dy=math.hypot(ux,(yn+0.5-cy)/dyden)
                fw=abs(dx-d)+abs(dy-d)
                cov=1.0 if fw==0 and d<0.5 else (0.0 if fw==0 else max(0.0,min(1.0,(0.5-d)/fw)))
                predicted_coverage=max(predicted_coverage,cov)
            mask8=round(predicted_coverage*255)
            source_alpha=(mask8/255.0)*ink[3]
            predicted=[round((ink[k]*source_alpha+(bg[k]/255.0)*(1.0-source_alpha))*255) for k in range(3)]
            predicted.append(round((source_alpha+(bg[3]/255.0)*(1.0-source_alpha))*255))
            idx=(y*width+x)*4
            observed=list(actual[idx:idx+4])
            pixel_errors=[abs(observed[k]-predicted[k]) for k in range(4)]
            errors.extend(pixel_errors)
            for channel,error in enumerate(pixel_errors):channel_errors[channel].append(error)
            if opaque_support_diagnostic:
                # Diagnostic only: opaque source-over may hide low coverage in
                # RGB rounding, so support is not an exact oracle in this case.
                observed_active=max(abs(observed[k]-initial[k]) for k in range(4))>1
                predicted_active=max(abs(predicted[k]-initial[k]) for k in range(4))>1
            else:
                observed_active=observed[3]!=initial[3]
                predicted_active=predicted[3]!=initial[3]
            if observed_active or predicted_active:
                support+=1;max_active=max(max_active,max(pixel_errors))
            if not observed_active and predicted_active: false_positive+=1
            if observed_active and not predicted_active: false_negative+=1
    if not opaque_support_diagnostic and (false_positive or false_negative):
        raise RuntimeError(f"shader scalar support mismatch for {control['id']}: FP={false_positive} FN={false_negative}")
    mae=sum(errors)/len(errors)
    if max(errors)>3:
        raise RuntimeError(f"shader scalar RGBA error exceeded 3-byte bounded precision for {control['id']}: {max(errors)}")
    return {'control_id':control['id'],'source':'runtime shader strings from pinned MagicPen DSO; scalar arithmetic models GLSL fwidth and captured blend contracts',
            'pixels_compared':width*height,'RGBA_channel_samples_compared':len(errors),
            'native_or_predicted_support_pixels':None if opaque_support_diagnostic else support,
            'support_diagnostic_pixels':support if opaque_support_diagnostic else None,
            'support_definition':'not exactly comparable on opaque backgrounds because output alpha is always 255 and low-coverage RGB changes can quantize away; opaque threshold >1 byte is diagnostic only' if opaque_support_diagnostic else 'transparent: final alpha differs from initial alpha, exact FP/FN support is checked',
            'false_positive_support_pixels':None if opaque_support_diagnostic else false_positive,
            'false_negative_support_pixels':None if opaque_support_diagnostic else false_negative,
            'final_RGBA_channel_byte_MAE_all_pixels':mae,'final_RGBA_channel_byte_max_all_pixels':max(errors),
            'final_RGBA_channel_byte_max_on_support':max_active,
            'per_channel_byte_max':[max(values) for values in channel_errors],
            'honest_precision_bound_bytes':3,
            'device_or_export_parity_claimed':False}


def make_child_arg(control):
    return [sys.executable,str(Path(__file__).resolve()),'--_capture',json.dumps(control,separators=(',',':'))]


def run_child(control):
    env=os.environ.copy()
    env['PYTHONPATH']=os.pathsep.join([str(ROOT/'conformance'),str(ROOT/'scratch/apk-analysis-runtime/python'),env.get('PYTHONPATH','')]).rstrip(os.pathsep)
    env['LD_LIBRARY_PATH']=os.pathsep.join([str(ROOT/'scratch/apk-analysis-runtime/binutils-aarch64/usr/lib/x86_64-linux-gnu'),env.get('LD_LIBRARY_PATH','')]).rstrip(os.pathsep)
    proc=subprocess.run(make_child_arg(control),cwd=ROOT,env=env,text=True,capture_output=True)
    if proc.returncode:
        raise RuntimeError(f"{control['id']} capture failed ({proc.returncode}):\n{proc.stdout}\n{proc.stderr}")
    try:
        return json.loads(proc.stdout)
    except Exception as e:
        raise RuntimeError(f"{control['id']} emitted invalid JSON: {e}\n{proc.stdout[:1000]}")


def build_fixture(pins):
    base=json.loads(BASE_FIXTURE.read_text())
    canonical_source=base['source_capture']
    records=[]
    for control in CAPTURES:
        print(f"capturing {control['id']} version={control['version']} bg={control['background']} view={control['scale']} phase={control['phase']}",file=sys.stderr,flush=True)
        captured=run_child(control)
        source_capture=captured['source_capture'];paint=captured['paint']
        if source_capture != canonical_source:
            raise RuntimeError(f"source Drawing capture changed for {control['id']}")
        shader_law(source_capture,paint)
        scalar=scalar_compare(control,source_capture,paint)
        records.append({'id':control['id'],'kind':control['kind'],'version':control['version'],
                        'background':control['background'],'view_scale':control['scale'],'phase':control['phase'],
                        'paint':paint,'scalar_validation':scalar})
    by_id={r['id']:r for r in records}
    pairs=[('identity-v1-transparent','view-s1-phase0'),('identity-v1-opaque','identity-v2-opaque')]
    identity=[]
    for left,right in pairs:
        a,b=by_id[left]['paint'],by_id[right]['paint']
        ignore={'source_case','native_rtv1_receiver','native_vector_object'}
        unequal=[k for k in sorted(set(a)|set(b)) if k not in ignore and a.get(k)!=b.get(k)]
        rgba_a=zlib.decompress(base64.b64decode(a['readback']['rgba8_zlib_base64']))
        rgba_b=zlib.decompress(base64.b64decode(b['readback']['rgba8_zlib_base64']))
        delta=max(abs(x-y) for x,y in zip(rgba_a,rgba_b))
        if unequal or delta:
            raise RuntimeError(f'GLV1/GLV2 identity mismatch {left} vs {right}: report={unequal}, pixel_delta={delta}')
        identity.append({'glv1_control':left,'glv2_control':right,'nonidentity_report_fields_equal':True,
                         'readback_byte_equal':True,'readback_sha256':sha(rgba_a),'different_channel_bytes':0,'max_channel_delta':0,
                         'same_native_rtv1_vector_sha256':a['native_vector_sha256']==b['native_vector_sha256']})
    return {'schema':'magicpen-paint-controls-native-v1','provenance':pins,
            'source_capture':canonical_source,'shader_model':shader_law(canonical_source,by_id['view-s1-phase0']['paint']),
            'controls':records,'version_identity_controls':identity,
            'scope':['Native MagicPen Drawing caller and native RTV1 Init/CreatePenCanvas/Update(false)/Draw executed under host Mesa GLES.',
                     'Scale/phase/anisotropic changes are explicit native Draw view matrices; source Drawing and native RTV1 vector remain identical.',
                     'GLV1 versus GLV2 comparison changes only which native saved-setting row is selected; paint uses the selected native receiver and its generated RTV1.',
                     'Host EGL/GLES, output bitmap, queue facade, and null native texture seeding are explicit adapters.',
                     'Scalar comparison evaluates the runtime shader strings against final host RGBA8 buffers with a measured maximum 3-byte bound; it does not claim Android device or vector-export parity.']}


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    action=parser.add_mutually_exclusive_group()
    action.add_argument('--write-fixture',action='store_true',help='explicitly replace the checked-in capture fixture')
    action.add_argument('--check-fixture',action='store_true',help='replay all controls and compare to the fixture (default)')
    parser.add_argument('--_capture',help=argparse.SUPPRESS)
    args=parser.parse_args()
    pins=preflight()
    if args._capture:
        control=json.loads(args._capture)
        if control not in CAPTURES: raise SystemExit('private capture control is not in the pinned control matrix')
        source_capture,paint=capture_one(control)
        print(json.dumps({'source_capture':source_capture,'paint':paint},sort_keys=True))
        return
    fixture=build_fixture(pins)
    encoded=(json.dumps(fixture,indent=2,sort_keys=True)+'\n').encode()
    if args.write_fixture:
        FIXTURE.write_bytes(encoded)
        print(f'wrote {FIXTURE.relative_to(ROOT)} sha256={sha(encoded)}')
    else:
        existing=FIXTURE.read_bytes()
        if existing != encoded:
            raise SystemExit(f'fresh native paint controls differ from {FIXTURE.relative_to(ROOT)}; use --write-fixture explicitly to update')
        print(f'verified {FIXTURE.relative_to(ROOT)} sha256={sha(encoded)}')

if __name__=='__main__':
    main()
