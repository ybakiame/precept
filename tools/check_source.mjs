// Repository maintenance only. Requires the locked Node test dependencies.
import { execFileSync } from 'node:child_process';
import { existsSync, readFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { parseDocument } from '../spikes/p1-e/tests/node_modules/yaml/dist/index.js';
const root = fileURLToPath(new URL('../', import.meta.url));
process.chdir(root);
const listed = execFileSync('git', ['ls-files', '--cached', '--others', '--exclude-standard', '-z'], { encoding:'utf8' }).split('\0').filter(Boolean);
const files = [...new Set(listed)].filter(file => existsSync(file));
const failures = [];
const requireCheck = (condition, message) => { if (!condition) failures.push(message); };
const historicalNames = new Set(['docs/adr/0002-local-ipc-capabilities.md', 'docs/adr/0005-local-storage-and-write-ownership.md', 'docs/adr/0006-rust-only-runtime.md']);
const archive = new Set(JSON.parse(readFileSync('docs/archive-index.json')).documents);
for (const file of archive) {
  requireCheck(files.includes(file), `Missing archival document: ${file}`);
  requireCheck(readFileSync(file, 'utf8').includes('Historical record — not a current runbook.'), `Unmarked archive: ${file}`);
}
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
  // Frozen deletion inventory is explanatory reference text, not a runbook. The
  // scanner's own deny patterns likewise do not execute the banned commands.
  if (archive.has(file) || file === 'docs/p1/python-retirement.md' || file === 'tools/check_source.mjs') continue;
  requireCheck(!/\bacpctl\b/.test(text) || historicalNames.has(file), `Unexplained retired entrypoint name: ${file}`);
  const obsolete = /\bPYTHONPATH\b|\bACP_TEST_PYTHON\b|agent_control_plane|setup-python|tools\/[\w-]+\.py|\bpython(?:3(?:\.\d+)?)?\s+(?:-m|-c|tools\/)|\bpip3?\s+install/;
  requireCheck(!obsolete.test(text), `Unexplained active prototype reference: ${file}`);
  // Mentioning retired schema names in refusal fixtures or historical prose is
  // intentional. Only executable obsolete target/example commands are rejected.
  requireCheck(!/(?:-p\s+acp-rehearsal|--example\s+(?:synthetic_rehearsal|contract_reader|sqlite_reader))/.test(text), `Removed Cargo target invocation: ${file}`);
  for (const match of text.matchAll(/--manifest-path[ '\"]+([\w./-]+Cargo\.toml)/g)) {
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
        for (const match of (step.run || '').matchAll(/\bnode\s+(?:--test\s+)?((?:tools|spikes)\/[\w./-]+\.mjs)/g)) {
          requireCheck(existsSync(match[1]), `Missing workflow script: ${file} -> ${match[1]}`);
        }
      }
    }
  }
}
if (failures.length) { console.error(failures.join('\n')); process.exitCode = 1; }
else console.log(`PASS: ${files.length} source paths; zero tracked prototype executables/packaging; ${archive.size} explicit archives; active refs, local Markdown links and workflow YAML/targets checked`);
