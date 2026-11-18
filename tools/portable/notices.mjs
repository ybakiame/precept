// Third-party notice ledger shared by the Windows and Linux portable recipes (G11).
// Every shipped-closure package must have a permissive declared license and a
// resolved notice source; anything else refuses packaging instead of shipping a
// "pending" entry. This is a notice ledger, not a linker-derived SBOM.
import fs from 'node:fs';
import path from 'node:path';
import { createHash } from 'node:crypto';

export class NoticeError extends Error {}
const need = (ok, message) => { if (!ok) throw new NoticeError(message); };
const digest = bytes => createHash('sha256').update(bytes).digest('hex');

/// Permissive licenses accepted for redistribution in the portable archives.
export const PERMISSIVE = new Set(['MIT', 'MIT-0', 'Apache-2.0', 'BSD-2-Clause', 'BSD-3-Clause', 'ISC', 'Zlib',
  'Unicode-3.0', 'Unicode-DFS-2016', 'CC0-1.0', 'Unlicense', '0BSD', 'BSL-1.0', 'CDLA-Permissive-2.0']);
/// Allowed `WITH` exceptions and the licenses each may modify; anything else refuses.
export const EXCEPTIONS = new Map([['LLVM-exception', new Set(['Apache-2.0'])]]);

/// Parses an SPDX-style expression (legacy `/` read as OR) and returns the license
/// ids it names plus whether some choice consists of permissive licenses only.
export function licensePolicy(expression) {
  need(typeof expression === 'string' && expression.length > 0 && expression.length <= 256, 'Missing declared license');
  const tokens = expression.replace(/\s*\/\s*/g, ' OR ').match(/\(|\)|[A-Za-z0-9.+-]+/g) ?? [];
  need(tokens.join('').length === expression.replace(/\s*\/\s*/g, 'OR').replace(/\s+/g, '').length, `Unparsable license expression: ${expression}`);
  let at = 0;
  const ids = new Set();
  const peek = () => tokens[at];
  function factor() {
    if (peek() === '(') { at++; const v = or(); need(tokens[at++] === ')', `Unbalanced license expression: ${expression}`); return v; }
    const id = tokens[at++];
    need(id && !['AND', 'OR', 'WITH', ')'].includes(id), `Unparsable license expression: ${expression}`);
    ids.add(id);
    let ok = PERMISSIVE.has(id);
    if (peek() === 'WITH') {
      at++;
      const ex = tokens[at++];
      need(ex && !['AND', 'OR', 'WITH', '(', ')'].includes(ex), `Missing license exception: ${expression}`);
      // Fail closed even when another OR branch would be permissive.
      need(EXCEPTIONS.has(ex), `Unknown license exception ${ex}: ${expression}`);
      need(EXCEPTIONS.get(ex).has(id), `License exception ${ex} does not apply to ${id}: ${expression}`);
    }
    return ok;
  }
  function and() { let v = factor(); while (peek() === 'AND') { at++; v = factor() && v; } return v; }
  function or() { let v = and(); while (peek() === 'OR') { at++; v = and() || v; } return v; }
  const permissive = or();
  need(at === tokens.length, `Unparsable license expression: ${expression}`);
  return { permissive, ids: [...ids].sort() };
}

// License texts and attribution NOTICE files are distinct: a directory resolves a
// package only when it holds at least one non-empty license text. NOTICE files ship
// alongside, never instead. Empty or whitespace-only candidates refuse.
const LICENSE_FILE = /^(licen[sc]e|copying|unlicense)([.-]|$)/i;
const ATTRIBUTION_FILE = /^notice([.-]|$)/i;
function noticeFiles(directory) {
  const found = fs.readdirSync(directory).sort().flatMap(name => {
    const kind = LICENSE_FILE.test(name) ? 'license' : ATTRIBUTION_FILE.test(name) ? 'notice' : null;
    if (!kind) return [];
    const file = path.join(directory, name);
    const info = fs.lstatSync(file);
    if (!info.isFile() || info.isSymbolicLink()) return [];
    const bytes = fs.readFileSync(file);
    need(bytes.toString('utf8').trim().length > 0, `Empty ${kind} file ${name}`);
    return [{ name, kind, bytes }];
  });
  return found.some(f => f.kind === 'license') ? found : [];
}
/// Splits a pinned GitHub blob URL into repository, full commit and upstream path.
export function githubProvenance(url) {
  const m = /^https:\/\/github\.com\/([^/]+\/[^/]+)\/blob\/([0-9a-f]{40})\/(.+)$/.exec(url ?? '');
  need(m, `Supplement source is not a commit-pinned GitHub blob URL: ${url}`);
  return { url, repository: `https://github.com/${m[1]}`, commit: m[2], path: m[3] };
}
/// For a Cargo git dependency, the pinned checkout root (…/git/checkouts/<repo>/<rev>).
export function gitCheckoutRoot(manifestPath) {
  const m = /^(.*[\\/]git[\\/]checkouts[\\/][^\\/]+[\\/][^\\/]+)[\\/]/.exec(manifestPath);
  return m ? m[1] : null;
}

export function loadSupplements(repo) {
  const dir = path.join(repo, 'tools/portable/notice-supplements');
  const doc = JSON.parse(fs.readFileSync(path.join(dir, 'supplements.json'), 'utf8'));
  need(doc.format === 1 && Array.isArray(doc.entries), 'Invalid notice supplements');
  const map = new Map();
  for (const e of doc.entries) {
    const key = `${e.package}@${e.version}`;
    need(!map.has(key) && typeof e.reason === 'string' && e.reason.length > 0, `Invalid supplement ${key}`);
    // Exactly one of: a non-empty text list, or an explicit no-text license.
    const hasTexts = Array.isArray(e.texts) && e.texts.length > 0;
    const noText = typeof e.no_text_required === 'string' && e.no_text_required.length > 0;
    need(hasTexts !== noText && (noText ? e.texts === undefined : true), `Supplement ${key} needs non-empty texts or a no-text license`);
    map.set(key, e);
  }
  return { dir, map };
}

/// `entries`: [{ workspace, linkage, pkg }] with Cargo metadata packages.
/// Writes deduplicated `<sha256>.txt` texts into `noticesDir` and returns the ledger.
export function noticeLedger(entries, noticesDir, repo, write) {
  const { dir, map } = loadSupplements(repo);
  const repositoryLicense = fs.readFileSync(path.join(repo, 'LICENSE'));
  const store = (bytes, provenance) => {
    const hash = digest(bytes), name = `${hash}.txt`;
    if (!fs.existsSync(path.join(noticesDir, name))) write(path.join(noticesDir, name), bytes);
    return { file: `notices/${name}`, sha256: hash, ...provenance };
  };
  const used = new Set();
  const ledger = { scope: 'normal-dependency closure of shipped packages for target/features; NOT a linker-derived SBOM',
    policy: { permissive: [...PERMISSIVE].sort(), exceptions: Object.fromEntries([...EXCEPTIONS].map(([ex, ids]) => [ex, [...ids].sort()])) }, packages: [] };
  for (const { workspace, linkage, pkg } of entries) {
    const source = pkg.source ? pkg.source.replace(/#.*$/, '') : 'first-party';
    const commit = pkg.source?.startsWith('git+') ? /#([0-9a-f]{40})$/.exec(pkg.source)?.[1] : undefined;
    const base = { workspace, package: pkg.name, version: pkg.version, linkage, source,
      ...(commit ? { git_commit: commit } : {}), declared_license: pkg.license };
    if (!pkg.source) {
      ledger.packages.push({ ...base, texts: [], review: 'FIRST-PARTY: Apache-2.0, see LICENSE.txt and notices/NOTICE.txt' });
      continue;
    }
    const policy = licensePolicy(pkg.license);
    need(policy.permissive, `${pkg.name} ${pkg.version}: license outside the permissive policy (${pkg.license})`);
    const local = (f, at) => store(f.bytes, { kind: f.kind, upstream_path: at ? `${at}/${f.name}` : f.name });
    let texts = noticeFiles(path.dirname(pkg.manifest_path)).map(f => local(f));
    let origin = 'crate-local';
    let note;
    const root = gitCheckoutRoot(pkg.manifest_path);
    if (!texts.length && root && pkg.source.startsWith('git+')) {
      // Walk up from the crate to the pinned checkout root; first directory with texts wins.
      for (let d = path.dirname(path.dirname(pkg.manifest_path)); d.length >= root.length; d = path.dirname(d)) {
        const found = noticeFiles(d);
        if (found.length) {
          const at = path.relative(root, d).split(path.sep).join('/');
          texts = found.map(f => local(f, at));
          origin = 'pinned-git-checkout-root';
          break;
        }
        if (d === root) break;
      }
    }
    if (!texts.length) {
      const s = map.get(`${pkg.name}@${pkg.version}`);
      need(s, `${pkg.name} ${pkg.version}: no notice text and no reviewed supplement`);
      used.add(`${pkg.name}@${pkg.version}`);
      note = s.reason;
      if (s.no_text_required) {
        need(policy.ids.includes(s.no_text_required), `${pkg.name}: no-text license not declared`);
        origin = `no-notice-required (${s.no_text_required})`;
      } else {
        if (s.election) need(policy.ids.includes(s.election) && PERMISSIVE.has(s.election), `${pkg.name}: invalid election`);
        texts = s.texts.map(t => {
          if (t.repository_license) {
            need(s.election === 'Apache-2.0', `${pkg.name}: repository license reuse needs an Apache-2.0 election`);
            return store(repositoryLicense, { kind: 'license', upstream: 'ACP repository LICENSE (shipped as LICENSE.txt); verbatim Apache-2.0 text' });
          }
          need(/^[a-f0-9]{64}\.txt$/.test(t.file ?? ''), `${pkg.name}: invalid supplement text`);
          const upstream = githubProvenance(t.source);
          const bytes = fs.readFileSync(path.join(dir, t.file));
          need(bytes.toString('utf8').trim().length > 0, `${pkg.name}: empty supplement text`);
          need(digest(bytes) === t.sha256 && t.file === `${t.sha256}.txt`, `${pkg.name}: supplement text hash mismatch`);
          need(t.kind === undefined || t.kind === 'notice', `${pkg.name}: invalid supplement text kind`);
          return store(bytes, { kind: t.kind ?? 'license', upstream });
        });
        need(texts.some(t => t.kind === 'license'), `${pkg.name}: supplement has no license text`);
        origin = s.election ? `elected ${s.election}; verbatim license text` : 'reviewed upstream text pinned to commit';
      }
    }
    ledger.packages.push({ ...base, texts, notice_source: origin, ...(note ? { note } : {}),
      review: `REVIEWED: permissive (${policy.ids.join(', ')}); notice source: ${origin}` });
  }
  ledger.summary = {
    first_party: ledger.packages.filter(p => p.source === 'first-party').length,
    third_party: ledger.packages.filter(p => p.source !== 'first-party').length,
    reviewed: ledger.packages.filter(p => p.review.startsWith('REVIEWED')).length,
    by_notice_source: Object.fromEntries([...new Set(ledger.packages.filter(p => p.notice_source).map(p => p.notice_source))]
      .sort().map(k => [k, ledger.packages.filter(p => p.notice_source === k).length])),
    supplements_used: [...used].sort(),
  };
  return ledger;
}
