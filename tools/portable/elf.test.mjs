import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import { spawnSync } from 'node:child_process';
import { elfInfo, compareVersionTags, minimumVersions } from './elf.mjs';

const readelf = (args, file) => spawnSync('readelf', [...args, file], { encoding: 'utf8' });
const haveReadelf = !readelf(['--version'], '').error;

test('ELF facts match readelf on real dynamically linked binaries', { skip: !haveReadelf || process.platform !== 'linux' }, () => {
  for (const file of ['/bin/ls', '/usr/bin/env', new URL('../../target/debug/acp-daemon', import.meta.url).pathname]) {
    if (!fs.existsSync(file)) continue;
    const info = elfInfo(fs.readFileSync(file));
    const dynamic = readelf(['-d', '-W'], file).stdout;
    const needed = [...dynamic.matchAll(/\(NEEDED\)\s+Shared library: \[([^\]]+)\]/g)].map(m => m[1]);
    assert.deepEqual(info.needed, needed, file);
    assert.equal(info.rpath.length, [...dynamic.matchAll(/\((?:RPATH|RUNPATH)\)/g)].length, file);
    const interp = readelf(['-l', '-W'], file).stdout.match(/Requesting program interpreter: ([^\]]+)\]/)?.[1] ?? null;
    assert.equal(info.interpreter, interp, file);
    const verneed = readelf(['-V', '-W'], file).stdout;
    const required = new Set([...verneed.matchAll(/Name: ((?:GLIBC|GLIBCXX|CXXABI|GCC)_[0-9.]+)/g)].map(m => m[1]));
    const ours = new Set(Object.values(info.versions).flat().filter(t => /^(GLIBC|GLIBCXX|CXXABI|GCC)_[0-9]/.test(t)));
    assert.deepEqual([...ours].sort(), [...required].sort(), file);
  }
});

test('non-ELF, truncated, wrong class and wrong architecture are refused', () => {
  const ls = fs.existsSync('/bin/ls') ? fs.readFileSync('/bin/ls') : null;
  assert.throws(() => elfInfo(Buffer.from('#!/bin/sh\n')));
  assert.throws(() => elfInfo(Buffer.alloc(0)));
  if (ls) {
    for (const length of [16, 63, 64, 200]) assert.throws(() => elfInfo(ls.subarray(0, length)));
    const elf32 = Buffer.from(ls); elf32[4] = 1; assert.throws(() => elfInfo(elf32));
    const arm = Buffer.from(ls); arm.writeUInt16LE(183, 18); assert.throws(() => elfInfo(arm));
  }
});

test('version tags compare numerically and minimums take the highest per family', () => {
  assert.ok(compareVersionTags('GLIBC_2.9', 'GLIBC_2.17') < 0);
  assert.ok(compareVersionTags('GLIBC_2.34', 'GLIBC_2.34') === 0);
  assert.deepEqual(
    minimumVersions([{ versions: { 'libc.so.6': ['GLIBC_2.2.5', 'GLIBC_2.34'] } }, { versions: { 'libc.so.6': ['GLIBC_2.17'], 'libgcc_s.so.1': ['GCC_3.0', 'GCC_4.2.0'] } }]),
    { GLIBC: 'GLIBC_2.34', GCC: 'GCC_4.2.0' }
  );
});
