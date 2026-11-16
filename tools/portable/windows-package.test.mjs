import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { binaries, digest, freshDirectory, peImports, regular, verifyInventory } from './windows-package.mjs';

function fixture(t) {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'acp-package-test-'));
  t.after(() => fs.rmSync(root, { recursive: true })); // Only our mkdtemp fixture.
  return root;
}
function pe() {
  const b = Buffer.alloc(1024);
  b.writeUInt16LE(0x5a4d, 0); b.writeUInt32LE(0x80, 0x3c); b.writeUInt32LE(0x4550, 0x80);
  b.writeUInt16LE(0x8664, 0x84); b.writeUInt16LE(1, 0x86); b.writeUInt16LE(240, 0x94);
  const opt = 0x98;
  b.writeUInt16LE(0x20b, opt); b.writeUInt32LE(16, opt + 108);
  b.writeUInt32LE(0x1000, opt + 120); b.writeUInt32LE(40, opt + 124);
  b.writeUInt32LE(0x1000, opt + 240 + 12); b.writeUInt32LE(512, opt + 240 + 16); b.writeUInt32LE(512, opt + 240 + 20);
  b.writeUInt32LE(0x1080, 512 + 12); b.write('KERNEL32.dll\0', 640);
  return b;
}
test('PE inspection reads actual named imports and rejects truncation, other architectures and unsafe names', () => {
  assert.deepEqual(peImports(pe()).imports, [{name:'kernel32.dll', delayed:false}]);
  for (const length of [0, 64, 200, 600]) assert.throws(() => peImports(pe().subarray(0, length)));
  const wrong = pe(); wrong.writeUInt16LE(0xaa64, 0x84); assert.throws(() => peImports(wrong));
  const unsafe = pe(); unsafe.write('../evil.dll\0', 640); assert.throws(() => peImports(unsafe));
  const partialDescriptor = pe(); partialDescriptor.writeUInt32LE(0x11ff, 0x98 + 120); partialDescriptor.writeUInt32LE(20, 0x98 + 124); assert.throws(() => peImports(partialDescriptor));
  const unterminated = pe(); unterminated.writeUInt32LE(20, 0x98 + 124); assert.throws(() => peImports(unterminated));
});
test('output never adopts an existing directory or follows a symlink/junction parent', t => {
  const root = fixture(t); const made = freshDirectory(root, 'owned');
  fs.writeFileSync(path.join(made, 'sentinel'), 'retain');
  assert.throws(() => freshDirectory(root, 'owned'));
  for (const value of ['../outside', '.', '..', 'bad/child', 'bad\\child', 'trailing.']) assert.throws(() => freshDirectory(root, value));
  const alias = path.join(root, 'alias'); fs.symlinkSync(made, alias, process.platform === 'win32' ? 'junction' : 'dir');
  assert.throws(() => freshDirectory(alias, 'child'));
  assert.equal(fs.readFileSync(path.join(made, 'sentinel'), 'utf8'), 'retain');
});
test('inventory refuses changed bytes, unlisted credentials and aliased executables', t => {
  const root = fixture(t); const file = path.join(root, 'acp-daemon.exe'); fs.writeFileSync(file, 'product');
  const expected = [{file:'acp-daemon.exe',size:7,sha256:digest(Buffer.from('product'))}];
  verifyInventory(root, expected);
  fs.writeFileSync(file, 'changed'); assert.throws(() => verifyInventory(root, expected)); fs.writeFileSync(file, 'product');
  fs.writeFileSync(path.join(root, 'operator.cap'), 'synthetic-not-a-token'); assert.throws(() => verifyInventory(root, expected));
  fs.linkSync(file, path.join(root, 'alias.exe')); assert.throws(() => regular(file));
});
test('shipping contract is exactly five product entrypoints', () => {
  assert.deepEqual([...binaries].sort(), ['acp-daemon.exe','acp-mcp.exe','acp-nsh-native.exe','acp-nsh-tui.exe','acp-query.exe']);
});

test('removal preflights every file, refuses forged paths, retains Profile/auth/unknown files', async t => {
  const {removeOwnedProduct} = await import('./windows-package.mjs');
  const root = fixture(t);
  const names = [...binaries, 'manifest.json','dependency-notices.json','README.txt','LICENSE-STATUS.txt','SHA256SUMS.txt'];
  const files = names.map(name => { fs.writeFileSync(path.join(root, name), 'product'); return {file:name, sha256:digest(Buffer.from('product'))}; });
  for (const name of ['profile.sqlite3','operator.cap','unknown.txt']) fs.writeFileSync(path.join(root, name), 'retain');
  assert.throws(() => removeOwnedProduct(root, [...files, {file:'../operator.cap',sha256:digest(Buffer.from('retain'))}]));
  const changed = path.join(root, 'README.txt'); fs.writeFileSync(changed, 'changed');
  assert.throws(() => removeOwnedProduct(root, files));
  assert.ok(fs.existsSync(path.join(root, binaries[0]))); // No partial removal before preflight.
  fs.writeFileSync(changed, 'product'); removeOwnedProduct(root, files);
  assert.deepEqual(fs.readdirSync(root).sort(), ['operator.cap','profile.sqlite3','unknown.txt']);
  for (const name of fs.readdirSync(root)) assert.equal(fs.readFileSync(path.join(root, name), 'utf8'), 'retain');
});
