// Developer-only Windows portable prototype recipe. No install, activation or publish.
import fs from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

export const binaries = ['acp-daemon.exe', 'acp-query.exe', 'acp-mcp.exe', 'acp-nsh-native.exe', 'acp-nsh-tui.exe'];
export const pin = 'b7f31a08478be262fd7b76ec856036fd5b8ffdb0';
export const digest = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
function requireValue(value, message) { if (!value) throw Error(message); }
export function regular(file) {
  const info = fs.lstatSync(file);
  requireValue(info.isFile() && !info.isSymbolicLink() && info.nlink === 1, 'Expected unaliased regular file');
  return info;
}
export function freshDirectory(parent, leaf) {
  requireValue(/^[a-zA-Z0-9][a-zA-Z0-9._-]{0,95}$/.test(leaf) && !leaf.endsWith('.'), 'Unsafe output name');
  const absolute = path.resolve(parent);
  // Resolve every existing component; refuse junctions/symlinks rather than following them.
  for (let part = absolute;; part = path.dirname(part)) {
    requireValue(fs.lstatSync(part).isDirectory() && !fs.lstatSync(part).isSymbolicLink(), 'Unsafe output parent');
    if (part === path.dirname(part)) break;
  }
  const result = path.join(absolute, leaf);
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
    throw Error('Unmapped PE RVA');
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
      if (bytes.subarray(at, at + stride).every(x => x === 0)) { terminated = true; break; }
      if (delayed) requireValue(read32(at) === 1, 'Unsupported delay import address form');
      imports.push({ name: dllName(read32(at + nameOffset)), delayed });
    }
    requireValue(terminated, 'Unterminated PE import directory');
  }
  return { machine: 'AMD64', subsystem: read16(optional + 68), imports };
}
function command(program, args, cwd, logFile) {
  const result = spawnSync(program, args, { cwd, encoding: 'utf8', maxBuffer: 64 * 1024 * 1024, windowsHide: true });
  if (logFile) fs.writeFileSync(logFile, (result.stdout ?? '') + (result.stderr ?? ''), { flag: 'wx' });
  requireValue(!result.error && result.status === 0, 'Developer command failed; inspect retained private build log');
  return result.stdout.trim();
}
function inventory(root) {
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
function cargo(program, args, repo, report, label) {
  return command(program, args, repo, path.join(report, label + '.log'));
}
export function build(repo, parent, name, cargoProgram) {
  requireValue(process.platform === 'win32' && process.arch === 'x64', 'Native Windows x86_64 required');
  requireValue(!command('git', ['status', '--porcelain', '--untracked-files=all'], repo), 'Clean committed source required');
  const sha = command('git', ['rev-parse', 'HEAD'], repo), tree = command('git', ['rev-parse', 'HEAD^{tree}'], repo);
  requireValue(/^[a-f0-9]{40}$/.test(sha) && /^[a-f0-9]{40}$/.test(tree), 'Invalid source identity');
  const report = freshDirectory(parent, name), target = 'x86_64-pc-windows-msvc';
  command('pwsh', ['-NoProfile', '-NonInteractive', '-File', path.join(repo, 'tools/portable/private-output.ps1'), '-Directory', report], repo);
  const stage = freshDirectory(report, 'acp-windows-x86_64-prototype');
  const manifest = { format: 1, prototype: true, public_distribution: false, source_sha: sha, source_tree: tree,
    target, profile: 'release', nsh_revision: pin, builder: { os_build: process.getSystemVersion(), node: process.version,
      vc_tools: process.env.VCToolsVersion ?? 'unknown', windows_sdk: process.env.WindowsSDKVersion ?? 'unknown',
      cargo: command(cargoProgram, ['--version'], repo), rustc: command('rustc', ['-Vv'], repo) },
    locks: {}, build_commands: [], binaries: [], limitations: ['Unsigned; not a release or reproducible-binary claim',
      'OS minimum and clean-target acceptance unverified', 'First-party license grant and complete third-party notice review pending'] };
  for (const file of ['Cargo.lock', 'apps/operator/Cargo.lock']) {
    manifest.locks[file] = digest(fs.readFileSync(path.join(repo, file)));
  }
  requireValue(fs.readFileSync(path.join(repo, 'apps/operator/ui/Cargo.toml'), 'utf8').includes(pin), 'NSH pin mismatch');
  // A new target directory for each attempt prevents packaging stale build products.
  for (const [label, manifestFile, packages, features] of [
    ['core', 'Cargo.toml', ['acp-daemon', 'acp-query', 'acp-mcp'], []],
    ['operator', 'apps/operator/Cargo.toml', ['acp-operator-ui'], ['--features', 'native,tui']]]) {
    const targetDir = path.join(report, label + '-target');
    const args = ['build', '--manifest-path', manifestFile, '--release', '--locked', '--target', target,
      '--target-dir', targetDir, '--message-format=json', ...packages.flatMap(p => ['-p', p]), '--bins', ...features];
    manifest.build_commands.push({ workspace: label, args: args.map(a => a === targetDir ? '<owned-build-target>' : a) });
    const output = cargo(cargoProgram, args, repo, report, label + '-build');
    const artifacts = output.split(/\r?\n/).filter(Boolean).map(line => JSON.parse(line))
      .filter(item => item.reason === 'compiler-artifact' && item.executable && item.target.kind.includes('bin'));
    for (const artifact of artifacts) {
      const name = path.basename(artifact.executable);
      if (!binaries.includes(name)) continue;
      regular(artifact.executable);
      requireValue(path.resolve(artifact.executable).startsWith(path.resolve(targetDir) + path.sep), 'Unexpected Cargo output location');
      const bytes = fs.readFileSync(artifact.executable), pe = peImports(bytes);
      fs.copyFileSync(artifact.executable, path.join(stage, name), fs.constants.COPYFILE_EXCL);
      manifest.binaries.push({ file: name, sha256: digest(bytes), size: bytes.length, ...pe });
    }
  }
  requireValue(JSON.stringify(manifest.binaries.map(b => b.file).sort()) === JSON.stringify([...binaries].sort()), 'Shipping binary set mismatch');
  // License inventory is a Windows resolved dependency superset, not a linker-derived SBOM.
  const ledger = [];
  const notices = freshDirectory(stage, 'notices');
  for (const [label, manifestFile, features] of [['core', 'Cargo.toml', []], ['operator', 'apps/operator/Cargo.toml', ['--features', 'native,tui']]]) {
    const metadata = JSON.parse(cargo(cargoProgram, ['metadata', '--format-version', '1', '--locked', '--filter-platform', target,
      '--manifest-path', manifestFile, ...features], repo, report, label + '-metadata'));
    const resolved = new Set(metadata.resolve.nodes.map(n => n.id));
    for (const pkg of metadata.packages.filter(p => resolved.has(p.id))) {
      const texts = [];
      const directory = path.dirname(pkg.manifest_path);
      for (const filename of fs.readdirSync(directory).filter(n => /^(licen[sc]e|copying|notice)([.-]|$)/i.test(n)).sort()) {
        const source = path.join(directory, filename);
        if (!fs.lstatSync(source).isFile() || fs.lstatSync(source).isSymbolicLink()) continue;
        regular(source);
        const bytes = fs.readFileSync(source), hash = digest(bytes);
        const destination = `${hash}.txt`;
        if (!fs.existsSync(path.join(notices, destination))) fs.writeFileSync(path.join(notices, destination), bytes, { flag: 'wx' });
        texts.push({ file: `notices/${destination}`, sha256: hash });
      }
      ledger.push({ workspace: label, package: pkg.name, version: pkg.version, source: pkg.source ?? 'first-party',
        declared_license: pkg.license, texts, review: 'PENDING: resolved superset; includes build/dev dependencies; verify actual linked/shipped obligations' });
    }
  }
  fs.writeFileSync(path.join(stage, 'dependency-notices.json'), JSON.stringify(ledger, null, 2) + '\n', { flag: 'wx' });
  fs.copyFileSync(path.join(repo, 'tools/portable/README-portable.txt'), path.join(stage, 'README.txt'), fs.constants.COPYFILE_EXCL);
  fs.writeFileSync(path.join(stage, 'LICENSE-STATUS.txt'), 'LOCAL UNPUBLISHED PROTOTYPE ONLY. First-party license text/ownership grant and third-party distribution review are pending. Cargo metadata is not a license grant. Do not publish this archive.\n', { flag: 'wx' });
  fs.writeFileSync(path.join(stage, 'manifest.json'), JSON.stringify(manifest, null, 2) + '\n', { flag: 'wx' });
  const files = inventory(stage);
  fs.writeFileSync(path.join(report, 'inventory.json'), JSON.stringify(files, null, 2) + '\n', { flag: 'wx' });
  fs.writeFileSync(path.join(stage, 'SHA256SUMS.txt'), files.map(f => `${f.sha256}  ${f.file}`).join('\n') + '\n', { flag: 'wx' });
  // No private source paths, credentials or raw compiler diagnostics in archive metadata.
  const json = fs.readFileSync(path.join(stage, 'manifest.json'), 'utf8') + fs.readFileSync(path.join(stage, 'dependency-notices.json'), 'utf8');
  requireValue(!json.includes(repo) && !json.includes(repo.replaceAll('\\', '\\\\')), 'Private build path in archive metadata');
  requireValue(command('git', ['rev-parse', 'HEAD'], repo) === sha && !command('git', ['status', '--porcelain'], repo), 'Source changed during build');
  const allFiles = inventory(stage);
  fs.writeFileSync(path.join(report, 'archive-inventory.json'), JSON.stringify(allFiles, null, 2) + '\n', { flag: 'wx' });
  return { report, stage, sha, tree };
}
if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try {
    requireValue(process.argv.length === 6, 'Usage: node windows-package.mjs REPO EXISTING_PARENT NEW_NAME CARGO');
    const result = build(...process.argv.slice(2));
    console.log(JSON.stringify({ source_sha: result.sha, source_tree: result.tree, status: 'ASSEMBLED_NOT_ZIPPED' }));
  } catch { console.error('PACKAGING_REFUSED: inspect owned private logs; no install, cleanup or source fallback performed'); process.exitCode = 1; }
}
