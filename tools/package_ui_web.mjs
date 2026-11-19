// Assemble only the accepted pinned NSH offline renderer; no product gateway.
import { cp, mkdir, readdir } from 'node:fs/promises';
import { execFileSync } from 'node:child_process';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
const [source, wasm, output, ...extra] = process.argv.slice(2);
if (!source || !wasm || !output || extra.length) throw Error('Usage: node tools/package_ui_web.mjs PINNED_NSH_CHECKOUT WASM NEW_OUTPUT');
const pin = '554163758e30d2f62f814096a6327937dcb4fe49';
if (execFileSync('git', ['-C', source, 'rev-parse', 'HEAD'], { encoding:'utf8' }).trim() !== pin) throw Error('Unexpected NSH revision');
// Refuse an existing destination rather than adopting/merging stale generated assets.
await mkdir(output, { recursive:false });
const upstream = path.join(source, 'crates/nsh-platform-web/web');
await cp(path.join(upstream, 'nsh.js'), path.join(output, 'nsh.js'));
await cp(path.join(upstream, 'browser'), path.join(output, 'browser'), { recursive:true });
await cp(path.join(source, 'LICENSE-APACHE'), path.join(output, 'NSH-LICENSE-APACHE.txt'));
await cp(wasm, path.join(output, 'acp_ui.wasm'));
const web = fileURLToPath(new URL('../apps/ui/web/', import.meta.url));
for (const name of ['index.html', 'start.js']) await cp(path.join(web, name), path.join(output, name));
console.log('PASS: pinned offline NSH assets', (await readdir(output)).length);
