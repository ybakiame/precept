import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { binaries, checkElf, checkTarListing, removable } from './linux-package.mjs';
import { digest, removeOwnedProduct } from './windows-package.mjs';

test('ELF policy: system loader, glibc, and never RPATH/RUNPATH', () => {
  const ok = { interpreter: '/lib64/ld-linux-x86-64.so.2', rpath: [], needed: ['libc.so.6', 'libgtk-4.so.1'] };
  assert.equal(checkElf('x', ok), ok);
  assert.throws(() => checkElf('x', { ...ok, rpath: ['$ORIGIN/../lib'] }));
  assert.throws(() => checkElf('x', { ...ok, interpreter: '/tmp/ld.so' }));
  assert.throws(() => checkElf('x', { ...ok, needed: ['libgtk-4.so.1'] }));
});

test('tar listing: ACP paths only, no links/devices/traversal, exact modes', () => {
  const good = [
    'drwxr-xr-x 0/0 0 2026-11-17 00:00 ACP/',
    'drwxr-xr-x 0/0 0 2026-11-17 00:00 ACP/bin/',
    '-rwxr-xr-x 0/0 9 2026-11-17 00:00 ACP/bin/acp-daemon',
    '-rw-r--r-- 0/0 9 2026-11-17 00:00 ACP/README.txt',
  ];
  assert.equal(checkTarListing(good).length, 4);
  for (const bad of [
    'lrwxrwxrwx 0/0 0 2026-11-17 00:00 ACP/bin/acp-daemon -> /bin/sh',
    'crw-r--r-- 0/0 0 2026-11-17 00:00 ACP/dev',
    '-rw-r--r-- 0/0 9 2026-11-17 00:00 ACP/../etc/passwd',
    '-rw-r--r-- 0/0 9 2026-11-17 00:00 /etc/passwd',
    '-rw-r--r-- 0/0 9 2026-11-17 00:00 other/file',
    '-rw-r--r-- 0/0 9 2026-11-17 00:00 ACP/bin/acp-daemon',
    '-rwxrwxrwx 0/0 9 2026-11-17 00:00 ACP/README.txt',
    '-rwsr-xr-x 0/0 9 2026-11-17 00:00 ACP/bin/acp-daemon',
  ]) assert.throws(() => checkTarListing([bad]), bad);
});

test('exact removal with the Linux allowlist keeps unknown, Profile and auth files', t => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'acp-linux-removal-'));
  t.after(() => fs.rmSync(root, { recursive: true }));
  const names = [...binaries.map(b => `ACP/bin/${b}`), 'ACP/runtime/manifest.json', 'ACP/notices/NOTICE.txt',
    'ACP/notices/dependency-notices.json', 'ACP/README.txt', 'ACP/LICENSE.txt'];
  for (const d of ['ACP', 'ACP/bin', 'ACP/runtime', 'ACP/notices']) fs.mkdirSync(path.join(root, d));
  const files = names.map(file => { fs.writeFileSync(path.join(root, file), 'product'); return { file, sha256: digest(Buffer.from('product')) }; });
  fs.writeFileSync(path.join(root, 'ACP/bin/unknown'), 'retain');
  fs.writeFileSync(path.join(root, 'profile.sqlite3'), 'retain');
  assert.throws(() => removeOwnedProduct(root, [...files, { file: 'ACP/bin/acp-daemon.exe', sha256: digest(Buffer.from('product')) }], removable, binaries));
  assert.throws(() => removeOwnedProduct(root, files.filter(f => !f.file.endsWith('acp-wsl-bridge')), removable, binaries));
  assert.deepEqual(removeOwnedProduct(root, files, removable, binaries).retained_directories, ['ACP/bin', 'ACP']);
  assert.deepEqual(fs.readdirSync(path.join(root, 'ACP/bin')), ['unknown']);
  assert.equal(fs.readFileSync(path.join(root, 'profile.sqlite3'), 'utf8'), 'retain');
});
