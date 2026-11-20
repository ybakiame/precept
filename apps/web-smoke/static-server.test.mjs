import { test } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtemp, mkdir, writeFile, symlink, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import http from 'node:http';
import { serveStatic } from './static-server.mjs';
test('loopback assets preserve WASM MIME and deny traversal, directories and writes', async () => {
  const temp = await mkdtemp(path.join(tmpdir(), 'precept-s4-web-'));
  await mkdir(path.join(temp, 'assets'));
  await writeFile(path.join(temp, 'assets/index.html'), 'synthetic');
  await writeFile(path.join(temp, 'assets/app.wasm'), new Uint8Array([0, 97, 115, 109]));
  await writeFile(path.join(temp, 'outside'), 'PRIVATE_SYNTHETIC');
  // Junctions need no Windows symlink privilege and still test realpath confinement.
  await symlink(temp, path.join(temp, 'assets/escape'), process.platform === 'win32' ? 'junction' : 'dir');
  const server = await serveStatic(path.join(temp, 'assets'));
  try {
    const get = async (raw, method = 'GET') => new Promise((resolve, reject) => {
      const req = http.request(server.origin, { path:raw, method }, response => {
        let body = ''; response.on('data', chunk => body += chunk);
        response.on('end', () => resolve({ status:response.statusCode, body, type:response.headers['content-type'] }));
      }); req.on('error', reject); req.end();
    });
    assert.equal((await get('/')).body, 'synthetic');
    assert.equal((await get('/app.wasm')).type, 'application/wasm');
    assert.equal((await get('/app.wasm', 'HEAD')).body, '');
    for (const raw of ['/../outside', '/%2e%2e/outside', '/escape/outside', '/escape/', '/missing', '/%00', '/%ff', '/..\\outside']) {
      const result = await get(raw); assert(result.status >= 400, raw); assert(!result.body.includes('PRIVATE_SYNTHETIC'));
    }
    assert.equal((await get('/', 'POST')).status, 405);
  } finally { await server.close(); await rm(temp, { recursive:true, force:true }); }
});
