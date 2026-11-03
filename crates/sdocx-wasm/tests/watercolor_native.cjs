const fs = require('node:fs');
const path = require('node:path');
if (process.argv.length !== 5) throw new Error('Usage: node watercolor_native.cjs MODULE FIXTURE ARTIFACT_DIR');
const modulePath = path.resolve(process.argv[2]);
const sdk = require(modulePath);
const nativePath = path.resolve(process.argv[3]);
const directory = path.resolve(process.argv[4]);
const fixture = JSON.parse(fs.readFileSync(nativePath));
const profile = {WaterColorHostRtv1: {screen_width: 1920, screen_height: 1080}};
function expected(row) {
  const [width, height] = row.frame;
  const pixels = Buffer.alloc(width * height * 4, 255);
  const sparse = Buffer.from(row.final_sparse_hex, 'hex');
  for (let i = 0; i < sparse.length; i += 8) {
    const index = sparse.readUInt32LE(i);
    const x = index % width, y = Math.floor(index / width);
    sparse.copy(pixels, ((height - 1 - y) * width + x) * 4, i + 4, i + 8);
  }
  return pixels;
}
if (Object.keys(fixture.cases).length !== 8) throw new Error('expected eight native cases');
let checked = 0;
for (const [name, row] of Object.entries(fixture.cases)) {
  const source = JSON.parse(fs.readFileSync(path.join(directory, name + '.stroke.json')));
  const canvas = new sdk.InkRasterCanvas({width: row.frame[0], height: row.frame[1], pixels_per_unit: row.scale, origin: 'TopLeft'}, Buffer.alloc(row.frame[0] * row.frame[1] * 4, 255));
  canvas.paint(source, profile);
  const pixels = Buffer.from(canvas.pixels());
  if (!pixels.equals(expected(row))) throw new Error(name + ': native pixels differ');
  if (!pixels.equals(fs.readFileSync(path.join(directory, name + '.rgba')))) throw new Error(name + ': Rust and WASM differ');
  const before = Buffer.from(canvas.pixels());
  let rejected = false;
  try { canvas.paint(source, {WaterColorHostRtv1: {screen_width: 0, screen_height: 1080}}); } catch { rejected = true; }
  if (!rejected || !before.equals(Buffer.from(canvas.pixels()))) throw new Error(name + ': rejected context changed pixels');
  const svg = canvas.render_svg();
  if (svg !== fs.readFileSync(path.join(directory, name + '.svg'), 'utf8')) throw new Error(name + ': SVG carrier differs from Rust');
  checked++;
  canvas.free();
}
console.log(`WaterColor: ${checked} native and Rust canvas cases passed`);
