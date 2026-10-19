#!/usr/bin/env python3
"""Bounded nearest-sampled Pencil3 texel-cell SVG witness.

Runs the pinned Pencil3 shader on host EGL, derives a one-particle SVG from
the original embedded brush/paper bytes, and compares grouped crispEdges and
ordinary public usvg/resvg output. This is not Samsung-device parity or
production Pencil admission.
"""
import argparse
import ctypes as C
import hashlib
import json
import math
import os
import struct
import subprocess
import sys
from collections import defaultdict
from pathlib import Path

import numpy as np
from PIL import Image

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / 'conformance'))
from inkpen_paint_native import GLES, cfunc, read_va

LIB = ROOT / 'scratch/apk-analysis-native/arm64-v8a/libSPenPencil3.so'
NATIVE = ROOT / 'conformance/pencil-v1-native.json'
FIXTURE = ROOT / 'conformance/pencil-vector-sampling.json'
V, F, Q = 0xf9730, 0xf9948, 0xf9690
LIB_SHA = '41a995cac7a597af53359401df26187d4299221f3460c91b56540596fc73d359'
COLOR = (.18, .32, .72, .8)


def sha(b):
    return hashlib.sha256(b).hexdigest()


def setup_native(raw, atlas_u8, paper_u8, w, h):
    g = GLES(); f = g.f
    program = g.program(read_va(raw, V, 8192).split(b'\0', 1)[0].decode(),
                        read_va(raw, F, 8192).split(b'\0', 1)[0].decode())
    f['glUseProgram'](program)
    def upload(unit, tw, th, pixels, wrap):
        handle = C.c_uint(); f['glGenTextures'](1, C.byref(handle))
        f['glActiveTexture'](0x84c0 + unit); f['glBindTexture'](0x0de1, handle.value)
        for pname, value in ((0x2801, 0x2600), (0x2800, 0x2600), (0x2802, wrap), (0x2803, wrap)):
            f['glTexParameteri'](0x0de1, pname, value)
        rgba = b''.join(bytes((x, 0, 0, 0)) for x in pixels)
        buf = C.create_string_buffer(rgba); f['glPixelStorei'](0x0cf5, 1)
        f['glTexImage2D'](0x0de1, 0, 0x8058, tw, th, 0, 0x1908, 0x1401, C.cast(buf, C.c_void_p))
        return handle.value
    brush = upload(0, 120, 1920, atlas_u8, 0x812f)
    paper = upload(1, 150, 150, paper_u8, 0x2901)
    f['glUniform1i'](f['glGetUniformLocation'](program, b'pointPattern'), 0)
    f['glUniform1i'](f['glGetUniformLocation'](program, b'paperTexture'), 1)
    f['glUniform4f'](f['glGetUniformLocation'](program, b'inputColor'), *COLOR)
    f['glUniform2f'] = cfunc(g.gl, 'glUniform2f', None, [C.c_int, C.c_float, C.c_float])
    f['glUniform2f'](f['glGetUniformLocation'](program, b'uPaperScale'), 1., 1.)
    projection = (C.c_float * 16)(1/64,0,0,0, 0,1/64,0,0, 0,0,1,0, 0,0,0,1)
    f['glUniformMatrix4fv'](f['glGetUniformLocation'](program,b'uProjectionMatrix'),1,0,projection)
    f['glEnable'](0x0be2); f['glBlendEquationSeparate'](0x8006,0x8006)
    f['glBlendFuncSeparate'](0x0302,0x0303,0x0001,0x0303)
    quad = [struct.unpack('<4f', read_va(raw,Q+i*16,16)) for i in range(4)]
    vao=C.c_uint(); f['glGenVertexArrays'](1,C.byref(vao)); f['glBindVertexArray'](vao.value)
    vb=g.buffer(0x8892,[v for q in quad for v in q]); f['glEnableVertexAttribArray'](0)
    f['glVertexAttribPointer'](0,4,0x1406,0,0,None); f['glVertexAttribDivisor'](0,0)
    target=C.c_uint(); f['glGenTextures'](1,C.byref(target)); f['glBindTexture'](0x0de1,target.value)
    for pname,value in ((0x2801,0x2600),(0x2800,0x2600),(0x2802,0x812f),(0x2803,0x812f)):
        f['glTexParameteri'](0x0de1,pname,value)
    f['glTexImage2D'](0x0de1,0,0x8058,w,h,0,0x1908,0x1401,None)
    fb=C.c_uint(); f['glGenFramebuffers'](1,C.byref(fb)); f['glBindFramebuffer'](0x8d40,fb.value)
    f['glFramebufferTexture2D'](0x8d40,0x8ce0,0x0de1,target.value,0)
    if f['glCheckFramebufferStatus'](0x8d40)!=0x8cd5: raise RuntimeError('incomplete host framebuffer')
    f['glViewport'](0,0,w,h); f['glActiveTexture'](0x84c0); f['glBindTexture'](0x0de1,brush)
    f['glActiveTexture'](0x84c1); f['glBindTexture'](0x0de1,paper)
    return g


def native_stamp(raw, atlas, paper, attrs, w, h):
    g=setup_native(raw,atlas,paper,w,h); f=g.f
    f['glClearColor'](1,1,1,1); f['glClear'](0x4000)
    vb=g.buffer(0x8892,attrs)
    f['glEnableVertexAttribArray'](1); f['glVertexAttribPointer'](1,4,0x1406,0,20,None); f['glVertexAttribDivisor'](1,1)
    f['glEnableVertexAttribArray'](2); f['glVertexAttribPointer'](2,1,0x1406,0,20,C.c_void_p(16)); f['glVertexAttribDivisor'](2,1)
    f['glDrawArraysInstanced'](0x0005,0,4,1)
    buf=C.create_string_buffer(w*h*4); f['glPixelStorei'](0x0d05,1)
    f['glReadPixels'](0,0,w,h,0x1908,0x1401,buf)
    if f['glGetError'](): raise RuntimeError('GLES error while drawing native stamp')
    return np.frombuffer(buf.raw,np.uint8).reshape(h,w,4)[::-1].copy()


def build_svg(atlas, paper, attrs, paper_enabled, crisp, sibling=True):
    x,y,half,row,point_alpha=attrs; row=int(row); left=64+x-half; top=64-y-half; side=2*half
    cell=side/120; source_alpha=COLOR[3]*point_alpha*1.5
    def cuts(lo,hi,step,offset=0):
        a=math.floor((lo-offset)/step)+1; b=math.ceil((hi-offset)/step)
        return [lo]+[offset+i*step for i in range(a,b) if lo<offset+i*step<hi]+[hi]
    groups=defaultdict(list); intersections=0
    for iy in range(120):
        for ix in range(120):
            b=atlas[(row*120+iy)*120+ix]
            if not b: continue
            x0=left+ix*cell; x1=x0+cell; y0=top+iy*cell; y1=y0+cell
            xs=cuts(x0,x1,128/150) if paper_enabled else [x0,x1]
            ys=cuts(y0,y1,128/150,128) if paper_enabled else [y0,y1]
            for ya,yb in zip(ys,ys[1:]):
                for xa,xb in zip(xs,xs[1:]):
                    if paper_enabled:
                        px=math.floor(((xa+xb)/2)*150/128)%150
                        py=math.floor((128-(ya+yb)/2)*150/128)%150
                        p=paper[py*150+px]
                    else: p=255
                    intersections+=1; product=int(b)*int(p)
                    if product: groups[product].append((xa,ya,xb-xa,yb-ya))
    hint=' shape-rendering="crispEdges"' if crisp else ''
    body=[f'<svg xmlns="http://www.w3.org/2000/svg" width="128" height="128" viewBox="0 0 128 128">',
          f'<g id="pencil-stamp"{hint}>']
    for product,cells in sorted(groups.items()):
        alpha=source_alpha*product/(255*255)
        d=''.join(f'M{a:.10f},{b:.10f}h{w:.10f}v{h:.10f}h{-w:.10f}z' for a,b,w,h in cells)
        body.append(f'<path fill="rgb(18%,32%,72%)" fill-opacity="{alpha:.12g}" d="{d}"/>')
    body.append('</g>')
    if sibling:
        # Independent neighboring artwork deliberately outside Pencil scope.
        body.append('<rect id="sibling-art" x="4" y="4" width="8" height="8" fill="#e05020"/>')
    body.append('</svg>')
    return '\n'.join(body)+'\n', {'cell_intersections':intersections,'nonzero_vector_cells':sum(map(len,groups.values())), 'opacity_groups':len(groups)}


def write_helper(tmp):
    d=tmp/'pencil-vector-resvg'; (d/'src').mkdir(parents=True,exist_ok=True)
    (d/'Cargo.toml').write_text(f'[package]\nname="pencil-vector-resvg"\nversion="0.1.0"\nedition="2024"\n[dependencies]\nresvg="=0.47.0"\nsdocx={{path="{(ROOT / "crates/sdocx").resolve()}",features=["pdf"]}}\n[workspace]\n')
    (d/'src/main.rs').write_text(r'''use std::{env,fs}; use resvg::{usvg,tiny_skia};
fn main(){let a:Vec<_>=env::args().collect();let s=fs::read_to_string(&a[1]).unwrap();let tree=usvg::Tree::from_str(&s,&usvg::Options::default()).unwrap();let (w,h)=(a[2].parse::<u32>().unwrap(),a[2].parse::<u32>().unwrap());let mut pix=tiny_skia::Pixmap::new(w,h).unwrap();resvg::render(&tree,tiny_skia::Transform::from_scale(w as f32/128.,h as f32/128.),&mut pix.as_mut());pix.save_png(&a[3]).unwrap();let page=sdocx::RenderedPage::new(0,128,128,s);let pdf=sdocx::render_svg_pages_pdf(&[page],&sdocx::PdfOptions::default()).unwrap();fs::write(&a[4],pdf).unwrap();}''')
    return d


def file_sha(path):
    return sha(Path(path).read_bytes())


def cargo_env(workdir):
    home=workdir/'cargo-home'; (home/'registry/src').mkdir(parents=True,exist_ok=True)
    for name in ('index','cache'):
        link=home/'registry'/name
        if not link.exists(): link.symlink_to(Path.home()/'.cargo/registry'/name, target_is_directory=True)
    env=os.environ.copy(); env['CARGO_NET_OFFLINE']='true'; env['CARGO_HOME']=str(home)
    env['CARGO_TARGET_DIR']=str(workdir/'cargo-target')
    return env


def source_snapshot(helper):
    paths={
        'runner':Path(__file__).resolve(),
        'native_gles_support':ROOT/'conformance/inkpen_paint_native.py',
        'related_texture_harness':ROOT/'conformance/airbrush_texture_native.py',
        'pencil_library':LIB,
        'native_particles':NATIVE,
        'workspace_lock':ROOT/'Cargo.lock',
        'sdocx_manifest':ROOT/'crates/sdocx/Cargo.toml',
        'pdf_renderer':ROOT/'crates/sdocx/src/pdf.rs',
        'svg_render':ROOT/'crates/sdocx/src/render.rs',
        'helper_manifest':helper/'Cargo.toml',
        'helper_source':helper/'src/main.rs',
        'helper_lock':helper/'Cargo.lock',
    }
    return {name:file_sha(path) for name,path in paths.items()}


def main():
    p=argparse.ArgumentParser(description=__doc__); p.add_argument('--fixture',type=Path,default=FIXTURE); p.add_argument('--write-fixture',action='store_true'); p.add_argument('--check-fixture',action='store_true'); p.add_argument('--work-dir',type=Path,default=ROOT/'tmp/pencil-vector-sampling'); a=p.parse_args()
    raw=LIB.read_bytes()
    if sha(raw)!=LIB_SHA: raise SystemExit('Pencil3 DSO SHA mismatch')
    atlas=read_va(raw,0x12c8c,16*120*120*4)[::4]; paper=read_va(raw,0xf3c8c,150*150)
    n=json.loads(NATIVE.read_text()); source=next(c for c in n['saved_native_outputs'] if c['name']=='saved-4-samples-size-4.01')
    attrs=source['packed_attributes'][:5]
    wd=a.work_dir.resolve();wd.mkdir(parents=True,exist_ok=True);helper=write_helper(wd)
    env=cargo_env(wd)
    subprocess.run(['cargo','generate-lockfile','--offline','--manifest-path',str(helper/'Cargo.toml')],cwd=ROOT,env=env,check=True,stdout=subprocess.DEVNULL)
    initial_sources=source_snapshot(helper)
    cargo_version=subprocess.run(['cargo','--version'],check=True,text=True,capture_output=True).stdout.strip()
    rustc_version=subprocess.run(['rustc','--version'],check=True,text=True,capture_output=True).stdout.strip()
    rows=[]; group_scope=None
    for enabled in (False,True):
      for size in (128,192,256):
        native=native_stamp(raw,atlas,paper if enabled else bytes([255])*22500,attrs,size,size)
        native_white=(native[:,:,:3].astype(np.uint16)*native[:,:,3:4].astype(np.uint16)+255*(255-native[:,:,3:4].astype(np.uint16))+127)//255
        native_rgb=native_white.astype(np.uint8)
        imgs={}; pdf_hashes={}
        for crisp in (False,True):
            svg,complexity=build_svg(atlas,paper,attrs,enabled,crisp)
            tag=('paper' if enabled else 'neutral')+('-crisp' if crisp else '-normal')+f'-{size}'
            sp=wd/(tag+'.svg'); ip=wd/(tag+'.png'); pp=wd/(tag+'.pdf'); sp.write_text(svg)
            subprocess.run(['cargo','run','--locked','--quiet','--manifest-path',str(helper/'Cargo.toml'),'--',str(sp),str(size),str(ip),str(pp)],cwd=ROOT,env=env,check=True)
            rendered=np.asarray(Image.open(ip).convert('RGBA')).astype(np.uint16)
            imgs[crisp]=((rendered[:,:,:3]*rendered[:,:,3:4]+255*(255-rendered[:,:,3:4])+127)//255).astype(np.uint8)
            pdf_hashes[crisp]=sha(pp.read_bytes())
            d=np.abs(imgs[crisp].astype(np.int16)-native_rgb.astype(np.int16))
            sibling=np.zeros((size,size),dtype=bool); sibling[4*size//128:12*size//128,4*size//128:12*size//128]=True
            d[sibling]=0
            rows.append({'paper':enabled,'size':size,'crisp_edges':crisp,'different_pixels_outside_sibling':int(np.any(d,axis=2).sum()),'max_rgb_error_outside_sibling':int(d.max()),'sum_rgb_error_outside_sibling':int(d.sum()),'native_rgb_sha256':sha(native_rgb.tobytes()),'resvg_rgb_sha256':sha(imgs[crisp].tobytes()),'svg_sha256':sha(svg.encode()),'pdf_sha256':pdf_hashes[crisp],**complexity})
        # Scope control: outside-artwork pixels must be invariant to the hint.
        if not group_scope or (enabled and size==128):
            mask=np.zeros((size,size),dtype=bool); mask[4*size//128:12*size//128,4*size//128:12*size//128]=True
            group_scope={'size':size,'outside_sibling_pixels_changed':int(np.any(imgs[False][mask]!=imgs[True][mask])),'sibling_region_rgb_sha256':sha(imgs[True][mask].tobytes())}
        if imgs[True].shape != native_rgb.shape: raise AssertionError('resvg dimension mismatch')
    # Direct paired public-PDF hint-loss control: exact bytes should match.
    pdf_pairs=[{'paper':e,'size':s,'crisp_and_normal_pdf_identical':next(r for r in rows if r['paper']==e and r['size']==s and r['crisp_edges'])['pdf_sha256']==next(r for r in rows if r['paper']==e and r['size']==s and not r['crisp_edges'])['pdf_sha256']} for e in (False,True) for s in (128,192,256)]
    final_sources=source_snapshot(helper)
    if final_sources!=initial_sources: raise SystemExit('A source/build input changed while producing the native-paired result')
    fixture={'version':1,'provenance':{'library_sha256':sha(raw),'native_input_fixture_sha256':sha(NATIVE.read_bytes()),'atlas_red_sha256':sha(atlas),'paper_gray_sha256':sha(paper),'shader_sha256':[sha(read_va(raw,z,8192).split(b'\0',1)[0]) for z in (V,F)],'saved_particle':source['name'],'packed_attributes_sha256':sha(struct.pack('<5f',*attrs)),'host':'Unmodified DSO GLSL and embedded textures on surfaceless EGL/GLES; synthetic centered 128-unit page; host is not device GLES.','source_inputs_sha256':final_sources,'unrelated_texture_harness_role':'airbrush_texture_native.py is hash-recorded as an adjacent conformance reference only; this runner does not import or execute it.','toolchain':{'cargo':cargo_version,'rustc':rustc_version,'target_dir':'<work-dir>/cargo-target','cargo_lock_sha256':final_sources['helper_lock']}},'render_path':'Pinned resvg 0.47.0/usvg direct path rendering from generated SVG; same SVG also passed to sdocx::render_svg_pages_pdf.','vector_model':'Original brush and paper mask texels become logical-coordinate path cells; paper is split at periodic source texel boundaries. No rasterized stroke or flattened framebuffer is embedded.','shape_rendering_scope':'crispEdges is on #pencil-stamp group only. A nonoverlapping sibling rect is outside the group and compared across hint controls.','native_sampler':'NEAREST; brush CLAMP_TO_EDGE, paper REPEAT; shader input color and source-over factors match conformance/pencil-paint-v1.json host profile.','results':rows,'scope_control':group_scope,'pdf_hint_loss_control':pdf_pairs,'limitations':['one saved native particle (saved-4 particle 0), source row/phase and alpha only','neutral paper and embedded default paper only','host EGL/resvg profiles, not Android device','PDF path export has no pixel-grid crispEdges guarantee; PDF identity only proves the hint was discarded in this paired control','no saved-caller/context/production Pencil admission']}
    enc=json.dumps(fixture,indent=2,sort_keys=True)+'\n'
    if a.write_fixture: a.fixture.write_text(enc)
    elif not a.fixture.exists() or json.loads(a.fixture.read_text()) != fixture: raise SystemExit('pencil vector sampling fixture mismatch; use --write-fixture to review outputs')
    print(json.dumps({'cases':len(rows),'exact_crisp_cases':sum(r['different_pixels_outside_sibling']==0 for r in rows if r['crisp_edges']),'group_scope':group_scope,'pdf_identical_pairs':sum(x['crisp_and_normal_pdf_identical'] for x in pdf_pairs)},indent=2))

if __name__=='__main__': main()
