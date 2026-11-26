import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { binaries, binariesForTarget, checkElf, checkTarListing, gnuTarget, libraryResolutions, packageManagerFor, removable, target } from './linux-package.mjs';
import { digest, removeOwnedProduct } from './windows-package.mjs';

test('Linux targets select their binary sets and package databases', () => {
  assert.deepEqual(binariesForTarget(target), binaries.filter(binary => binary !== 'precept-desktop'));
  assert.deepEqual(binariesForTarget(gnuTarget), binaries);
  assert.equal(packageManagerFor({ ID: 'arch' }), 'pacman');
  assert.equal(packageManagerFor({ ID: 'linuxmint', ID_LIKE: 'ubuntu debian' }), 'dpkg');
  assert.throws(() => packageManagerFor({ ID: 'fedora' }));
});

test('ldd resolution names arrow and bare loaders by basename', () => {
  const resolved = libraryResolutions([
    '\tlibc.so.6 => /usr/lib/libc.so.6 (0x000070f9dac00000)',
    '\t/lib64/ld-linux-x86-64.so.2 => /usr/lib64/ld-linux-x86-64.so.2 (0x000070f9db8d9000)',
  ].join('\n'));
  assert.equal(resolved['libc.so.6'], '/usr/lib/libc.so.6');
  assert.equal(resolved['ld-linux-x86-64.so.2'], '/usr/lib64/ld-linux-x86-64.so.2');
  const bare = libraryResolutions('\t/lib64/ld-linux-x86-64.so.2 (0x000070f9db8d9000)');
  assert.equal(bare['ld-linux-x86-64.so.2'], '/lib64/ld-linux-x86-64.so.2');
});

test('ELF policy: system loader, glibc, and never RPATH/RUNPATH', () => {
  const ok = { interpreter: '/lib64/ld-linux-x86-64.so.2', rpath: [], needed: ['libc.so.6', 'libgtk-4.so.1'] };
  assert.equal(checkElf('x', ok), ok);
  assert.equal(checkElf('musl', { interpreter: null, rpath: [], needed: [] }).needed.length, 0);
  assert.throws(() => checkElf('x', { ...ok, rpath: ['$ORIGIN/../lib'] }));
  assert.throws(() => checkElf('x', { ...ok, interpreter: '/tmp/ld.so' }));
  assert.throws(() => checkElf('x', { ...ok, needed: ['libgtk-4.so.1'] }));
});

test('tar listing: Precept paths only, no links/devices/traversal, exact modes', () => {
  const good = [
    'drwxr-xr-x 0/0 0 2026-11-17 00:00 Precept/',
    'drwxr-xr-x 0/0 0 2026-11-17 00:00 Precept/bin/',
    '-rwxr-xr-x 0/0 9 2026-11-17 00:00 Precept/bin/preceptd',
    '-rw-r--r-- 0/0 9 2026-11-17 00:00 Precept/README.txt',
  ];
  assert.equal(checkTarListing(good).length, 4);
  for (const bad of [
    'lrwxrwxrwx 0/0 0 2026-11-17 00:00 Precept/bin/preceptd -> /bin/sh',
    'crw-r--r-- 0/0 0 2026-11-17 00:00 Precept/dev',
    '-rw-r--r-- 0/0 9 2026-11-17 00:00 Precept/../etc/passwd',
    '-rw-r--r-- 0/0 9 2026-11-17 00:00 /etc/passwd',
    '-rw-r--r-- 0/0 9 2026-11-17 00:00 other/file',
    '-rw-r--r-- 0/0 9 2026-11-17 00:00 Precept/bin/preceptd',
    '-rwxrwxrwx 0/0 9 2026-11-17 00:00 Precept/README.txt',
    '-rwsr-xr-x 0/0 9 2026-11-17 00:00 Precept/bin/preceptd',
  ]) assert.throws(() => checkTarListing([bad]), bad);
});

test('exact removal with the Linux allowlist keeps unknown, Profile and auth files', t => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'precept-linux-removal-'));
  t.after(() => fs.rmSync(root, { recursive: true }));
  const names = [...binaries.map(b => `Precept/bin/${b}`), 'Precept/runtime/manifest.json', 'Precept/notices/NOTICE.txt',
    'Precept/notices/dependency-notices.json', 'Precept/README.txt', 'Precept/LICENSE.txt'];
  for (const d of ['Precept', 'Precept/bin', 'Precept/runtime', 'Precept/notices']) fs.mkdirSync(path.join(root, d));
  const files = names.map(file => { fs.writeFileSync(path.join(root, file), 'product'); return { file, sha256: digest(Buffer.from('product')) }; });
  fs.writeFileSync(path.join(root, 'Precept/bin/unknown'), 'retain');
  fs.writeFileSync(path.join(root, 'profile.sqlite3'), 'retain');
  assert.throws(() => removeOwnedProduct(root, [...files, { file: 'Precept/bin/preceptd.exe', sha256: digest(Buffer.from('product')) }], removable, binaries));
  assert.throws(() => removeOwnedProduct(root, files.filter(f => !f.file.endsWith('precept-bridge')), removable, binaries));
  assert.deepEqual(removeOwnedProduct(root, files, removable, binaries).retained_directories, ['Precept/bin', 'Precept']);
  assert.deepEqual(fs.readdirSync(path.join(root, 'Precept/bin')), ['unknown']);
  assert.equal(fs.readFileSync(path.join(root, 'profile.sqlite3'), 'utf8'), 'retain');
});
