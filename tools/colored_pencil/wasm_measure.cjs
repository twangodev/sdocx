const fs = require('node:fs');
const path = require('node:path');
const crypto = require('node:crypto');
const zlib = require('node:zlib');
const root = path.resolve(__dirname, '../..');
const modulePath = path.resolve(process.argv[2]);
const sdk = require(modulePath);
const directory = path.join(root, 'scratch/colored-pencil-public/exports');
const contextMode = process.argv[3] === 'context';
const displayMode = process.argv[3] === 'display';
const mixedMode = process.argv[3] === 'mixed';
const drawingMode = process.argv[3] === 'drawing' || mixedMode;
const depthMode = process.argv[3] === 'depth' || displayMode || drawingMode;
const extendedMode = contextMode || depthMode;
const nativePath = path.join(root, 'conformance', mixedMode ? 'colored-pencil-v4-pencil2-drawing-native.json' : drawingMode ? 'colored-pencil-v4-drawing-lifecycle-native.json' : displayMode ? 'colored-pencil-v4-display-depth-native.json' : depthMode ? 'colored-pencil-v4-depth-native.json' : contextMode ? 'colored-pencil-v4-context-paint-native.json' : 'colored-pencil-v4-canvas-native.json');
const fixture = JSON.parse(fs.readFileSync(nativePath));
const geometryPath = path.join(root, 'conformance/colored-pencil-v4-source-context-native.json');
const geometry = extendedMode ? JSON.parse(fs.readFileSync(geometryPath)).cases : null;
const prefix = mixedMode ? 'mixed_' : drawingMode ? 'drawing_' : displayMode ? 'display_' : depthMode ? 'depth_' : contextMode ? 'context_' : '';
const profile = {ColoredPencilV4Host: {pen_data_width: 1920, pen_data_height: 1080}};
const sha = value => crypto.createHash('sha256').update(value).digest('hex');
function expected(row) {
  const [width, height] = row.frame;
  const pixels = Buffer.alloc(width * height * 4, Buffer.from(row.background));
  if (row.rgba_zlib_base64) {
    const full = zlib.inflateSync(Buffer.from(row.rgba_zlib_base64, 'base64'));
    if (full.length !== pixels.length || sha(full) !== row.frame_sha256) throw new Error('Invalid native full frame');
    for (let y = 0; y < height; ++y) full.copy(pixels, (height - 1 - y) * width * 4, y * width * 4, (y + 1) * width * 4);
    return pixels;
  }
  const sparse = zlib.inflateSync(Buffer.from(row.sparse_zlib_base64, 'base64'));
  for (let i = 0; i < sparse.length; i += 8) {
    const index = sparse.readUInt32LE(i);
    const x = index % width, y = Math.floor(index / width);
    sparse.copy(pixels, ((height - 1 - y) * width + x) * 4, i + 4, i + 8);
  }
  return pixels;
}
const results = {};
const activeCanvases = new Map();
for (const [name, row] of Object.entries(fixture.cases)) {
  const artifact = prefix + name;
  const source = JSON.parse(fs.readFileSync(path.join(directory, artifact + '.stroke.json')));
  const screen = extendedMode ? geometry.find(c => c.id === row.source).screen : [1920, 1080];
  const selectedProfile = extendedMode ? {ColoredPencilV4Host: {pen_data_width: screen[0], pen_data_height: screen[1]}} : profile;
  const prior = mixedMode ? expected(fixture.prior_inputs[row.prior_id]) : Buffer.alloc(row.frame[0] * row.frame[1] * 4, Buffer.from(row.background));
  const cached = drawingMode ? activeCanvases.get(row.sequence) : null;
  const canvas = cached ? cached.canvas : new sdk.InkRasterCanvas({width: row.frame[0], height: row.frame[1], pixels_per_unit: row.scale, origin: row.origin === 'top' ? 'TopLeft' : 'BottomLeft'}, prior);
  const state = cached || {canvas, drawing: null, gl: null, rt: null};
  if (drawingMode) {
    if (row.target === 'drawing') state.drawing = row.argument;
    else if (row.target === 'gl') state.gl = row.argument;
    else if (row.target === 'rt' && row.argument !== null) state.rt = row.argument;
    if (state.drawing !== null) state.gl = state.drawing;
    if (state.gl !== null) state.rt = state.gl;
    if (state.drawing !== row.drawing_depth || state.gl !== row.gl_depth || state.rt !== row.effective_depth) throw new Error(name + ': reconstructed depth state differs');
    activeCanvases.set(row.sequence, state);
  }
  const depth = depthMode ? fixture.depth_inputs[drawingMode ? state.rt : row.depth] : null;
  const depthContext = displayMode || drawingMode ? sdk.colored_pencil_depth_context(depth.width, depth.height, screen[0], screen[1]) : depthMode ? {width: depth.width, height: depth.height, background_scale: row.background_scale} : null;
  if ((displayMode || drawingMode) && depthContext.background_scale !== row.background_scale) throw new Error(name + ': derived scale differs from native');
  if (depthMode) canvas.paint_with_depth_map(source, selectedProfile, depthContext, Buffer.from(depth.rgba_hex, 'hex'));
  else canvas.paint(source, selectedProfile);
  const pixels = Buffer.from(canvas.pixels());
  if (!pixels.equals(expected(row))) throw new Error(name + ': native pixels differ');
  if (!pixels.equals(fs.readFileSync(path.join(directory, artifact + '.rgba')))) throw new Error(name + ': Rust and WASM differ');
  let rejected = false;
  try { canvas.paint(source, {ColoredPencilV4Host: {pen_data_width: 0, pen_data_height: 1080}}); } catch { rejected = true; }
  if (!rejected || !pixels.equals(Buffer.from(canvas.pixels()))) throw new Error(name + ': rejected context changed pixels');
  if (depthMode) {
    let depthRejected = false;
    try {canvas.paint_with_depth_map(source, selectedProfile, depthContext, Buffer.alloc(0));} catch {depthRejected = true;}
    if (!depthRejected || !pixels.equals(Buffer.from(canvas.pixels()))) throw new Error(name + ': rejected depth storage changed pixels');
  }
  fs.writeFileSync(path.join(directory, 'wasm_' + artifact + '.rgba'), pixels);
  let svgMatchesRust = null;
  if (row.background.every(v => v === 255)) {
    fs.writeFileSync(path.join(directory, 'wasm_' + artifact + '.png'), canvas.to_png());
    const svg = canvas.render_svg();
    if (svg !== fs.readFileSync(path.join(directory, artifact + '.svg'), 'utf8')) throw new Error(name + ': SVG carrier differs from Rust');
    fs.writeFileSync(path.join(directory, 'wasm_' + artifact + '.direct.pdf'), canvas.render_pdf());
    svgMatchesRust = true;
  }
  results[name] = {native_pixel_errors: 0, rust_pixel_errors: 0, rejected_context_atomic: true, svg_matches_rust: svgMatchesRust, top_rgba_sha256: sha(pixels)};
  if (!drawingMode || row.step === (mixedMode ? 2 : 7)) {
    canvas.free();
    if (drawingMode) activeCanvases.delete(row.sequence);
  }
}
if (activeCanvases.size !== 0) throw new Error('Drawing sequence did not finish');
const receipt = {
  schema: 'sdocx-colored-pencil-wasm-public-canvas-v1',
  capture: {script: 'tools/colored_pencil/wasm_measure.cjs', script_sha256: sha(fs.readFileSync(__filename)), native_receipt_sha256: sha(fs.readFileSync(nativePath)), wasm_sha256: sha(fs.readFileSync(modulePath.replace(/\.js$/, '_bg.wasm')))},
  runtime: {node: process.version, binding: 'wasm-bindgen 0.2.114'},
  results,
  scope: `Actual compiled Node/WASM public canvas, ${Object.keys(results).length} native record/source/context/backdrop controls and atomic rejected context. Opaque frames compare SVG equality with Rust and emit PDF for independent reader checks. Not browser UI, normal document dispatch or saved-note/device parity.`
};
if (extendedMode) receipt.capture.geometry_receipt_sha256 = sha(fs.readFileSync(geometryPath));
if (displayMode) {
  receipt.capture.scale_producer_receipt_sha256 = sha(fs.readFileSync(path.join(root, 'conformance/colored-pencil-v4-background-scale-native.json')));
  let rejected = false;
  try {sdk.colored_pencil_depth_context(31, 17, 0, 1080);} catch {rejected = true;}
  if (!rejected) throw new Error('Display helper admitted invalid context');
  receipt.display_helper_invalid_context_rejected = true;
}
fs.writeFileSync(path.join(root, 'conformance', `colored-pencil-${prefix.replace('_', '-')}wasm-canvas.json`), JSON.stringify(receipt) + '\n');
console.log('Native-exact Node/WASM controls:', Object.keys(results).length);
