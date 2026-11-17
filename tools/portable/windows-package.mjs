// Developer-only Windows portable recipe. No install, activation or publish.
//
// Archive layout (all entries under one top-level product directory):
//   ACP/bin/                    five product executables only
//   ACP/runtime/manifest.json   provenance, prerequisites, every other file's size/SHA-256
//   ACP/notices/                NOTICE.txt, dependency ledger, notice texts
//   ACP/README.txt, ACP/LICENSE.txt (Apache-2.0)
// The Microsoft Visual C++ v14 runtime is a documented user prerequisite, not bundled.
import fs from 'node:fs';
import path from 'node:path';
import os from 'node:os';
import crypto from 'node:crypto';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

export const binaries = ['acp-daemon.exe', 'acp-query.exe', 'acp-mcp.exe', 'acp-nsh-native.exe', 'acp-nsh-tui.exe'];
export const product = 'ACP';
export const pin = 'b7f31a08478be262fd7b76ec856036fd5b8ffdb0';
export const target = 'x86_64-pc-windows-msvc';
// SHA-256 of the canonical https://www.apache.org/licenses/LICENSE-2.0.txt (11,358 bytes).
export const apacheLicenseSha256 = 'cfc7749b96f63bd31c3c42b5c471bf756814053e847c10f3eb003417bc523d30';
export const digest = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
export class PolicyError extends Error {}
function requireValue(value, message) { if (!value) throw new PolicyError(message); }

export function regular(file) {
  const info = fs.lstatSync(file);
  requireValue(info.isFile() && !info.isSymbolicLink() && info.nlink === 1, 'Expected unaliased regular file');
  return info;
}
function realDirectoryChain(directory) {
  // Resolve every existing component; refuse junctions/symlinks rather than following them.
  for (let part = path.resolve(directory);; part = path.dirname(part)) {
    const info = fs.lstatSync(part);
    requireValue(info.isDirectory() && !info.isSymbolicLink(), 'Unsafe directory alias');
    if (part === path.dirname(part)) break;
  }
}
export function freshDirectory(parent, leaf) {
  requireValue(/^[a-zA-Z0-9][a-zA-Z0-9._-]{0,95}$/.test(leaf) && !leaf.endsWith('.'), 'Unsafe output name');
  realDirectoryChain(parent);
  const result = path.join(path.resolve(parent), leaf);
  fs.mkdirSync(result); // No recursive creation, overwrite, adoption or deletion.
  return result;
}

export function peImports(bytes) {
  const read16 = at => { requireValue(at >= 0 && at + 2 <= bytes.length, 'Truncated PE'); return bytes.readUInt16LE(at); };
  const read32 = at => { requireValue(at >= 0 && at + 4 <= bytes.length, 'Truncated PE'); return bytes.readUInt32LE(at); };
  requireValue(read16(0) === 0x5a4d, 'Not PE');
  const pe = read32(0x3c);
  requireValue(read32(pe) === 0x4550 && read16(pe + 4) === 0x8664, 'Expected x86_64 PE');
  const count = read16(pe + 6), optionalSize = read16(pe + 20), optional = pe + 24;
  requireValue(count > 0 && count <= 96 && optionalSize >= 240 && read16(optional) === 0x20b, 'Invalid PE32+');
  requireValue(read32(optional + 108) >= 14, 'Missing PE directory table');
  const sections = optional + optionalSize;
  const offset = rva => {
    for (let i = 0; i < count; i++) {
      const at = sections + i * 40, start = read32(at + 12), size = read32(at + 16);
      if (rva >= start && rva - start < size) {
        const result = read32(at + 20) + rva - start;
        requireValue(result < bytes.length, 'PE RVA outside file');
        return result;
      }
    }
    throw new PolicyError('Unmapped PE RVA');
  };
  const dllName = rva => {
    const start = offset(rva), end = bytes.indexOf(0, start);
    requireValue(end > start && end - start <= 128, 'Invalid PE import name');
    const name = bytes.toString('ascii', start, end).toLowerCase();
    requireValue(/^[a-z0-9_.-]+\.dll$/.test(name), 'Unsafe PE import name');
    return name;
  };
  const imports = [];
  for (const [directory, stride, nameOffset, delayed] of [[1, 20, 12, false], [13, 32, 4, true]]) {
    const rva = read32(optional + 112 + directory * 8), size = read32(optional + 116 + directory * 8);
    if (!rva) { requireValue(size === 0, 'Invalid PE directory'); continue; }
    requireValue(size >= stride && size <= 1024 * 1024, 'Invalid PE import directory');
    let terminated = false;
    for (let i = 0; i < Math.floor(size / stride); i++) {
      const at = offset(rva + i * stride);
      requireValue(at + stride <= bytes.length, 'Truncated PE import descriptor');
      if (bytes.subarray(at, at + stride).every(x => x === 0)) { terminated = true; break; }
      if (delayed) requireValue(read32(at) === 1, 'Unsupported delay import address form');
      imports.push({ name: dllName(read32(at + nameOffset)), delayed });
    }
    requireValue(terminated, 'Unterminated PE import directory');
  }
  return { machine: 'AMD64', subsystem: read16(optional + 68), imports };
}

// VC++ runtime DLLs required by the ACTUAL PE imports. They are not Windows
// components and are not bundled: the user installs the Microsoft Visual C++
// v14 Redistributable, at least as new as the MSVC build tools used here.
// Debug CRTs refuse. UCRT (api-ms-win-crt-*) is an OS component on Windows 10+.
const vcRelease = /^(vcruntime|msvcp|concrt|vccorlib)140(_[a-z0-9_]+)?\.dll$/;
const vcDebug = /^((vcruntime|msvcp|concrt|vccorlib)140(_[a-z0-9]+)?d|ucrtbased)\.dll$/;
export function vcRuntimeImports(records) {
  const names = new Set();
  for (const record of records) {
    for (const { name } of record.imports) {
      requireValue(!vcDebug.test(name), 'Debug C++ runtime import refused');
      if (vcRelease.test(name)) names.add(name);
    }
  }
  return [...names].sort();
}

// Packages linked into shipped binaries: normal-dependency closure from the
// shipped package roots, for the resolved target/features. Build-dependencies
// and dev-dependencies are excluded. Proc-macro crates run only at compile
// time; they are recorded separately and not traversed.
export function normalClosure(metadata, rootNames) {
  const members = new Set(metadata.workspace_members);
  const byId = new Map(metadata.packages.map(p => [p.id, p]));
  const nodes = new Map(metadata.resolve.nodes.map(n => [n.id, n]));
  const roots = metadata.packages.filter(p => members.has(p.id) && rootNames.includes(p.name));
  requireValue(roots.length === rootNames.length, 'Shipping package root missing');
  const linked = new Set(), compileTime = new Set(), queue = roots.map(p => p.id);
  while (queue.length) {
    const id = queue.pop();
    if (linked.has(id) || compileTime.has(id)) continue;
    const pkg = byId.get(id);
    requireValue(pkg && nodes.has(id), 'Unresolved dependency node');
    if (pkg.targets.some(t => t.kind.includes('proc-macro'))) { compileTime.add(id); continue; }
    linked.add(id);
    for (const dep of nodes.get(id).deps) {
      requireValue(Array.isArray(dep.dep_kinds), 'Cargo metadata lacks dependency kinds');
      if (dep.dep_kinds.some(k => k.kind === null)) queue.push(dep.pkg);
    }
  }
  const order = (a, b) => a.name.localeCompare(b.name) || a.version.localeCompare(b.version);
  return { linked: [...linked].map(id => byId.get(id)).sort(order), compileTime: [...compileTime].map(id => byId.get(id)).sort(order) };
}

// Cargo inputs that would silently change the build are removed; the recipe's
// own flags are passed in the encoded form, which outranks all config rustflags.
// An empty wrapper value overrides any configured rustc wrapper.
const inheritedBuildInput = /^(RUSTFLAGS|RUSTDOCFLAGS|CARGO_ENCODED_RUSTFLAGS|CARGO_ENCODED_RUSTDOCFLAGS|RUSTC|RUSTDOC|RUSTC_WRAPPER|RUSTC_WORKSPACE_WRAPPER|RUSTC_BOOTSTRAP|CARGO_INCREMENTAL|CARGO_BUILD_.*|CARGO_PROFILE_.*|CARGO_TARGET_.*)$/i;
export function cargoEnvironment(base, rustflags) {
  const env = {};
  for (const [key, value] of Object.entries(base)) if (!inheritedBuildInput.test(key)) env[key] = value;
  return { ...env, CARGO_ENCODED_RUSTFLAGS: rustflags.join('\x1f'), RUSTC_WRAPPER: '', RUSTC_WORKSPACE_WRAPPER: '',
    CARGO_INCREMENTAL: '0', CARGO_NET_OFFLINE: 'true' };
}

// Reports only whether builder-private prefixes remain in shipped bytes (ASCII
// or UTF-16LE, either separator, case-insensitive); never the prefixes themselves.
export function builderPathScan(bytes, prefixes) {
  const haystack = bytes.toString('latin1').toLowerCase();
  const wide = Buffer.from(haystack, 'latin1');
  const result = {};
  for (const [label, prefix] of Object.entries(prefixes)) {
    const forms = new Set([prefix, prefix.replaceAll('\\', '/'), prefix.replaceAll('/', '\\')].map(p => p.toLowerCase()));
    result[label] = [...forms].some(form => haystack.includes(form) || wide.includes(Buffer.from(form, 'utf16le')));
  }
  return result;
}

function command(program, args, cwd, logFile, env) {
  const result = spawnSync(program, args, { cwd, env, encoding: 'utf8', maxBuffer: 256 * 1024 * 1024, windowsHide: true });
  if (logFile) fs.writeFileSync(logFile, (result.stdout ?? '') + (result.stderr ?? ''), { flag: 'wx' });
  requireValue(!result.error && result.status === 0, `Developer command failed: ${path.basename(program)} status=${result.status} code=${result.error?.code ?? 'exit'}; inspect retained private log`);
  return result.stdout.trim();
}
export function inventory(root) {
  const files = [];
  function walk(relative) {
    for (const name of fs.readdirSync(path.join(root, relative)).sort()) {
      requireValue(/^[A-Za-z0-9][A-Za-z0-9._-]*$/.test(name), 'Unsafe archive member');
      const next = path.posix.join(relative, name), disk = path.join(root, next), stat = fs.lstatSync(disk);
      requireValue(!stat.isSymbolicLink(), 'Archive alias refused');
      if (stat.isDirectory()) walk(next);
      else { regular(disk); files.push({ file: next, size: stat.size, sha256: digest(fs.readFileSync(disk)) }); }
    }
  }
  walk(''); return files;
}
export function verifyInventory(root, expected) {
  requireValue(JSON.stringify(inventory(root)) === JSON.stringify(expected), 'Archive inventory mismatch');
}

// Explicit removal of an owned extracted prototype, never product-data cleanup.
// Paths are relative to the extraction root, so they start with `ACP/`.
const removable = /^ACP\/(bin\/acp-(daemon|query|mcp|nsh-native|nsh-tui)\.exe|runtime\/manifest\.json|notices\/(NOTICE\.txt|dependency-notices\.json|[a-f0-9]{64}\.txt)|README\.txt|LICENSE\.txt)$/;
const productDirectories = ['ACP/bin', 'ACP/runtime', 'ACP/notices', 'ACP'];
/// `allowed`/`required` default to the Windows layout; the Linux recipe passes its own.
export function removeOwnedProduct(root, files, allowed = removable, required = binaries) {
  requireValue(Array.isArray(files) && files.length >= required.length + 4 && files.length <= 10000, 'Invalid removal inventory');
  requireValue(required.every(name => files.some(f => f.file === `ACP/bin/${name}`)), 'Incomplete removal inventory');
  realDirectoryChain(root);
  const seen = new Set();
  for (const f of files) {
    requireValue(allowed.test(f.file) && !seen.has(f.file.toLowerCase()) && /^[a-f0-9]{64}$/.test(f.sha256), 'Unsafe removal member');
    seen.add(f.file.toLowerCase());
    const file = path.join(root, f.file);
    realDirectoryChain(path.dirname(file));
    regular(file);
    requireValue(digest(fs.readFileSync(file)) === f.sha256, 'Changed product file retained');
  }
  // Preflight every member before the first unlink. Unknown files are never enumerated.
  for (const f of files) fs.unlinkSync(path.join(root, f.file));
  // Known product directories are removed only when already empty (rmdir is non-recursive).
  const retained = [];
  for (const directory of productDirectories) {
    try { fs.rmdirSync(path.join(root, directory)); } catch (error) {
      if (error.code !== 'ENOTEMPTY' && error.code !== 'EEXIST' && error.code !== 'ENOENT') throw error;
      if (error.code !== 'ENOENT') retained.push(directory);
    }
  }
  return { removed_files: files.length, retained_directories: retained };
}

function cargoHome() { return process.env.CARGO_HOME || path.join(os.homedir(), '.cargo'); }

export function build(repo, parent, name, cargoProgram) {
  requireValue(process.platform === 'win32' && process.arch === 'x64', 'Native Windows x86_64 required');
  requireValue(!command('git', ['status', '--porcelain', '--untracked-files=all'], repo), 'Clean committed source required');
  const sha = command('git', ['rev-parse', 'HEAD'], repo), tree = command('git', ['rev-parse', 'HEAD^{tree}'], repo);
  requireValue(/^[a-f0-9]{40}$/.test(sha) && /^[a-f0-9]{40}$/.test(tree), 'Invalid source identity');
  const vcTools = process.env.VCToolsVersion?.trim();
  requireValue(/^14\.\d+\.\d+$/.test(vcTools ?? ''), 'MSVC v14 build tools environment unavailable; no installation attempted');
  // The first-party grant is committed source; only the verbatim Apache-2.0 text ships.
  const license = fs.readFileSync(path.join(repo, 'LICENSE'));
  requireValue(digest(license) === apacheLicenseSha256, 'LICENSE is not the verbatim Apache-2.0 text');
  const notice = fs.readFileSync(path.join(repo, 'NOTICE'));
  const report = freshDirectory(parent, name);
  command('pwsh', ['-NoProfile', '-NonInteractive', '-File', path.join(repo, 'tools/portable/private-output.ps1'), '-Directory', report], repo);
  const packageRoot = freshDirectory(report, 'package');
  const acp = freshDirectory(packageRoot, product);
  const bin = freshDirectory(acp, 'bin'), runtime = freshDirectory(acp, 'runtime'), notices = freshDirectory(acp, 'notices');
  const targetDir = path.join(report, 'build-target');

  // Builder-private prefixes are remapped in compiler-recorded paths. Later
  // remaps win, so the nested private report directory comes after the checkout.
  const remaps = [[repo, '/acp/source'], [cargoHome(), '/cargo'], [report, '/acp/build']];
  const rustflags = remaps.map(([from, to]) => `--remap-path-prefix=${from}=${to}`);
  const env = cargoEnvironment(process.env, rustflags);
  const manifest = { format: 2, layout: 'ACP/{bin,runtime,notices}', prototype: true, public_distribution: false,
    source_sha: sha, source_tree: tree, target, profile: 'release', nsh_revision: pin,
    builder: { os_build: os.release(), node: process.version,
      vc_tools: vcTools,
      windows_sdk: (process.env.WindowsSDKVersion ?? 'unknown').replace(/\\$/, ''),
      cargo: command(cargoProgram, ['--version'], repo, null, env), rustc: command('rustc', ['-Vv'], repo, null, env) },
    rustflags: remaps.map(([, to], i) => `--remap-path-prefix=<${['checkout', 'cargo-home', 'build-output'][i]}>=${to}`),
    license: 'Apache-2.0', locks: {}, build_commands: [], binaries: [], prerequisites: [], files: [],
    limitations: ['Unsigned product executables; not a release or reproducible-binary claim',
      'OS minimum and clean-target acceptance unverified',
      'Third-party notice review pending; the ledger is not a linker-derived SBOM'] };
  for (const file of ['Cargo.lock', 'apps/operator/Cargo.lock']) manifest.locks[file] = digest(fs.readFileSync(path.join(repo, file)));
  requireValue(fs.readFileSync(path.join(repo, 'apps/operator/ui/Cargo.toml'), 'utf8').includes(pin), 'NSH pin mismatch');

  const workspaces = [
    ['core', 'Cargo.toml', ['acp-daemon', 'acp-query', 'acp-mcp'], []],
    ['operator', 'apps/operator/Cargo.toml', ['acp-operator-ui'], ['--features', 'native,tui']]];
  // A new target directory for each attempt prevents packaging stale build products.
  for (const [label, manifestFile, packages, features] of workspaces) {
    const args = ['build', '--manifest-path', manifestFile, '--release', '--locked', '--target', target,
      '--target-dir', targetDir, '--message-format=json', ...packages.flatMap(p => ['-p', p]), '--bins', ...features];
    manifest.build_commands.push({ workspace: label, args: args.map(a => a === targetDir ? '<owned-build-target>' : a) });
    const output = command(cargoProgram, args, repo, path.join(report, label + '-build.log'), env);
    const artifacts = output.split(/\r?\n/).filter(Boolean).map(line => JSON.parse(line))
      .filter(item => item.reason === 'compiler-artifact' && item.executable && item.target.kind.includes('bin'));
    for (const artifact of artifacts) {
      const file = path.basename(artifact.executable);
      if (!binaries.includes(file)) continue;
      // Cargo hard-links its own final output to deps/ inside this new private target.
      // Never ship that link: read the completed artifact and create a distinct archive file.
      const input = fs.lstatSync(artifact.executable);
      requireValue(input.isFile() && !input.isSymbolicLink(), 'Cargo output is not a regular executable');
      requireValue(path.resolve(artifact.executable).startsWith(path.resolve(targetDir) + path.sep), 'Unexpected Cargo output location');
      const bytes = fs.readFileSync(artifact.executable);
      fs.writeFileSync(path.join(bin, file), bytes, { flag: 'wx' });
      regular(path.join(bin, file));
      // Release binaries must not carry builder-private paths (checkout, home, Cargo home, output).
      const leaked = builderPathScan(bytes, { checkout: repo, user_home: os.homedir(), cargo_home: cargoHome(), build_output: report });
      requireValue(!Object.values(leaked).some(Boolean), `Builder path bytes in ${file}`);
      manifest.binaries.push({ file: `bin/${file}`, sha256: digest(bytes), size: bytes.length, ...peImports(bytes), builder_path_bytes: false });
    }
  }
  requireValue(JSON.stringify(manifest.binaries.map(b => path.basename(b.file)).sort()) === JSON.stringify([...binaries].sort()), 'Shipping binary set mismatch');

  // VC++ runtime is a user prerequisite: record exactly what the imports need.
  const vcImports = vcRuntimeImports(manifest.binaries);
  requireValue(vcImports.length > 0, 'Expected VC++ runtime imports');
  manifest.prerequisites.push({ component: 'Microsoft Visual C++ v14 Redistributable (x64)', minimum_version: vcTools,
    reason: 'installed version must be the same or later than the MSVC build tools used to build these executables',
    imports: vcImports.map(name => ({ name, importers: manifest.binaries.filter(b => b.imports.some(i => i.name === name)).map(b => b.file) })),
    download: 'https://aka.ms/vc14/vc_redist.x64.exe', bundled: false });

  // Notices: crate-local texts for the linked normal-dependency closure.
  const ledger = { scope: 'normal-dependency closure of shipped packages for target/features; NOT a linker-derived SBOM', packages: [] };
  for (const [label, manifestFile, packages, features] of workspaces) {
    const metadata = JSON.parse(command(cargoProgram, ['metadata', '--format-version', '1', '--locked', '--filter-platform', target,
      '--manifest-path', manifestFile, ...features], repo, path.join(report, label + '-metadata.log'), env));
    const closure = normalClosure(metadata, packages);
    for (const [linkage, list] of [['linked', closure.linked], ['compile-time-proc-macro', closure.compileTime]]) {
      for (const pkg of list) {
        const texts = [];
        const directory = path.dirname(pkg.manifest_path);
        for (const filename of fs.readdirSync(directory).filter(n => /^(licen[sc]e|copying|notice)([.-]|$)/i.test(n)).sort()) {
          const source = path.join(directory, filename), info = fs.lstatSync(source);
          if (!info.isFile() || info.isSymbolicLink()) continue;
          const bytes = fs.readFileSync(source), hash = digest(bytes), destination = `${hash}.txt`;
          if (!fs.existsSync(path.join(notices, destination))) fs.writeFileSync(path.join(notices, destination), bytes, { flag: 'wx' });
          texts.push({ file: `notices/${destination}`, sha256: hash });
        }
        ledger.packages.push({ workspace: label, package: pkg.name, version: pkg.version, linkage,
          source: pkg.source ? pkg.source.replace(/#.*$/, '') : 'first-party', declared_license: pkg.license, texts,
          review: !pkg.source ? 'FIRST-PARTY: Apache-2.0, see LICENSE.txt and notices/NOTICE.txt'
            : texts.length ? 'PENDING' : 'PENDING: no crate-local notice text found' });
      }
    }
  }
  fs.writeFileSync(path.join(notices, 'dependency-notices.json'), JSON.stringify(ledger, null, 2) + '\n', { flag: 'wx' });
  fs.writeFileSync(path.join(notices, 'NOTICE.txt'), notice, { flag: 'wx' });
  fs.copyFileSync(path.join(repo, 'tools/portable/README-portable.txt'), path.join(acp, 'README.txt'), fs.constants.COPYFILE_EXCL);
  fs.writeFileSync(path.join(acp, 'LICENSE.txt'), license, { flag: 'wx' });
  manifest.notices = { linked_packages: ledger.packages.filter(p => p.linkage === 'linked').length,
    compile_time_packages: ledger.packages.filter(p => p.linkage !== 'linked').length,
    third_party_without_text: ledger.packages.filter(p => p.source !== 'first-party' && !p.texts.length).length };
  manifest.files = inventory(acp);
  fs.writeFileSync(path.join(runtime, 'manifest.json'), JSON.stringify(manifest, null, 2) + '\n', { flag: 'wx' });

  // No private source paths, credentials or raw compiler diagnostics in archive text.
  for (const file of ['runtime/manifest.json', 'notices/dependency-notices.json', 'notices/NOTICE.txt', 'README.txt', 'LICENSE.txt']) {
    const scan = builderPathScan(fs.readFileSync(path.join(acp, file)), { checkout: repo, user_home: os.homedir(), report });
    requireValue(!Object.values(scan).some(Boolean), 'Private build path in archive metadata');
  }
  requireValue(command('git', ['rev-parse', 'HEAD'], repo) === sha && !command('git', ['status', '--porcelain'], repo), 'Source changed during build');
  fs.writeFileSync(path.join(report, 'archive-inventory.json'), JSON.stringify(inventory(packageRoot), null, 2) + '\n', { flag: 'wx' });
  return { report, packageRoot, sha, tree };
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try {
    if (process.argv[2] === '--verify' || process.argv[2] === '--remove-owned-product') {
      requireValue(process.argv.length === 5, 'Inventory command arguments required');
      const files = JSON.parse(fs.readFileSync(process.argv[4], 'utf8'));
      if (process.argv[2] === '--verify') { verifyInventory(process.argv[3], files); console.log('PASS: extracted inventory matches'); }
      else console.log(JSON.stringify({ result: 'PASS', ...removeOwnedProduct(process.argv[3], files) }));
      process.exit(0);
    }
    requireValue(process.argv.length === 6, 'Usage: node windows-package.mjs REPO EXISTING_PARENT NEW_NAME CARGO');
    const result = build(...process.argv.slice(2));
    console.log(JSON.stringify({ source_sha: result.sha, source_tree: result.tree, status: 'ASSEMBLED_NOT_ZIPPED' }));
  } catch (error) {
    const detail = error instanceof PolicyError ? error.message : (error.code ?? error.name);
    console.error(`PACKAGING_REFUSED: ${detail}; no install, cleanup or source fallback performed`); process.exitCode = 1;
  }
}
