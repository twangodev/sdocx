// Run against a Node-target wasm-pack build: node pencil_translation.cjs /path/to/sdocx_wasm.js
const assert = require('node:assert/strict');
const path = require('node:path');
const fs = require('node:fs');
const zlib = require('node:zlib');
const sdk = require(path.resolve(process.argv[2]));
const geometry = require('../../../conformance/pencil2-v14-native.json');
const input = geometry.cases.find(value => value.id === 'channel_baseline').input;
const size = Buffer.from(input.object_stroke_size_f32_bits, 'hex').readFloatLE();
const stroke = {
  bbox: {x_min: 0, y_min: 0, x_max: 0, y_max: 0},
  points: input.points.map(([x, y]) => ({x, y})),
  pressures: input.pressure, tilts: input.tilt, orientations: input.orientation,
  timestamps: input.timestamps, pen_width: size, color: {r: 51, g: 102, b: 153},
  rendering: {
    metadata: null, pen_name: 'com.samsung.android.sdk.pen.pen.preload.Pencil2',
    advanced_settings: '14;', tool_type_raw: 2,
    style: {color_argb: 0x80336699, pen_size: size, size_level: 0, stroke_type_raw: 0, color_type_raw: 0, trailing_data: []},
    properties: {compressed: true, stylus_channels: true, replay_only: false, eraser: false,
      fixed_width: false, millisecond_timestamps: false, top_layer_pen: false, alpha_lock: false,
      binary_added: false, generated: false, fixed_opacity: false, rainbow_effect: false,
      straighten: false, reveal_mode: false}
  }
};
for (const origin of ['TopLeft', 'BottomLeft']) {
  for (const inverse of ['One', 'Half']) {
    const context = {width: 192, height: 192, pixels_per_unit: 1, origin};
    const backdrop = new Uint8Array(192 * 192 * 4);
    const standard = new sdk.InkRasterCanvas(context, backdrop);
    const translated = new sdk.InkRasterCanvas(context, backdrop);
    try {
      standard.paint(stroke, {Pencil2V14: {inverse_scale: inverse}});
      translated.paint_pencil2_translated(stroke, inverse, 0, 0);
      assert.deepEqual(translated.pixels(), standard.pixels());
      const zero = new sdk.InkRasterCanvas(context, backdrop);
      try {
        zero.paint_pencil2_with_grain_offset(stroke, inverse, 0, 0, 0, 0);
        assert.deepEqual(zero.pixels(), translated.pixels());
      } finally { zero.free(); }
      const before = translated.pixels();
      for (const [scale, x, y] of [['Invalid', 0, 0], [inverse, NaN, 0], [inverse, 0, Infinity], [inverse, 16385, 0]]) {
        assert.throws(() => translated.paint_pencil2_translated(stroke, scale, x, y));
        assert.deepEqual(translated.pixels(), before);
      }
      for (const [scale, x, y, gx, gy] of [
        ['Invalid', 0, 0, 0, 0], [inverse, NaN, 0, 0, 0],
        [inverse, 0, Infinity, 0, 0], [inverse, 0, 0, NaN, 0],
        [inverse, 0, 0, 0, Infinity], [inverse, 0, 0, 16385, 0],
        [inverse, 0, 0, 0, -16385],
      ]) {
        assert.throws(() => translated.paint_pencil2_with_grain_offset(stroke, scale, x, y, gx, gy));
        assert.deepEqual(translated.pixels(), before);
      }
      const moved = new sdk.InkRasterCanvas(context, backdrop);
      try {
        moved.paint_pencil2_translated(stroke, inverse, 6, 0);
        assert.notDeepEqual(moved.pixels(), standard.pixels());
      } finally { moved.free(); }
      const raised = new sdk.InkRasterCanvas(context, backdrop);
      try {
        raised.paint_pencil2_translated(stroke, inverse, 0, 6);
        const meanRow = pixels => {
          let weighted = 0, alpha = 0;
          for (let i = 3; i < pixels.length; i += 4) {
            weighted += Math.floor((i - 3) / 4 / context.width) * pixels[i];
            alpha += pixels[i];
          }
          assert.ok(alpha > 0);
          return weighted / alpha;
        };
        assert.ok(meanRow(raised.pixels()) < meanRow(standard.pixels()));
      } finally { raised.free(); }
    } finally { standard.free(); translated.free(); }
  }
}

const limited = new sdk.InkRasterCanvas(
  {width: 192, height: 192, pixels_per_unit: 1, origin: 'TopLeft'},
  new Uint8Array(192 * 192 * 4),
);
try {
  let succeeded = 0;
  for (;;) {
    const before = limited.pixels();
    try {
      limited.paint_pencil2_translated(stroke, 'One', 16384, 0);
      succeeded++;
      assert.ok(succeeded <= 256, 'work limit was not enforced');
    } catch (error) {
      assert.match(String(error), /work limit/i);
      assert.deepEqual(limited.pixels(), before);
      assert.ok(succeeded > 0);
      break;
    }
  }
} finally { limited.free(); }

const budget = new sdk.InkRasterCanvas(
  {width: 1, height: 1, pixels_per_unit: 1, origin: 'BottomLeft'}, new Uint8Array(4));
try {
  assert.throws(() => budget.paint_pencil2_with_grain_offset(stroke, 'One', 0, 0, NaN, 0));
  const expensive = {...stroke, points: stroke.points.map((p, i) =>
    i === stroke.points.length - 1 ? {...p, x: 100000} : p)};
  assert.throws(() => budget.paint_pencil2_with_grain_offset(expensive, 'One', 0, 0, 6, 9), /work limit/i);
  for (let i = 0; i < 256; i++) budget.paint_pencil2_with_grain_offset(stroke, 'One', 0, 0, 6, 9);
  const before = budget.pixels();
  assert.throws(() => budget.paint_pencil2_with_grain_offset(stroke, 'One', 0, 0, 6, 9), /work limit/i);
  assert.deepEqual(budget.pixels(), before);
} finally { budget.free(); }

const prior = new Uint8Array(192 * 192 * 4);

// Synthetic Drawing handoff captured on native host Mesa, with explicit offset.
const drawingStroke = {
  ...stroke, points: [[0, 0], [10, 0], [20, 0], [30, 10]].map(([x, y]) => ({x, y})),
  pressures: [0.25, 0.5, 0.75, 1], tilts: [0.4, 0.4, 0.4, 0.4],
  orientations: [0, 0, 0, 0], timestamps: [10, 20, 30, 40], pen_width: 2,
  rendering: {...stroke.rendering, style: {...stroke.rendering.style, pen_size: 2}},
};
const nextF32After32 = 32 + 2 ** -18;
for (const [file, tx, ty, gx, gy] of [
  ['drawing-native-target-6-9.rgba.zlib', 6, 9, 6, 9],
  ['drawing-native-target-translated.rgba.zlib', 32, 32, nextF32After32, nextF32After32],
]) {
  const frame = zlib.inflateSync(fs.readFileSync(path.join(
    __dirname, '../../sdocx/src/ink/pencil2/test_assets/drawing/public-replay', file)));
  const expected = Buffer.alloc(frame.length);
  for (let y = 0; y < 192; y++) {
    frame.copy(expected, y * 192 * 4, (191 - y) * 192 * 4, (192 - y) * 192 * 4);
  }
  const context = {width: 192, height: 192, pixels_per_unit: 1, origin: 'BottomLeft'};
  const canvas = new sdk.InkRasterCanvas(context, prior);
  const legacy = new sdk.InkRasterCanvas(context, prior);
  const zero = new sdk.InkRasterCanvas(context, prior);
  try {
    canvas.paint_pencil2_with_grain_offset(drawingStroke, 'One', tx, ty, gx, gy);
    assert.deepEqual(Buffer.from(canvas.pixels()), expected, `native Drawing host frame ${file}`);
    legacy.paint_pencil2_translated(drawingStroke, 'One', tx, ty);
    zero.paint_pencil2_with_grain_offset(drawingStroke, 'One', tx, ty, 0, 0);
    assert.deepEqual(zero.pixels(), legacy.pixels());
    assert.notDeepEqual(zero.pixels(), canvas.pixels());
    if (tx === 32) {
      const rounded = new sdk.InkRasterCanvas(context, prior);
      try {
        rounded.paint_pencil2_with_grain_offset(drawingStroke, 'One', tx, ty, 32, 32);
        assert.notDeepEqual(rounded.pixels(), canvas.pixels(), 'captured f32 ULP affects the frame');
      } finally { rounded.free(); }
    }
  } finally { canvas.free(); legacy.free(); zero.free(); }
}
