// Compare production canvas coverage with the hash-verified APK shaders.
// From web/: bun run conformance:fountain SHADERS.json [vite-url] [ink_geometry.json] [software-masks.json]
import { chromium } from '@playwright/test';
import { readFile } from 'node:fs/promises';
import type { ReplayStroke } from '../../src/lib/debugger/model';
import type { FountainRaster } from '../../src/lib/debugger/fountain-raster';

type Version = 4 | 5;
type Shaders = Record<Version, { alpha_vertex: string; alpha_fragment: string }>;
interface SoftwareMask {
    viewport: { scale_x: number; scale_y: number; offset_x: number; offset_y: number };
    mask: { x: number; y: number; width: number; height: number; alpha: string } | null;
}
interface PixelCase {
    name: (string | number)[];
    row: ReplayStroke;
    sx: number;
    sy: number;
    tx: number;
    ty: number;
    mask?: SoftwareMask['mask'];
}


const usage = 'Usage: bun run conformance:fountain SHADERS.json [vite-url] [ink_geometry.json] [software-masks.json]';
if (process.argv.includes('--help')) {
    console.log(usage);
    process.exit(0);
}
if (process.argv.length < 3 || process.argv.length > 6) {
    console.error(usage);
    process.exit(2);
}
// Allow one coverage level for float32 interpolation/UNORM rounding differences.
const tolerance = 1;
const shaders: Shaders = JSON.parse(await readFile(process.argv[2], 'utf8'));
const rows: ReplayStroke[] = process.argv[4] ? JSON.parse(await readFile(process.argv[4], 'utf8')) : [];
const masks: SoftwareMask[] = process.argv[5] ? JSON.parse(await readFile(process.argv[5], 'utf8')) : [];
if (masks.length && masks.length !== rows.length) throw Error('Software mask count differs');
const browser = await chromium.launch({ args: ['--use-angle=swiftshader', '--enable-unsafe-swiftshader'] });
try {
    const page = await browser.newPage();
    await page.goto(process.argv[3] ?? 'http://127.0.0.1:5194');
    const result = await page.evaluate(async ({ shaders, rows, masks, tolerance }) => {
        // Resolved by the Vite server inside Chromium, not by Bun.
        const moduleUrl = '/src/lib/debugger/fountain-raster.ts';
        const { loadFountainRaster }: { loadFountainRaster: () => Promise<() => FountainRaster> } =
            await import(moduleUrl);
        const renderer = (await loadFountainRaster())();
        const canvas = document.createElement('canvas');
        canvas.width = canvas.height = 128;
        const ctx = canvas.getContext('2d');
        if (!ctx) throw Error('Canvas2D unavailable');
        const reference = document.createElement('canvas');
        reference.width = reference.height = 128;
        const context = reference.getContext('webgl2', {
            antialias: false, premultipliedAlpha: true, preserveDrawingBuffer: true
        });
        if (!context) throw Error('WebGL2 unavailable');
        const gl = context;
        function shader(type: number, source: string) {
            const shader = gl.createShader(type);
            if (!shader) throw Error('Could not allocate reference shader');
            gl.shaderSource(shader, source);
            gl.compileShader(shader);
            if (!gl.getShaderParameter(shader, gl.COMPILE_STATUS)) throw Error(gl.getShaderInfoLog(shader) ?? 'Shader compilation failed');
            return shader;
        }
        function attribute(location: number, size: number, data: number[]) {
            gl.bindBuffer(gl.ARRAY_BUFFER, gl.createBuffer());
            gl.bufferData(gl.ARRAY_BUFFER, new Float32Array(data), gl.STATIC_DRAW);
            gl.enableVertexAttribArray(location);
            gl.vertexAttribPointer(location, size, gl.FLOAT, false, 0, 0);
        }
        const programs = new Map<Version, { program: WebGLProgram; vao: WebGLVertexArrayObject }>();
        for (const version of [4, 5] as const) {
            const program = gl.createProgram();
            if (!program) throw Error('Could not allocate reference program');
            gl.attachShader(program, shader(gl.VERTEX_SHADER, shaders[version].alpha_vertex));
            gl.attachShader(program, shader(gl.FRAGMENT_SHADER, shaders[version].alpha_fragment));
            gl.linkProgram(program);
            if (!gl.getProgramParameter(program, gl.LINK_STATUS)) throw Error(gl.getProgramInfoLog(program) ?? 'Program linking failed');
            const vao = gl.createVertexArray();
            if (!vao) throw Error('Could not allocate reference vertex array');
            gl.bindVertexArray(vao);
            if (version === 4) {
                attribute(0, 2, [0, 1, 0, 0, 1, 1, 1, 0]);
                attribute(1, 2, [1, -1, -1, 1, 1, 1, -1, -1]);
            } else {
                attribute(0, 4, [-1, -1, 0, 1, -1, 1, 0, 0, 1, -1, 1, 1, 1, 1, 1, 0]);
            }
            programs.set(version, { program, vao });
        }
        const scales = [.1, .25, .7, 1, 1.25, Math.PI, 4].map(s => [s, s]);
        scales.push([.7, 1.3], [1, 4], [4, 1]);
        function* cases(): Generator<PixelCase> {
            for (const version of [4, 5] as const) {
                for (const radius of [.1, .49, .5, 1, 2, 5]) {
                    for (const [sx, sy] of scales) {
                        for (const angle of [0, .4, 1.57, 2.1]) {
                            for (const count of [1, 3]) {
                                const direction = { x: Math.cos(angle), y: Math.sin(angle) };
                                const points = Array.from({ length: count }, (_, i) => ({
                                    x: (64 + i * .7) / sx, y: (64 + i * .3) / sy
                                }));
                                yield {
                                    name: [version, radius, sx, sy, angle, count], sx, sy, tx: 0, ty: 0,
                                    row: {
                                        offset: 0, milliseconds: false,
                                        stroke: {
                                            points, pressures: [], timestamps: [], tilts: [], orientations: [],
                                            pen_width: radius * 2, color: null,
                                            bbox: { x_min: 0, y_min: 0, x_max: 128, y_max: 128 }
                                        },
                                        geometry: {
                                            width: radius * 2, segment_widths: null, bounds: null,
                                            color: '#ffffff', profile: null, support: 'reconstructed',
                                            points, dot_radii: points.map(() => radius),
                                            dot_directions: version === 4 ? points.map(() => direction) : undefined,
                                            fountain_shader: version, opacity: 1
                                        }
                                    }
                                };
                            }
                        }
                    }
                }
            }
            for (const [index, row] of rows.entries()) {
                if (!row.geometry.fountain_shader) continue;
                if (masks.length) {
                    const { viewport: v, mask } = masks[index];
                    yield { name: ['software', index], row, mask, sx: v.scale_x, sy: v.scale_y,
                        tx: v.offset_x, ty: v.offset_y };
                    continue;
                }
                const b = row.geometry.bounds;
                if (!b) throw Error(`Stroke ${index} is missing prepared bounds`);
                const scale = Math.min(4, 112 / Math.max(b.x_max - b.x_min, b.y_max - b.y_min));
                yield { name: ['stroke', index], row, sx: scale, sy: scale,
                    tx: 64 - (b.x_min + b.x_max) * scale / 2,
                    ty: 64 - (b.y_min + b.y_max) * scale / 2 };
            }
        }
        let maximum = 0, count = 0, differingPixels = 0;
        const failed: { name: PixelCase['name']; error: number; worst: number[] | undefined }[] = [];
        for (const test of cases()) {
            const { row, sx, sy, tx, ty } = test;
            const { geometry } = row;
            const points = geometry.points ?? row.stroke.points;
            const version = geometry.fountain_shader;
            if ((version !== 4 && version !== 5) || geometry.dot_radii?.length !== points.length ||
                (version === 4 && geometry.dot_directions?.length !== points.length)) {
                throw Error(`Invalid prepared fountain geometry: ${test.name.join(', ')}`);
            }
            ctx.resetTransform();
            ctx.clearRect(0, 0, 128, 128);
            ctx.setTransform(sx, 0, 0, sy, tx, ty);
            // Canvas may round setTransform's doubles internally. Both
            // renderers must receive the transform actually retained by it.
            const transform = ctx.getTransform();
            ctx.strokeStyle = '#ffffff';
            renderer.draw(ctx, row, points.length - 1);
            const actual = ctx.getImageData(0, 0, 128, 128).data;
            if (test.name[0] === 'software') {
                actual.fill(0);
                const m = test.mask;
                if (m) {
                    const alpha = atob(m.alpha);
                    for (let y = 0; y < m.height; y++) {
                        for (let x = 0; x < m.width; x++) {
                            actual[((y + m.y) * 128 + x + m.x) * 4 + 3] = alpha.charCodeAt(y * m.width + x);
                        }
                    }
                }
            }
            gl.clearColor(0, 0, 0, 0);
            gl.clear(gl.COLOR_BUFFER_BIT);
            gl.enable(gl.BLEND);
            gl.blendEquation(gl.MAX);
            gl.blendFunc(gl.ONE, gl.ONE);
            gl.viewport(0, 0, 128, 128);
            const { program, vao } = programs.get(version)!;
            gl.useProgram(program);
            gl.bindVertexArray(vao);
            gl.uniformMatrix4fv(gl.getUniformLocation(program, 'ProjectionMatrix'), false,
                new Float32Array([1 / 64, 0, 0, 0, 0, -1 / 64, 0, 0, 0, 0, 1, 0, -1, 1, 0, 1]));
            const f = Math.fround;
            for (const [i, pt] of points.entries()) {
                const radius = f(geometry.dot_radii[i]);
                const rx = f(radius / f(1 / transform.a)), ry = f(radius / f(1 / transform.d));
                const factor = Math.min(rx, ry) < .5 ? f(.5 / Math.min(rx, ry)) : 1;
                const ex = f(f(rx * factor) + .5), ey = f(f(ry * factor) + .5);
                const inner = f(f(f(Math.max(ex, ey) - 1) / Math.max(ex, ey)) * .5);
                gl.vertexAttrib4f(version === 4 ? 2 : 1,
                    pt.x * transform.a + transform.e, pt.y * transform.d + transform.f, ex, ey);
                if (version === 4) {
                    const d = geometry.dot_directions![i];
                    gl.vertexAttrib2f(3, d.x, d.y);
                    gl.vertexAttrib1f(4, 1);
                    gl.vertexAttrib2f(5, inner, f(1 / factor));
                } else {
                    gl.vertexAttrib1f(2, 1);
                    gl.vertexAttrib2f(3, inner, f(1 / factor));
                }
                gl.drawArrays(gl.TRIANGLE_STRIP, 0, 4);
            }
            const expected = new Uint8Array(128 * 128 * 4);
            gl.readPixels(0, 0, 128, 128, gl.RGBA, gl.UNSIGNED_BYTE, expected);
            let error = 0;
            let worst;
            for (let y = 0; y < 128; y++) {
                for (let x = 0; x < 128; x++) {
                    // RTV4 stores coverage in alpha; RTV5 stores it in red.
                    const a = actual[(y * 128 + x) * 4 + 3];
                    const b = expected[((127 - y) * 128 + x) * 4 + (version === 4 ? 3 : 0)];
                    if (a !== b) differingPixels++;
                    if (Math.abs(a - b) > error) {
                        error = Math.abs(a - b);
                        worst = [x, y, a, b];
                    }
                }
            }
            maximum = Math.max(maximum, error);
            count++;
            if (error > tolerance && failed.length < 10) failed.push({ name: test.name, error, worst });
        }
        renderer.dispose();
        return { cases: count, tolerance, maximum, differingPixels, failed };
    }, { shaders, rows, masks, tolerance });
    console.log(JSON.stringify(result));
    if (result.failed.length) throw new Error('Native fountain pixel parity failed');
} finally {
    await browser.close();
}
