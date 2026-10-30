const fs = require('node:fs');
const crypto = require('node:crypto');
const assert = require('node:assert/strict');
const path = require('node:path');
const wasm = require(path.resolve(process.argv[2]));
const source = fs.readFileSync(process.argv[3], 'utf8');
const cases = [
 [source, ['78a0a70b418c59c460f8791d48d7f131ad7d445f3797323eb041fd9b689ba189','f290e56addf62df35c7fbd4bfb6bddaeccc047ab382c52aa9766cafb38b7d292']],
 [source.replaceAll('#3F5480','#AD2244'), ['7243f4e9fb60d921b7dc94dac72a197bf5632582e23a7e0056e59baf24bdc5d5','66b7258d7acb3abd69c1919f4b5739ed574f102badfe124f61340b6445647b37']],
 [source.replaceAll('stroke-width="1.7"','stroke-width="2.3"'), ['9f19ac6ab3702de697d059ab6da77b91edf233f62584dfb2e6c23a5a56ef771e','76d5e8064e7ab1b93f002d54095f514dc58ed06e180ddb580861a1f131beb6bf']]
];
for (const [svg, hashes] of cases) for (const [i,size] of [64,128].entries()) assert.equal(crypto.createHash('sha256').update(wasm.rasterize_tapepen_pattern(svg,size)).digest('hex'), hashes[i]);
for (const [svg,size] of [['',64],[source,63],[source.replaceAll('#3F5480','#XXXXXX'),64],[source.replaceAll('stroke-width="1.7"','stroke-width="1e30"'),64]]) assert.throws(()=>wasm.rasterize_tapepen_pattern(svg,size),Error);

assert.equal(crypto.createHash("sha256").update(wasm.rasterize_tapepen_pattern(source,256)).digest("hex"), "42abf1113cfd13c27f1e0ccb29affae62221c0888f5ffcdc2425e2169b71b13d");
assert.throws(()=>wasm.rasterize_tapepen_pattern(source,257),Error);
console.log('7 retained native hashes and 5 rejected-input WASM controls passed');
