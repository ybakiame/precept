import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { apacheLicenseSha256, binaries, builderPathScan, cargoEnvironment, digest, freshDirectory, normalClosure, peImports,
  regular, removeOwnedProduct, vcRuntimeImports, verifyInventory } from './windows-package.mjs';

function fixture(t) {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'precept-package-test-'));
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
  const root = fixture(t); const file = path.join(root, 'preceptd.exe'); fs.writeFileSync(file, 'product');
  const expected = [{file:'preceptd.exe',size:7,sha256:digest(Buffer.from('product'))}];
  verifyInventory(root, expected);
  fs.writeFileSync(file, 'changed'); assert.throws(() => verifyInventory(root, expected)); fs.writeFileSync(file, 'product');
  fs.writeFileSync(path.join(root, 'operator.cap'), 'synthetic-not-a-token'); assert.throws(() => verifyInventory(root, expected));
  fs.linkSync(file, path.join(root, 'alias.exe')); assert.throws(() => regular(file));
});
test('shipping contract is exactly five product entrypoints', () => {
  assert.deepEqual([...binaries].sort(), ['precept-desktop.exe','precept-mcp.exe','precept-tui.exe','precept.exe','preceptd.exe']);
});

test('committed first-party LICENSE is the verbatim Apache-2.0 text and NOTICE names the holder', () => {
  const repo = fileURLToPath(new URL('../..', import.meta.url));
  assert.equal(digest(fs.readFileSync(path.join(repo, 'LICENSE'))), apacheLicenseSha256);
  assert.match(fs.readFileSync(path.join(repo, 'NOTICE'), 'utf8'), /^Precept\nCopyright 2026 ybakiame\n/);
});

test('VC++ prerequisite set follows actual imports; debug CRTs refuse; UCRT is not listed', () => {
  const record = (...names) => ({ imports: names.map(name => ({ name, delayed: false })) });
  const imports = vcRuntimeImports([
    record('kernel32.dll', 'vcruntime140.dll', 'api-ms-win-crt-runtime-l1-1-0.dll'),
    record('vcruntime140.dll', 'vcruntime140_1.dll', 'd3d11.dll')]);
  assert.deepEqual(imports, ['vcruntime140.dll', 'vcruntime140_1.dll']);
  assert.deepEqual(vcRuntimeImports([record('msvcp140_codecvt_ids.dll', 'vcruntime140_threads.dll')]), ['msvcp140_codecvt_ids.dll', 'vcruntime140_threads.dll']);
  for (const debug of ['vcruntime140d.dll', 'vcruntime140_1d.dll', 'msvcp140d.dll', 'ucrtbased.dll']) {
    assert.throws(() => vcRuntimeImports([record(debug)]));
  }
});

test('notice closure follows normal edges from shipped roots only; proc-macros are compile-time', () => {
  const pkg = (name, kind = 'lib') => ({ id: name, name, version: '1.0.0', targets: [{ kind: [kind] }] });
  const edge = (to, kind) => ({ pkg: to, dep_kinds: [{ kind, target: null }] });
  const metadata = {
    workspace_members: ['app', 'snapshot'],
    packages: [pkg('app', 'bin'), pkg('snapshot', 'bin'), pkg('linked'), pkg('derive', 'proc-macro'), pkg('derive-only'),
      pkg('test-only'), pkg('build-only'), pkg('snapshot-only')],
    resolve: { nodes: [
      { id: 'app', deps: [edge('linked', null), edge('derive', null), edge('test-only', 'dev'), edge('build-only', 'build')] },
      { id: 'snapshot', deps: [edge('snapshot-only', null)] },
      { id: 'linked', deps: [] }, { id: 'derive', deps: [edge('derive-only', null)] }, { id: 'derive-only', deps: [] },
      { id: 'test-only', deps: [] }, { id: 'build-only', deps: [] }, { id: 'snapshot-only', deps: [] }] } };
  const closure = normalClosure(metadata, ['app']);
  assert.deepEqual(closure.linked.map(p => p.name), ['app', 'linked']);
  assert.deepEqual(closure.compileTime.map(p => p.name), ['derive']);
  assert.throws(() => normalClosure(metadata, ['app', 'missing']));
  assert.throws(() => normalClosure(metadata, ['linked'])); // Not a workspace member.
});

test('cargo environment drops inherited flags, wrappers, target and profile overrides', () => {
  const env = cargoEnvironment({ Path: 'x', CARGO_HOME: 'h', RUSTFLAGS: '-Cfoo', rustc_wrapper: 'sccache', CARGO_TARGET_DIR: 't',
    CARGO_PROFILE_RELEASE_DEBUG: 'true', CARGO_BUILD_TARGET: 'other', CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_LINKER: 'evil', RUSTC: 'other' },
    ['--remap-path-prefix=a=b', '--remap-path-prefix=c d=e']);
  assert.deepEqual(Object.keys(env).sort(), ['CARGO_ENCODED_RUSTFLAGS', 'CARGO_HOME', 'CARGO_INCREMENTAL', 'CARGO_NET_OFFLINE',
    'Path', 'RUSTC_WORKSPACE_WRAPPER', 'RUSTC_WRAPPER']);
  assert.equal(env.CARGO_ENCODED_RUSTFLAGS, '--remap-path-prefix=a=b\x1f--remap-path-prefix=c d=e');
  assert.equal(env.RUSTC_WRAPPER, '');
});

test('builder path scan finds ASCII/UTF-16, either separator, any case, and reports booleans only', () => {
  const prefixes = { checkout: 'C:\\Users\\Dev\\src\\precept', user_home: 'C:\\Users\\Dev' };
  const clean = builderPathScan(Buffer.from('/precept/source/crates/x.rs /cargo/registry'), prefixes);
  assert.deepEqual(clean, { checkout: false, user_home: false });
  assert.deepEqual(builderPathScan(Buffer.from('panic at c:/users/dev/src/precept/crates'), prefixes), { checkout: true, user_home: true });
  assert.deepEqual(builderPathScan(Buffer.from('C:\\Users\\Dev\\.cargo', 'utf16le'), prefixes), { checkout: false, user_home: true });
});

function product(root) {
  const names = [...binaries.map(b => `Precept/bin/${b}`),
    'Precept/runtime/manifest.json', 'Precept/notices/dependency-notices.json', 'Precept/notices/NOTICE.txt',
    `Precept/notices/${'a'.repeat(64)}.txt`, 'Precept/README.txt', 'Precept/LICENSE.txt'];
  for (const dir of ['Precept', 'Precept/bin', 'Precept/runtime', 'Precept/notices']) fs.mkdirSync(path.join(root, dir));
  return names.map(file => { fs.writeFileSync(path.join(root, file), 'product'); return { file, sha256: digest(Buffer.from('product')) }; });
}
test('removal preflights every file, refuses forged paths, retains Profile/auth/unknown files', t => {
  const root = fixture(t);
  const files = product(root);
  for (const name of ['profile.sqlite3', 'operator.cap', 'unknown.txt']) fs.writeFileSync(path.join(root, name), 'retain');
  for (const forged of ['../operator.cap', 'Precept/../operator.cap', 'operator.cap', 'Precept/bin/evil.dll', 'Precept/bin/VCRUNTIME140.dll', 'Precept/runtime/operator.cap', 'Precept/bin/preceptd.exe.bak']) {
    assert.throws(() => removeOwnedProduct(root, [...files, { file: forged, sha256: digest(Buffer.from('retain')) }]));
  }
  const changed = path.join(root, 'Precept/README.txt'); fs.writeFileSync(changed, 'changed');
  assert.throws(() => removeOwnedProduct(root, files));
  assert.ok(fs.existsSync(path.join(root, 'Precept/bin', binaries[0]))); // No partial removal before preflight.
  fs.writeFileSync(changed, 'product');
  assert.deepEqual(removeOwnedProduct(root, files), { removed_files: files.length, retained_directories: [] });
  assert.deepEqual(fs.readdirSync(root).sort(), ['operator.cap', 'profile.sqlite3', 'unknown.txt']);
  for (const name of fs.readdirSync(root)) assert.equal(fs.readFileSync(path.join(root, name), 'utf8'), 'retain');
});
test('removal keeps a product directory holding an unknown file and never follows a directory alias', t => {
  const root = fixture(t);
  const files = product(root);
  fs.writeFileSync(path.join(root, 'Precept/bin/unknown.txt'), 'retain');
  assert.deepEqual(removeOwnedProduct(root, files).retained_directories, ['Precept/bin', 'Precept']);
  assert.deepEqual(fs.readdirSync(path.join(root, 'Precept')), ['bin']);
  assert.deepEqual(fs.readdirSync(path.join(root, 'Precept/bin')), ['unknown.txt']);

  const aliased = fixture(t);
  const outside = fixture(t);
  const listed = product(outside);
  fs.mkdirSync(path.join(aliased, 'Precept'));
  fs.symlinkSync(path.join(outside, 'Precept/bin'), path.join(aliased, 'Precept/bin'), process.platform === 'win32' ? 'junction' : 'dir');
  for (const dir of ['runtime', 'notices']) fs.cpSync(path.join(outside, 'Precept', dir), path.join(aliased, 'Precept', dir), { recursive: true });
  for (const file of ['README.txt', 'LICENSE.txt']) fs.copyFileSync(path.join(outside, 'Precept', file), path.join(aliased, 'Precept', file));
  assert.throws(() => removeOwnedProduct(aliased, listed));
  assert.equal(fs.readdirSync(path.join(outside, 'Precept/bin')).length, binaries.length);
});
