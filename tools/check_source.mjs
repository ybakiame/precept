// Repository maintenance only. Requires the locked Node dependencies in tools/
// (`npm ci --prefix tools`).
import { execFileSync } from 'node:child_process';
import { existsSync, readFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { parseDocument } from './node_modules/yaml/dist/index.js';
const root = fileURLToPath(new URL('../', import.meta.url));
process.chdir(root);
const listed = execFileSync('git', ['ls-files', '--cached', '--others', '--exclude-standard', '-z'], { encoding:'utf8' }).split('\0').filter(Boolean);
const files = [...new Set(listed)].filter(file => existsSync(file));
const failures = [];
const requireCheck = (condition, message) => { if (!condition) failures.push(message); };
for (const file of files) {
  requireCheck(!file.endsWith('.py'), `Tracked executable prototype: ${file}`);
  requireCheck(!['pyproject.toml', 'setup.py', 'setup.cfg', 'requirements.txt'].includes(file), `Retired packaging: ${file}`);
  if (!/\.(md|rs|mjs|ps1|sh|toml|json|ya?ml)$/.test(file)) continue;
  const text = readFileSync(file, 'utf8');
  if (file.endsWith('.md')) {
    for (const match of text.matchAll(/\[[^\]]*\]\(([^)]+)\)/g)) {
      const target = match[1].split('#')[0];
      if (!target || /^[a-z]+:/.test(target)) continue;
      requireCheck(existsSync(path.resolve(path.dirname(file), target)), `Broken Markdown target: ${file} -> ${target}`);
    }
  }
  // The scanner's own deny patterns do not execute the banned commands.
  if (file === 'tools/check_source.mjs') continue;
  requireCheck(!/\bacpctl\b/.test(text), `Unexplained retired entrypoint name: ${file}`);
  const obsolete = /\bPYTHONPATH\b|\bACP_TEST_PYTHON\b|agent_control_plane|setup-python|tools\/[\w-]+\.py|\bpython(?:3(?:\.\d+)?)?\s+(?:-m|-c|tools\/)|\bpip3?\s+install/;
  requireCheck(!obsolete.test(text), `Unexplained active prototype reference: ${file}`);
  // Mentioning retired schema names in refusal fixtures or historical prose is
  // intentional. Only executable obsolete target/example commands are rejected.
  requireCheck(!/(?:-p\s+precept-rehearsal|--example\s+(?:synthetic_rehearsal|contract_reader|sqlite_reader))/.test(text), `Removed Cargo target invocation: ${file}`);
  for (const match of text.matchAll(/--manifest-path[ '"]+([\w./-]+Cargo\.toml)/g)) {
    requireCheck(existsSync(match[1]), `Missing Cargo manifest: ${file} -> ${match[1]}`);
  }
  if (file.startsWith('.github/workflows/')) {
    const parsed = parseDocument(text, { uniqueKeys:true });
    requireCheck(parsed.errors.length === 0, `Invalid workflow YAML: ${file}`);
    const workflow = parsed.toJS();
    requireCheck(workflow?.on && workflow?.jobs, `Missing workflow jobs/triggers: ${file}`);
    for (const job of Object.values(workflow?.jobs || {})) {
      requireCheck(Array.isArray(job.steps), `Missing workflow steps: ${file}`);
      for (const step of job.steps || []) {
        for (const match of (step.run || '').matchAll(/\bnode\s+(?:--test\s+)?((?:tools|spikes|apps)\/[\w./-]+\.mjs)/g)) {
          requireCheck(existsSync(match[1]), `Missing workflow script: ${file} -> ${match[1]}`);
        }
      }
    }
  }
}
// Cargo's core dependency is the authority; compare every active consumer.
// This validates pins, never resolves them.
// One product version: both Cargo workspaces, every crate and the app manifest.
const workspaceVersion = manifest => /\[workspace\.package\][^[]*?\nversion = "([^"]+)"/.exec(readFileSync(manifest, 'utf8'))?.[1];
const productVersion = workspaceVersion('Cargo.toml');
requireCheck(/^\d+\.\d+\.\d+$/.test(productVersion || ''), 'Cargo.toml [workspace.package] version is semantic');
requireCheck(workspaceVersion('apps/Cargo.toml') === productVersion, 'apps/Cargo.toml version matches Cargo.toml');
requireCheck(JSON.parse(readFileSync('apps/ui/nsh.json', 'utf8')).version === productVersion, 'apps/ui/nsh.json version matches Cargo.toml');
for (const manifest of files.filter(file => /^(crates|apps\/(core|client|ui))\/[^/]*\/?Cargo\.toml$|^apps\/(core|client|ui)\/Cargo\.toml$/.test(file))) {
  requireCheck(/^version\.workspace = true$/m.test(readFileSync(manifest, 'utf8')), `${manifest} takes the workspace version`);
}
const nshDependencies = [...readFileSync('apps/ui/Cargo.toml', 'utf8')
  .matchAll(/^(nsh-[\w-]+)\s*=\s*\{([^\n]+)\}\s*$/gm)]
  .map(([, name, fields]) => ({ name, body:fields, revision:fields.match(/\brev\s*=\s*"([0-9a-f]{40})"/)?.[1] }));
const nshPin = nshDependencies.find(dependency => dependency.name === 'nsh-core')?.revision;
requireCheck(!!nshPin, 'Missing immutable NSH Cargo core pin');
for (const dependency of nshDependencies) {
  requireCheck(!!nshPin && dependency.revision === nshPin, `NSH Cargo dependency pin mismatch: ${dependency.name}`);
}
// NSH 0.6.0 gates SVG rendering behind the native platform crates' `svg`
// feature; without it every icon silently draws nothing (test builds hide this:
// dev-dependencies unify the feature in).
for (const platform of ['nsh-platform-linux', 'nsh-platform-macos', 'nsh-platform-windows']) {
  const dependency = nshDependencies.find(d => d.name === platform);
  requireCheck(!!dependency && /features\s*=\s*\[[^\]]*"svg"/.test(dependency.body),
    `NSH native platform without the svg feature (icons would not draw): ${platform}`);
}
const portablePin = readFileSync('tools/portable/windows-package.mjs', 'utf8')
  .match(/^export const pin = '([0-9a-f]{40})';$/m)?.[1];
requireCheck(!!nshPin && portablePin === nshPin, 'NSH portable packager pin differs from Cargo');
if (failures.length) { console.error(failures.join('\n')); process.exitCode = 1; }
else console.log(`PASS: ${files.length} source paths; zero tracked prototype executables/packaging; active refs, local Markdown links, workflow YAML/targets and immutable NSH consumer pins checked`);
