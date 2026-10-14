// Smoke test for the WebAssembly build: load a pack the way the page does and ask the
// four questions the page asks. Run by `just wasm-smoke` after `just wasm`.
//
//   node crates/wasm/tests/smoke.mjs <dist dir> <pack.gz>

import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { pathToFileURL } from 'node:url';
import { gunzipSync } from 'node:zlib';

const [dist, pack] = process.argv.slice(2).map((p) => resolve(p));
const m = await import(pathToFileURL(`${dist}/pkg/overrules_wasm.js`));
await m.default({ module_or_path: readFileSync(`${dist}/pkg/overrules_wasm_bg.wasm`) });

const explorer = new m.Explorer(gunzipSync(readFileSync(pack)));
const ask = (params) => JSON.parse(explorer.resolve(JSON.stringify(params)));

const stats = JSON.parse(explorer.stats());
assert.ok(stats.segments > 0, 'pack has segments');
assert.equal(JSON.parse(explorer.network()).type, 'FeatureCollection');
assert.ok(JSON.parse(explorer.find('Pintor')).length > 0, 'find by name');

const lorry = ask({ segment: '645f3ec7', heading: 'forward', mode: 'truck', vehicle: 'weight=12' });
assert.equal(lorry.access.value, 'denied');
assert.equal(lorry.access.determining, 1);

const speed = ask({ segment: 'e26d3384', at: '0.05', heading: 'forward', mode: 'car' });
assert.equal(speed.speed_limit.determining, 5);
assert.equal(speed.speed_limit.value.max_speed.value, 80);

assert.throws(() => ask({ segment: 'zzzz' }), /no segment/);

console.log(`wasm smoke test passed (${stats.segments} segments from ${explorer.source()})`);
