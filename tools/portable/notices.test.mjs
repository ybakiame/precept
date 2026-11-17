import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { createHash } from 'node:crypto';
import { fileURLToPath } from 'node:url';
import { gitCheckoutRoot, licensePolicy, noticeLedger, loadSupplements } from './notices.mjs';

const repo = fileURLToPath(new URL('../..', import.meta.url));
const sha = b => createHash('sha256').update(b).digest('hex');

test('license policy: permissive choices, legacy slash, WITH exceptions, refusals', () => {
  for (const ok of ['MIT', 'MIT/Apache-2.0', 'Apache-2.0 / MIT', '(MIT OR Apache-2.0) AND Unicode-3.0',
    'Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT', 'Apache-2.0 WITH LLVM-exception', 'CC0-1.0 OR MIT-0 OR Apache-2.0',
    'Apache-2.0 OR BSL-1.0', 'CDLA-Permissive-2.0', 'Unlicense/MIT'])
    assert.equal(licensePolicy(ok).permissive, true, ok);
  assert.equal(licensePolicy('GPL-3.0').permissive, false);
  assert.equal(licensePolicy('GPL-3.0 OR MIT').permissive, true);
  assert.equal(licensePolicy('GPL-3.0 AND MIT').permissive, false);
  assert.equal(licensePolicy('MPL-2.0').permissive, false);
  assert.equal(licensePolicy('Apache-2.0 WITH Classpath-exception-2.0').permissive, false);
  assert.deepEqual(licensePolicy('(MIT OR Apache-2.0) AND Unicode-3.0').ids, ['Apache-2.0', 'MIT', 'Unicode-3.0']);
  for (const bad of ['', 'MIT OR', '(MIT', 'MIT, Apache-2.0', 'MIT AND (Apache-2.0', null]) assert.throws(() => licensePolicy(bad), String(bad));
});

test('git checkout root detection is limited to Cargo git checkouts', () => {
  assert.equal(gitCheckoutRoot('/c/.cargo/git/checkouts/nsh-9218/b7f31a0/crates/nsh-core/Cargo.toml'), '/c/.cargo/git/checkouts/nsh-9218/b7f31a0');
  assert.equal(gitCheckoutRoot('C:\\c\\.cargo\\git\\checkouts\\nsh-9218\\b7f31a0\\crates\\x\\Cargo.toml'), 'C:\\c\\.cargo\\git\\checkouts\\nsh-9218\\b7f31a0');
  assert.equal(gitCheckoutRoot('/c/.cargo/registry/src/index/x-1.0.0/Cargo.toml'), null);
});

test('committed supplements are hash-exact and pinned to GitHub commits', () => {
  const { dir, map } = loadSupplements(repo);
  assert.ok(map.size >= 13);
  for (const [key, e] of map) {
    if (e.no_text_required) continue;
    for (const t of e.texts) {
      if (t.repository_license) { assert.equal(e.election, 'Apache-2.0', key); continue; }
      assert.match(t.source, /^https:\/\/github\.com\/[^/]+\/[^/]+\/blob\/[0-9a-f]{40}\//, key);
      assert.equal(sha(fs.readFileSync(path.join(dir, t.file))), t.sha256, key);
    }
  }
});

function fixture(t) {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'acp-notices-'));
  t.after(() => fs.rmSync(root, { recursive: true }));
  const crate = (rel, files = {}) => {
    const d = path.join(root, rel);
    fs.mkdirSync(d, { recursive: true });
    for (const [n, c] of Object.entries(files)) fs.writeFileSync(path.join(d, n), c);
    return path.join(d, 'Cargo.toml');
  };
  const notices = path.join(root, 'notices');
  fs.mkdirSync(notices);
  return { root, crate, notices, write: (f, b) => fs.writeFileSync(f, b, { flag: 'wx' }) };
}
const pkg = (name, version, license, manifest_path, source = 'registry+https://github.com/rust-lang/crates.io-index') =>
  ({ name, version, license, manifest_path, source });

test('ledger resolves crate-local, checkout-root, supplement, election and no-text sources', t => {
  const f = fixture(t);
  const entries = [
    { workspace: 'w', linkage: 'linked', pkg: pkg('first', '0.0.0', 'Apache-2.0', f.crate('first'), null) },
    { workspace: 'w', linkage: 'linked', pkg: pkg('local', '1.0.0', 'MIT', f.crate('reg/local', { 'LICENSE-MIT': 'MIT text' })) },
    { workspace: 'w', linkage: 'linked', pkg: pkg('nsh-core', '0.5.0', 'Apache-2.0',
      f.crate('git/checkouts/nsh-1/abc/crates/nsh-core'), 'git+https://github.com/longbridge/nsh?rev=abc') },
    { workspace: 'w', linkage: 'linked', pkg: pkg('taffy', '0.15.1', 'MIT', f.crate('reg/taffy')) },
    { workspace: 'w', linkage: 'linked', pkg: pkg('mac', '0.1.1', 'MIT/Apache-2.0', f.crate('reg/mac')) },
    { workspace: 'w', linkage: 'linked', pkg: pkg('hexf-parse', '0.2.1', 'CC0-1.0', f.crate('reg/hexf')) },
    { workspace: 'w', linkage: 'linked', pkg: pkg('ratex-font', '0.1.15', 'MIT', f.crate('reg/ratex-font')) },
  ];
  fs.writeFileSync(path.join(f.root, 'git/checkouts/nsh-1/abc/LICENSE-APACHE'), 'Apache text');
  const ledger = noticeLedger(entries, f.notices, repo, f.write);
  const by = Object.fromEntries(ledger.packages.map(p => [p.package, p]));
  assert.match(by.first.review, /^FIRST-PARTY/);
  assert.equal(by.local.notice_source, 'crate-local');
  assert.equal(by['nsh-core'].notice_source, 'pinned-git-checkout-root');
  assert.equal(by.taffy.notice_source, 'reviewed upstream text pinned to commit');
  assert.equal(by.mac.notice_source, 'elected Apache-2.0; verbatim license text');
  assert.equal(by.mac.texts[0].sha256, sha(fs.readFileSync(path.join(repo, 'LICENSE'))));
  assert.equal(by['hexf-parse'].notice_source, 'no-notice-required (CC0-1.0)');
  assert.equal(by['hexf-parse'].texts.length, 0);
  assert.equal(by['ratex-font'].texts.length, 3, 'MIT + KaTeX font notice + SIL OFL');
  assert.equal(ledger.summary.reviewed, 6);
  for (const p of ledger.packages) for (const x of p.texts)
    assert.equal(sha(fs.readFileSync(path.join(f.notices, path.basename(x.file)))), x.sha256);
});

test('ledger refuses unresolved texts, other versions, non-permissive licenses', t => {
  const f = fixture(t);
  const one = p => noticeLedger([{ workspace: 'w', linkage: 'linked', pkg: p }], f.notices, repo, f.write);
  assert.throws(() => one(pkg('unknown', '1.0.0', 'MIT', f.crate('reg/unknown'))), /no notice text and no reviewed supplement/);
  assert.throws(() => one(pkg('taffy', '0.15.2', 'MIT', f.crate('reg/taffy2'))), /no reviewed supplement/);
  assert.throws(() => one(pkg('gpl', '1.0.0', 'GPL-3.0', f.crate('reg/gpl', { LICENSE: 'x' }))), /outside the permissive policy/);
  assert.throws(() => one(pkg('hexf-parse', '0.2.1', 'MIT', f.crate('reg/hexf2'))), /no-text license not declared/);
});
