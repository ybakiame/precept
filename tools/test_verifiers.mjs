// Real Linux helper control-flow tests in an owned synthetic Git repository.
// Cargo is a fixture stub here; this is NOT product/runtime acceptance.
import { mkdtempSync, mkdirSync, cpSync, writeFileSync, readFileSync, existsSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { spawnSync, execFileSync } from 'node:child_process';
import assert from 'node:assert/strict';
const root = mkdtempSync(path.join(tmpdir(), 'acp-s5-verify-'));
const repo = path.join(root, 'repo'), bin = path.join(root, 'bin'), trace = path.join(root, 'cargo-trace');
mkdirSync(repo); mkdirSync(bin); mkdirSync(path.join(repo,'tools'));
cpSync('tools/local_verify.sh', path.join(repo,'tools/local_verify.sh'));
writeFileSync(path.join(bin,'cargo'), '#!/bin/sh\nprintf "%s\\n" "$*" >> "$ACP_S5_TRACE"\nexit 0\n', { mode:0o700 });
const env = { ...process.env, GIT_CONFIG_NOSYSTEM:'1', GIT_CONFIG_GLOBAL:path.join(root,'empty'), PATH:bin+path.delimiter+process.env.PATH, ACP_S5_TRACE:trace };
writeFileSync(env.GIT_CONFIG_GLOBAL,'');
const git = (...args) => execFileSync('git', args, { cwd:repo, env, encoding:'utf8' }).trim();
const cases=[];
try {
  git('init','-q'); git('config','user.name','Synthetic'); git('config','user.email','synthetic@example.invalid');
  writeFileSync(path.join(repo,'sample'),'base\n');git('add','.');git('commit','-qm','base');const base=git('rev-parse','HEAD');
  writeFileSync(path.join(repo,'sample'),'bad \n');git('commit','-qam','committed whitespace defect');
  const run=(name,baseline,pass=false,existing=false) => {
    rmSync(trace,{force:true}); const report=path.join(root,name);if(existing)mkdirSync(report);
    const args=['tools/local_verify.sh','core',report];if(baseline!==null)args.push(baseline);
    const result=spawnSync('bash',args,{cwd:repo,env,encoding:'utf8'});
    assert.equal(result.status===0,pass,`${name}: unexpected exit ${result.status}`);
    assert.equal(existsSync(trace),pass,`${name}: Cargo before valid branch diff`);
    if(pass)assert(readFileSync(path.join(report,'commands.txt'),'utf8').includes(`git diff --check ${base} HEAD`));
    if(name==='committed-whitespace')assert(readFileSync(path.join(report,'result.tsv'),'utf8').includes('branch-diff\tFAIL\t2'));
    cases.push({name,status:'PASS',helper_exit:result.status});
  };
  run('committed-whitespace',base);
  writeFileSync(path.join(repo,'sample'),'fixed\n');git('commit','-qam','fixed');
  run('fixed-branch',base,true);
  const fixed=git('rev-parse','HEAD');git('commit','--allow-empty','-qm','empty delta');run('empty-delta',fixed);
  run('missing-base',null);run('invalid-base','f'.repeat(40));run('head-equal',git('rev-parse','HEAD'));
  git('branch','ambiguous');git('tag','ambiguous');run('ambiguous-ref','ambiguous');
  const orphan=execFileSync('git',['commit-tree',git('rev-parse','HEAD^{tree}')],{cwd:repo,env,input:'unrelated\n',encoding:'utf8'}).trim();
  run('nonancestor-base',orphan);run('existing-report',base,false,true);
  writeFileSync(path.join(repo,'sample'),'dirty\n');run('dirty-source',base);
  console.log(JSON.stringify({status:'PASS',platform:process.platform,proof:'actual helper and Git; Cargo stub for control-flow only',cases},null,2));
} finally { rmSync(root,{recursive:true,force:true}); }
