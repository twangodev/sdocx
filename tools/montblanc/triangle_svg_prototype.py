#!/usr/bin/env python3
"""Emit a source-derived triangle SVG approximation from a V12 result JSON.

This is a pure file transform. It does not run native code or a renderer.
The captured geometry and shader alpha expressions are used directly; GLSL
fwidth is replaced by an explicitly approximate analytic derivative.
"""
import argparse
import hashlib
import json
import math
import pathlib
import struct
import xml.sax.saxutils as xml

MAX_INPUT = 16 * 1024 * 1024
MAX_VERTICES = 4096
MAX_SUBDIVISION = 12


def smoothstep(edge0, edge1, value):
    if edge0 == edge1:
        return 0.0 if value < edge0 else 1.0
    t = max(0.0, min(1.0, (value - edge0) / (edge1 - edge0)))
    return t * t * (3.0 - 2.0 * t)


def screen_point(vertex):
    # 256x256 target-sized viewport is assumed; V12 did not serialize the
    # glViewport arguments. Projection scale is 1/128.
    return 128.0 + vertex[0], 128.0 - vertex[1]


def triangles_for_buffer(result, buffer_id):
    upload = next((row for row in result['vbo_uploads']
                   if row.get('boundBuffer') == buffer_id and row.get('dataHex')), None)
    if upload is None:
        raise ValueError(f'missing captured BufferData for buffer {buffer_id}')
    raw = bytes.fromhex(upload['dataHex'])
    if len(raw) != upload['size'] or hashlib.sha256(raw).hexdigest() != upload['dataSha256']:
        raise ValueError('BufferData bytes do not match recorded length and digest')
    if len(raw) % 20:
        raise ValueError('five-f32 vertex stream has a partial vertex')
    vertices = list(struct.iter_unpack('<5f', raw))
    if not vertices or len(vertices) > MAX_VERTICES or len(vertices) % 3:
        raise ValueError('vertex stream is not a bounded complete triangle list')
    return [vertices[i:i+3] for i in range(0, len(vertices), 3)]


def uv_derivatives(tri):
    p = [screen_point(v) for v in tri]
    uv = [(v[2], v[3]) for v in tri]
    (x0,y0),(x1,y1),(x2,y2) = p
    det = (x1-x0)*(y2-y0) - (x2-x0)*(y1-y0)
    if abs(det) < 1e-14:
        return None
    result=[]
    for axis in range(2):
        f0,f1,f2=(uv[k][axis] for k in range(3))
        result.append((((f1-f0)*(y2-y0)-(f2-f0)*(y1-y0))/det,
                       ((x1-x0)*(f2-f0)-(x2-x0)*(f1-f0))/det))
    return result


def alpha_at(program, vertex, duv):
    u,v,transparency=vertex[2],vertex[3],vertex[4]
    dist=math.hypot(u-.5,v-.5)
    if duv is None or dist == 0.0:
        fw=0.0
    else:
        du_dx,du_dy=duv[0]
        dv_dx,dv_dy=duv[1]
        ddx=((u-.5)*du_dx+(v-.5)*dv_dx)/dist
        ddy=((u-.5)*du_dy+(v-.5)*dv_dy)/dist
        fw=abs(ddx)+abs(ddy)
    if program == 3:
        dist_y=abs(.5-v)
        grad=max(0.0,min(1.0,1.0-(.93/.4)*(.4-(.5-dist_y))))
        denominator=abs(.5-dist)
        circle=0.0 if denominator == 0.0 else (.5-dist)/denominator
        edge=1.0-smoothstep(.5-fw,.5,abs(.5-u))
        alpha=transparency*grad*circle*edge
    elif program == 4:
        circle=1.0-smoothstep(.5-fw,.5,dist)
        alpha=transparency*v*circle
    elif program == 5:
        circle=1.0-smoothstep(.5-fw,.5,dist)
        grad=max(0.0,min(1.0,2.0*(1.0-v)))
        alpha=transparency*grad*circle
    else:
        raise ValueError(f'no captured fragment law for program {program}')
    # SVG stop-opacity cannot represent negative or out-of-range shader alpha.
    return max(0.0,min(1.0,alpha))


def bary(tri,u,v):
    w=1.0-u-v
    return tuple(w*tri[0][i]+u*tri[1][i]+v*tri[2][i] for i in range(5))


def subdivide(tri,n):
    points={(i,j):bary(tri,i/n,j/n) for i in range(n+1) for j in range(n+1-i)}
    for i in range(n):
        for j in range(n-i):
            yield points[i,j],points[i+1,j],points[i,j+1]
            if i+j<=n-2:
                yield points[i+1,j],points[i+1,j+1],points[i,j+1]


def svg_piece(points, alphas, ident, color):
    (x0,y0),(x1,y1),(x2,y2)=points
    a0,a1,a2=alphas
    path=f'M{x0:.6f},{y0:.6f} L{x1:.6f},{y1:.6f} L{x2:.6f},{y2:.6f} Z'
    det=(x1-x0)*(y2-y0)-(x2-x0)*(y1-y0)
    if abs(det)<1e-14 or max(alphas)-min(alphas)<1e-8:
        return '',f'<path d="{path}" fill="{color}" fill-opacity="{sum(alphas)/3:.8f}"/>'
    gx=((a1-a0)*(y2-y0)-(a2-a0)*(y1-y0))/det
    gy=((x1-x0)*(a2-a0)-(x2-x0)*(a1-a0))/det
    norm=gx*gx+gy*gy
    if norm<1e-20:
        return '',f'<path d="{path}" fill="{color}" fill-opacity="{sum(alphas)/3:.8f}"/>'
    lo,hi=min(alphas),max(alphas)
    cx=(x0+x1+x2)/3; cy=(y0+y1+y2)/3; ac=sum(alphas)/3
    p0=(cx+gx*(lo-ac)/norm,cy+gy*(lo-ac)/norm)
    p1=(cx+gx*(hi-ac)/norm,cy+gy*(hi-ac)/norm)
    definition=(f'<linearGradient id="a{ident}" gradientUnits="userSpaceOnUse" '
                f'x1="{p0[0]:.8f}" y1="{p0[1]:.8f}" x2="{p1[0]:.8f}" y2="{p1[1]:.8f}">'
                f'<stop offset="0" stop-color="{color}" stop-opacity="{lo:.8f}"/>'
                f'<stop offset="1" stop-color="{color}" stop-opacity="{hi:.8f}"/>'
                '</linearGradient>')
    return definition,f'<path d="{path}" fill="url(#a{ident})"/>'


def build(result_path, output_path, subdivision):
    with pathlib.Path(result_path).open('rb') as source:
        raw=source.read(MAX_INPUT + 1)
    if len(raw)>MAX_INPUT:
        raise ValueError('source result exceeds the bounded JSON input cap')
    result=json.loads(raw)
    draws=result['draw_calls']; provenance=result['draw_provenance']
    if len(draws)!=3 or len(provenance)!=3:
        raise ValueError('expected the three captured native Draw calls')
    if [d['state']['program'] for d in draws] != [p['actual_program'] for p in provenance]:
        raise ValueError('draw and program provenance order mismatch')
    gradients=[]; groups=[]; patch_id=0; base_triangles=[]
    for draw,proof in zip(draws,provenance):
        program=proof['actual_program']
        attrs={a['index']:a for a in proof['attributes']}
        if not {0,1}.issubset(attrs):
            raise ValueError('missing position or transparency input')
        if attrs[0]['buffer']!=attrs[1]['buffer'] or attrs[0]['stride']!=20 or attrs[1]['stride']!=20:
            raise ValueError('vertex attribute ABI differs from the captured 5-f32 layout')
        if attrs[0]['type']!=5126 or attrs[1]['type']!=5126:
            raise ValueError('captured input attributes are not GL_FLOAT')
        tris=triangles_for_buffer(result,attrs[0]['buffer'])
        if len(tris)*3 != draw['values'][2]:
            raise ValueError('BufferData vertex count does not match native draw count')
        base_triangles.append(len(tris)); paths=[]
        for tri in tris:
            duv=uv_derivatives(tri)
            for patch in subdivide(tri,subdivision):
                points=[screen_point(v) for v in patch]
                alphas=[alpha_at(program,v,duv) for v in patch]
                definition,path=svg_piece(points,alphas,patch_id,'#336699')
                if definition: gradients.append(definition)
                paths.append(path); patch_id+=1
        groups.append(f'<g id="native-draw-{program}" data-program="{program}" data-vertices="{draw["values"][2]}" data-triangles="{len(tris)}">'+''.join(paths)+'</g>')
    doc=('<?xml version="1.0" encoding="UTF-8"?>\n'
         '<svg xmlns="http://www.w3.org/2000/svg" width="256" height="256" viewBox="0 0 256 256" color-interpolation="sRGB">\n'
         '<title>Montblanc RTV1 source-derived triangle approximation</title>\n'
         f'<desc>Host-controlled input {xml.escape(result["style"]["argb"])}. Captured triangle streams and three draw groups are retained in order. Alpha is a piecewise-linear approximation of captured shader expressions; fwidth uses analytic screen-space derivatives. This is not a native parity result.</desc>\n'
         '<defs>'+''.join(gradients)+'</defs>\n'+'\n'.join(groups)+'\n</svg>\n')
    out=pathlib.Path(output_path); out.parent.mkdir(parents=True,exist_ok=True)
    metadata={'schema':'montblanc-rtv1-svg-prototype-v1','source_case':result['case'],'source_result_sha256':hashlib.sha256(raw).hexdigest(),'host_argb':result['style']['argb'],'base_triangle_counts':base_triangles,'subdivision_per_base_triangle':subdivision,'patch_triangles':patch_id,'draw_program_order':[d['state']['program'] for d in draws],'viewport_assumption':'target-sized 256x256 viewport; V12 compact receipt does not serialize glViewport args','coordinate_map':'SVG x=128+position.x; SVG y=128-position.y, from captured projection 1/128 and a 256 viewport','alpha_model':'Captured fragment laws evaluated at patch vertices, with analytic fwidth approximation per source triangle; sampled values linearly interpolated by SVG gradients. Shader alpha clamped to [0,1] for SVG opacity.','unmodeled':'Exact implementation-specific fwidth, fixed-function blend factors, device parity, and supplied paint alpha beyond the controlled observed cases.','svg_sha256':hashlib.sha256(doc.encode()).hexdigest(),'svg_bytes':len(doc.encode())}
    side=out.with_suffix(out.suffix+'.json')
    # Reserve both names before writing, and remove our reservations on failure.
    with out.open('x', encoding='utf-8') as svg_file:
        try:
            with side.open('x', encoding='utf-8') as side_file:
                try:
                    svg_file.write(doc)
                    side_file.write(json.dumps(metadata, indent=2, sort_keys=True)+'\n')
                except BaseException:
                    side.unlink(missing_ok=True)
                    raise
        except BaseException:
            out.unlink(missing_ok=True)
            raise
    return metadata


def main():
    parser=argparse.ArgumentParser()
    parser.add_argument('result_json'); parser.add_argument('output_svg')
    parser.add_argument('--subdivision',type=int,default=4)
    args=parser.parse_args()
    if not 1<=args.subdivision<=MAX_SUBDIVISION:
        raise SystemExit(f'subdivision must be 1..{MAX_SUBDIVISION}')
    print(json.dumps(build(args.result_json,args.output_svg,args.subdivision),sort_keys=True))


if __name__=='__main__': main()
