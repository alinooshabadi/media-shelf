import { loadModule } from '../site/js/runtime.js';
import { readFileSync, writeFileSync, existsSync, mkdirSync } from 'fs';
const bytes = readFileSync('site/wasm/scenes.wasm');
const outDir = process.argv[3] || 'build/shots'; mkdirSync(outDir, { recursive: true });
const only = process.argv[2];
const m0 = await loadModule(bytes);
const results = [];
for (const c of m0.contents) {
  if (only && c.name !== only) continue;
  for (const dark of [0, 1]) {
    const m = await loadModule(bytes);             // fresh statics, like the native tool
    const x = m.exports;
    const t = performance.now();
    const inst = x.inst_new(c.index, c.width, c.height, 2, c.theme === 1 ? 1 : dark, c.accent, 0);
    x.inst_paint(inst);        // same frame the native tool writes
    const ms = performance.now() - t;
    const px = m.pixels(inst);
    const name = c.name + (dark ? '-dark' : '');
    const raw = Buffer.from(px.data.buffer, px.data.byteOffset, px.data.length);
    writeFileSync(`${outDir}/${name}.rgba`, raw);
    writeFileSync(`${outDir}/${name}.size`, `${px.w} ${px.h}`);
    results.push({ name, w: px.w, h: px.h, ms: Math.round(ms), paint: x.inst_paint_ms(inst).toFixed(1) });
  }
}
writeFileSync(`${outDir}/index.json`, JSON.stringify(results));
console.log(results.length, 'renders');
