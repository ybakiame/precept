// Developer-only Linux x86_64 portable recipe (P1-M5/S1-C). No install, activation or publish.
//
// Archive: one top-level ACP/ directory inside a .tar.gz
//   ACP/bin/                    six product executables (0755)
//   ACP/runtime/manifest.json   provenance, ELF facts, declared runtime packages, file hashes
//   ACP/notices/                NOTICE.txt, dependency ledger, license texts
//   ACP/README.txt, ACP/LICENSE.txt (Apache-2.0)
// OS-owned libraries (glibc, loader, GTK, GStreamer, Mesa, fonts, D-Bus) are never
// bundled: they are declared user-distro runtime dependencies.
import fs from 'node:fs';
import path from 'node:path';
import os from 'node:os';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { apacheLicenseSha256, builderPathScan, cargoEnvironment, digest, freshDirectory, inventory, normalClosure,
  pin, PolicyError, regular, removeOwnedProduct, verifyInventory } from './windows-package.mjs';
import { elfInfo, minimumVersions } from './elf.mjs';

export const binaries = ['acp-daemon', 'acp-query', 'acp-mcp', 'acp-nsh-native', 'acp-nsh-tui', 'acp-wsl-bridge'];
export const target = 'x86_64-unknown-linux-gnu';
export const interpreter = '/lib64/ld-linux-x86-64.so.2';
const need = (ok, message) => { if (!ok) throw new PolicyError(message); };
export const removable = /^ACP\/(bin\/acp-(daemon|query|mcp|nsh-native|nsh-tui|wsl-bridge)|runtime\/manifest\.json|notices\/(NOTICE\.txt|dependency-notices\.json|[a-f0-9]{64}\.txt)|README\.txt|LICENSE\.txt)$/;

/// Every shipped ELF must use the system loader and carry no RPATH/RUNPATH:
/// libraries always come from the user distribution, never from the archive.
export function checkElf(name, info) {
  need(info.interpreter === interpreter, `${name}: unexpected ELF interpreter`);
  need(info.rpath.length === 0, `${name}: RPATH/RUNPATH refused`);
  need(info.needed.length > 0 && info.needed.includes('libc.so.6'), `${name}: expected dynamic glibc linkage`);
  return info;
}

/// `tar -tvz` listing lines: only ACP/ paths, no traversal, absolute paths, links or
/// devices; executables 0755 under ACP/bin, everything else 0644 files / 0755 dirs.
export function checkTarListing(lines) {
  const entries = [];
  for (const line of lines.filter(Boolean)) {
    const m = /^([-d])([rwx-]{9})\s+\S+\s+\d+\s+\S+\s+\S+\s+(\S+)$/.exec(line);
    need(m, 'Unexpected archive entry type or listing');
    const [, kind, perms, name] = m;
    need(/^ACP(\/[A-Za-z0-9][A-Za-z0-9._-]*)*\/?$/.test(name) && !name.split('/').includes('..'), 'Unsafe archive path');
    const file = name.replace(/\/$/, '');
    const expected = kind === 'd' || file.startsWith('ACP/bin/') ? 'rwxr-xr-x' : 'rw-r--r--';
    need(perms === expected, `Unexpected mode for ${file}`);
    entries.push({ file, directory: kind === 'd' });
  }
  return entries;
}

function run(program, args, options = {}) {
  const r = spawnSync(program, args, { encoding: 'utf8', maxBuffer: 256 * 1024 * 1024, ...options });
  need(!r.error && r.status === 0, `Developer command failed: ${path.basename(program)} status=${r.status}`);
  return r.stdout.trim();
}
function cargoHome() { return process.env.CARGO_HOME || path.join(os.homedir(), '.cargo'); }

/// Direct DT_NEEDED name -> resolved path on the builder (ldd) -> owning Debian package.
function packagesFor(file, needed) {
  const resolved = {};
  for (const line of run('ldd', [file]).split('\n')) {
    // "libX.so.N => /path (0x..)", or the loader itself as "/lib64/ld-linux-x86-64.so.2 (0x..)".
    const arrow = /^\s*(\S+) => (\/\S+) \(0x/.exec(line);
    const bare = /^\s*(\/\S+) \(0x/.exec(line);
    if (arrow) resolved[arrow[1]] = arrow[2];
    else if (bare) resolved[path.basename(bare[1])] = bare[1];
  }
  return needed.map(lib => {
    const where = resolved[lib];
    need(where, `${lib} not resolvable on the builder`);
    const real = fs.realpathSync(where);
    const owner = [real, where].map(p => spawnSync('dpkg', ['-S', p], { encoding: 'utf8' }))
      .find(r => r.status === 0)?.stdout.split(':')[0].trim();
    need(owner, `${lib}: no owning package`);
    return { library: lib, package: owner };
  });
}

export function build(repo, parent, name, cargoProgram) {
  need(process.platform === 'linux' && process.arch === 'x64', 'Native Linux x86_64 builder required');
  need(!run('git', ['status', '--porcelain', '--untracked-files=all'], { cwd: repo }), 'Clean committed source required');
  const sha = run('git', ['rev-parse', 'HEAD'], { cwd: repo }), tree = run('git', ['rev-parse', 'HEAD^{tree}'], { cwd: repo });
  const commitTime = run('git', ['show', '-s', '--format=%ct', 'HEAD'], { cwd: repo });
  const license = fs.readFileSync(path.join(repo, 'LICENSE'));
  need(digest(license) === apacheLicenseSha256, 'LICENSE is not the verbatim Apache-2.0 text');
  const notice = fs.readFileSync(path.join(repo, 'NOTICE'));
  need(fs.readFileSync(path.join(repo, 'apps/operator/ui/Cargo.toml'), 'utf8').includes(pin), 'NSH pin mismatch');
  const report = freshDirectory(parent, name);
  fs.chmodSync(report, 0o700);
  const packageRoot = freshDirectory(report, 'package');
  const acp = freshDirectory(packageRoot, 'ACP');
  const bin = freshDirectory(acp, 'bin'), runtime = freshDirectory(acp, 'runtime'), notices = freshDirectory(acp, 'notices');
  const targetDir = path.join(report, 'build-target');
  const remaps = [[repo, '/acp/source'], [cargoHome(), '/cargo'], [report, '/acp/build']];
  const env = cargoEnvironment(process.env, remaps.map(([from, to]) => `--remap-path-prefix=${from}=${to}`));
  const osRelease = Object.fromEntries(fs.readFileSync('/etc/os-release', 'utf8').split('\n')
    .map(l => l.split('=')).filter(p => p.length === 2).map(([k, v]) => [k, v.replace(/^"|"$/g, '')]));
  const manifest = { format: 1, platform: 'linux', layout: 'ACP/{bin,runtime,notices}', prototype: true, public_distribution: false,
    source_sha: sha, source_tree: tree, target, profile: 'release', nsh_revision: pin, license: 'Apache-2.0',
    builder: { distribution: `${osRelease.NAME} ${osRelease.VERSION}`, kernel: os.release(), node: process.version,
      glibc: run('ldd', ['--version']).split('\n')[0], cargo: run(cargoProgram, ['--version'], { env }),
      rustc: run('rustc', ['-Vv'], { env }) },
    rustflags: ['checkout', 'cargo-home', 'build-output'].map((k, i) => `--remap-path-prefix=<${k}>=${remaps[i][1]}`),
    locks: {}, build_commands: [], binaries: [], runtime: {}, files: [],
    limitations: ['Unsigned; not a release or reproducible-binary claim',
      'Validated only on the recorded Ubuntu 24.04 WSL2 + WSLg target; no standalone Linux or cross-distro claim',
      'Third-party notice review pending; the ledger is not a linker-derived SBOM'] };
  for (const file of ['Cargo.lock', 'apps/operator/Cargo.lock']) manifest.locks[file] = digest(fs.readFileSync(path.join(repo, file)));
  const workspaces = [
    ['core', 'Cargo.toml', ['acp-daemon', 'acp-query', 'acp-mcp'], []],
    ['operator', 'apps/operator/Cargo.toml', ['acp-operator-ui', 'acp-operator-model'],
      ['--features', 'acp-operator-ui/native,acp-operator-ui/tui,acp-operator-model/native']]];
  for (const [label, manifestFile, packages, features] of workspaces) {
    const args = ['build', '--manifest-path', manifestFile, '--release', '--locked', '--target', target,
      '--target-dir', targetDir, '--message-format=json', ...packages.flatMap(p => ['-p', p]), '--bins', ...features];
    manifest.build_commands.push({ workspace: label, args: args.map(a => a === targetDir ? '<owned-build-target>' : a) });
    const r = spawnSync(cargoProgram, args, { cwd: repo, env, encoding: 'utf8', maxBuffer: 256 * 1024 * 1024 });
    fs.writeFileSync(path.join(report, `${label}-build.log`), (r.stdout ?? '') + (r.stderr ?? ''), { flag: 'wx' });
    need(!r.error && r.status === 0, `Cargo build failed (${label}); inspect the private build log`);
    for (const artifact of r.stdout.split('\n').filter(Boolean).map(l => JSON.parse(l))
      .filter(i => i.reason === 'compiler-artifact' && i.executable && i.target.kind.includes('bin'))) {
      const file = path.basename(artifact.executable);
      if (!binaries.includes(file)) continue;
      need(path.resolve(artifact.executable).startsWith(path.resolve(targetDir) + path.sep), 'Unexpected Cargo output location');
      const bytes = fs.readFileSync(artifact.executable);
      const leaked = builderPathScan(bytes, { checkout: repo, user_home: os.homedir(), cargo_home: cargoHome(), build_output: report });
      need(!Object.values(leaked).some(Boolean), `Builder path bytes in ${file}`);
      const out = path.join(bin, file);
      fs.writeFileSync(out, bytes, { flag: 'wx', mode: 0o755 });
      fs.chmodSync(out, 0o755);
      regular(out);
      const elf = checkElf(file, elfInfo(bytes));
      manifest.binaries.push({ file: `bin/${file}`, sha256: digest(bytes), size: bytes.length, interpreter: elf.interpreter,
        needed: elf.needed, versions: minimumVersions([elf]), packages: packagesFor(out, elf.needed), builder_path_bytes: false });
    }
  }
  need(JSON.stringify(manifest.binaries.map(b => path.basename(b.file)).sort()) === JSON.stringify([...binaries].sort()), 'Shipping binary set mismatch');
  // Declared runtime dependency: owning packages of direct DT_NEEDED libraries.
  const byPackage = {};
  for (const b of manifest.binaries) for (const p of b.packages) (byPackage[p.package] ??= new Set()).add(b.file);
  manifest.runtime = {
    minimum_symbol_versions: minimumVersions(manifest.binaries.map(b => ({ versions: Object.fromEntries(Object.entries(b.versions).map(([k, v]) => [k, [v]])) }))),
    packages: Object.keys(byPackage).sort().map(name => ({ name, builder_version: run('dpkg-query', ['-W', '-f=${Version}', name]), needed_by: [...byPackage[name]].sort() })),
    note: 'Direct DT_NEEDED owners on the builder. Dynamic GTK/GIO/GStreamer modules, fonts and GPU/display integration are observed separately at runtime.'
  };
  // Notices for the shipped packages' normal-dependency closure.
  const ledger = { scope: 'normal-dependency closure of shipped packages for target/features; NOT a linker-derived SBOM', packages: [] };
  for (const [label, manifestFile, packages, features] of workspaces) {
    const metadata = JSON.parse(run(cargoProgram, ['metadata', '--format-version', '1', '--locked', '--filter-platform', target,
      '--manifest-path', manifestFile, ...features], { cwd: repo, env }));
    const closure = normalClosure(metadata, packages);
    for (const [linkage, list] of [['linked', closure.linked], ['compile-time-proc-macro', closure.compileTime]]) {
      for (const pkg of list) {
        const texts = [];
        const directory = path.dirname(pkg.manifest_path);
        for (const filename of fs.readdirSync(directory).filter(n => /^(licen[sc]e|copying|notice)([.-]|$)/i.test(n)).sort()) {
          const source = path.join(directory, filename), info = fs.lstatSync(source);
          if (!info.isFile() || info.isSymbolicLink()) continue;
          const bytes = fs.readFileSync(source), hash = digest(bytes), destination = `${hash}.txt`;
          if (!fs.existsSync(path.join(notices, destination))) fs.writeFileSync(path.join(notices, destination), bytes, { flag: 'wx', mode: 0o644 });
          texts.push({ file: `notices/${destination}`, sha256: hash });
        }
        ledger.packages.push({ workspace: label, package: pkg.name, version: pkg.version, linkage,
          source: pkg.source ? pkg.source.replace(/#.*$/, '') : 'first-party', declared_license: pkg.license, texts,
          review: !pkg.source ? 'FIRST-PARTY: Apache-2.0, see LICENSE.txt and notices/NOTICE.txt'
            : texts.length ? 'PENDING' : 'PENDING: no crate-local notice text found' });
      }
    }
  }
  fs.writeFileSync(path.join(notices, 'dependency-notices.json'), JSON.stringify(ledger, null, 2) + '\n', { flag: 'wx', mode: 0o644 });
  fs.writeFileSync(path.join(notices, 'NOTICE.txt'), notice, { flag: 'wx', mode: 0o644 });
  const packageLine = manifest.runtime.packages.map(p => p.name).join(' ');
  const readme = fs.readFileSync(path.join(repo, 'tools/portable/README-linux.txt'), 'utf8')
    .replace('{{RUNTIME_PACKAGES}}', packageLine)
    .replace('{{GLIBC}}', manifest.runtime.minimum_symbol_versions.GLIBC ?? 'unknown');
  fs.writeFileSync(path.join(acp, 'README.txt'), readme, { flag: 'wx', mode: 0o644 });
  fs.writeFileSync(path.join(acp, 'LICENSE.txt'), license, { flag: 'wx', mode: 0o644 });
  manifest.notices = { linked_packages: ledger.packages.filter(p => p.linkage === 'linked').length,
    compile_time_packages: ledger.packages.filter(p => p.linkage !== 'linked').length,
    third_party_without_text: ledger.packages.filter(p => p.source !== 'first-party' && !p.texts.length).length };
  manifest.files = inventory(acp);
  fs.writeFileSync(path.join(runtime, 'manifest.json'), JSON.stringify(manifest, null, 2) + '\n', { flag: 'wx', mode: 0o644 });
  for (const file of ['runtime/manifest.json', 'notices/dependency-notices.json', 'notices/NOTICE.txt', 'README.txt', 'LICENSE.txt']) {
    const scan = builderPathScan(fs.readFileSync(path.join(acp, file)), { checkout: repo, user_home: os.homedir(), report });
    need(!Object.values(scan).some(Boolean), 'Private build path in archive metadata');
  }
  for (const dir of [acp, bin, runtime, notices]) fs.chmodSync(dir, 0o755);
  need(run('git', ['rev-parse', 'HEAD'], { cwd: repo }) === sha && !run('git', ['status', '--porcelain'], { cwd: repo }), 'Source changed during build');
  // tar.gz: sorted names, numeric root owner, commit time, no links (verified below).
  const archive = path.join(report, `acp-linux-x86_64-prototype-${sha.slice(0, 12)}.tar.gz`);
  run('tar', ['--sort=name', '--owner=0', '--group=0', '--numeric-owner', `--mtime=@${commitTime}`,
    '-C', packageRoot, '-czf', archive, 'ACP']);
  const entries = checkTarListing(run('tar', ['-tvzf', archive, '--numeric-owner']).split('\n'));
  const extracted = path.join(report, 'extracted');
  need(!fs.existsSync(extracted), 'Extraction collision refused');
  fs.mkdirSync(extracted, { mode: 0o700 });
  run('tar', ['-xzf', archive, '-C', extracted, '--no-same-owner']);
  const files = inventory(packageRoot);
  verifyInventory(extracted, files);
  for (const b of binaries) need((fs.statSync(path.join(extracted, 'ACP/bin', b)).mode & 0o777) === 0o755, `${b} not executable after extraction`);
  fs.writeFileSync(path.join(report, 'archive-inventory.json'), JSON.stringify(files, null, 2) + '\n', { flag: 'wx' });
  const result = { archive: path.basename(archive), archive_sha256: digest(fs.readFileSync(archive)),
    archive_bytes: fs.statSync(archive).size, archive_entries: entries.length, extract_inventory: 'PASS', source_sha: sha };
  fs.writeFileSync(path.join(report, 'package-result.json'), JSON.stringify(result, null, 2) + '\n', { flag: 'wx' });
  return result;
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try {
    if (process.argv[2] === '--verify' || process.argv[2] === '--remove-owned-product') {
      need(process.argv.length === 5, 'Inventory command arguments required');
      const files = JSON.parse(fs.readFileSync(process.argv[4], 'utf8'));
      if (process.argv[2] === '--verify') { verifyInventory(process.argv[3], files); console.log('PASS: extracted inventory matches'); }
      else console.log(JSON.stringify({ result: 'PASS', ...removeOwnedProduct(process.argv[3], files, removable, binaries) }));
      process.exit(0);
    }
    need(process.argv.length === 6, 'Usage: node linux-package.mjs REPO EXISTING_PARENT NEW_NAME CARGO');
    console.log(JSON.stringify(build(...process.argv.slice(2))));
  } catch (error) {
    const detail = error instanceof PolicyError || error?.name === 'ElfError' ? error.message : (error.code ?? error.name);
    console.error(`PACKAGING_REFUSED: ${detail}; no install, cleanup or source fallback performed`);
    process.exitCode = 1;
  }
}
