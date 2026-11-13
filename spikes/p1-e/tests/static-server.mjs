// Test-only loopback assets. No native IPC, API, credentials or directory listing.
import { createServer } from 'node:http';
import { realpath, readFile, stat } from 'node:fs/promises';
import path from 'node:path';
export async function serveStatic(directory) {
  const root = await realpath(directory);
  const server = createServer(async (request, response) => {
    try {
      if (!['GET', 'HEAD'].includes(request.method)) {
        response.writeHead(405).end(); return;
      }
      const pathname = decodeURIComponent(request.url.split('?')[0]);
      if (!pathname.startsWith('/') || pathname.includes('\\') || pathname.includes('\0')) {
        response.writeHead(400).end(); return;
      }
      const candidate = path.resolve(root, '.' + (pathname === '/' ? '/index.html' : pathname));
      const confined = file => file.startsWith(root + path.sep);
      if (!confined(candidate)) { response.writeHead(403).end(); return; }
      const resolved = await realpath(candidate);
      if (!confined(resolved) || !(await stat(resolved)).isFile()) {
        response.writeHead(403).end(); return;
      }
      const mime = { '.html':'text/html', '.js':'text/javascript', '.css':'text/css', '.wasm':'application/wasm', '.json':'application/json' };
      response.writeHead(200, { 'Content-Type':mime[path.extname(resolved)] || 'application/octet-stream', 'Cache-Control':'no-store' });
      response.end(request.method === 'HEAD' ? undefined : await readFile(resolved));
    } catch {
      response.writeHead(404).end();
    }
  });
  await new Promise((resolve, reject) => { server.once('error', reject); server.listen(0, '127.0.0.1', resolve); });
  return { origin:`http://127.0.0.1:${server.address().port}`, close:() => new Promise((resolve, reject) => server.close(error => error ? reject(error) : resolve())) };
}
