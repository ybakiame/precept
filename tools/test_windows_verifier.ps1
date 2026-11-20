param([string]$SourceRoot = (Split-Path -Parent $PSScriptRoot))
$ErrorActionPreference = 'Stop'
$originalEnvironment = [Environment]::GetEnvironmentVariables('Process')
function Restore-Environment($Saved) {
    foreach ($key in @([Environment]::GetEnvironmentVariables('Process').Keys)) {
        if (-not $Saved.Contains($key)) { [Environment]::SetEnvironmentVariable($key, [NullString]::Value, 'Process') }
    }
    foreach ($key in $Saved.Keys) { [Environment]::SetEnvironmentVariable($key, $Saved[$key], 'Process') }
}
$root = Join-Path ([System.IO.Path]::GetTempPath()) ('precept-s5-verify-' + [guid]::NewGuid().ToString('N'))
$repo = Join-Path $root 'repo'
New-Item -ItemType Directory -Path (Join-Path $repo 'tools') | Out-Null
Copy-Item -LiteralPath (Join-Path $SourceRoot 'tools/windows_local_verify.ps1') -Destination (Join-Path $repo 'tools/windows_local_verify.ps1')
$env:GIT_CONFIG_NOSYSTEM = '1'
$env:GIT_CONFIG_GLOBAL = Join-Path $root 'empty.gitconfig'
Set-Content -LiteralPath $env:GIT_CONFIG_GLOBAL -Value ''
$env:PRECEPT_S5_TRACE = Join-Path $root 'cargo-trace'
function cargo { Add-Content -LiteralPath $env:PRECEPT_S5_TRACE -Value ($args -join ' '); $global:LASTEXITCODE = 0 }
$results = [System.Collections.Generic.List[object]]::new()
function Git-Run([string[]]$Arguments) {
    $result = & git @Arguments 2>$null
    if ($LASTEXITCODE -ne 0) { throw 'Synthetic Git operation failed' }
    return $result
}
function Check-Case([string]$Name, [string]$Base, [bool]$Pass = $false, [bool]$Existing = $false) {
    Remove-Item -LiteralPath $env:PRECEPT_S5_TRACE -ErrorAction SilentlyContinue
    $report = Join-Path $root $Name
    if ($Existing) { New-Item -ItemType Directory -Path $report | Out-Null }
    $ok = $true
    $environment = [Environment]::GetEnvironmentVariables('Process')
    try { & ./tools/windows_local_verify.ps1 -Suite Core -ReportDir $report -DiffBase $Base *> (Join-Path $root "$Name.log") }
    catch { $ok = $false; $failure = $_.Exception.Message }
    finally {
        $after = [Environment]::GetEnvironmentVariables('Process')
        $changed = @($after.Keys | Where-Object {
            -not $environment.Contains($_) -or $after[$_] -cne $environment[$_]
        }) + @($environment.Keys | Where-Object { -not $after.Contains($_) })
        Restore-Environment $environment
        if ($changed.Count) { throw "Helper did not restore caller process environment: $Name ($($changed -join ', '))" }
    }
    if ($ok -ne $Pass) { throw "Unexpected helper outcome: $Name ($failure)" }
    if ((Test-Path $env:PRECEPT_S5_TRACE) -ne $Pass) { throw "Cargo before valid branch diff: $Name" }
    if ($Name -eq 'committed-whitespace') {
        $data = Get-Content -Raw -LiteralPath (Join-Path $report 'report.json') | ConvertFrom-Json
        if ($data.gates[0].name -ne 'branch-diff' -or $data.gates[0].exit_code -ne 2 -or $data.gates[0].status -ne 'FAIL') { throw 'Whitespace failure report incorrect' }
    }
    if ($Pass) {
        $data = Get-Content -Raw -LiteralPath (Join-Path $report 'report.json') | ConvertFrom-Json
        if ($data.diff_base -ne $Base -or $data.gates[0].command[-1] -ne 'HEAD') { throw 'Missing exact diff command' }
    }
    $results.Add(@{ name = $Name; status = 'PASS'; caller_environment_restored = $true })
}
Push-Location -LiteralPath $repo
try {
    Git-Run @('init','-q')
    Git-Run @('config','user.name','Synthetic')
    Git-Run @('config','user.email','synthetic@example.invalid')
    Git-Run @('config','core.autocrlf','true')
    Set-Content -LiteralPath sample -Value 'base'
    Git-Run @('add','.')
    Git-Run @('commit','-qm','base')
    $base = Git-Run @('rev-parse','HEAD')
    Set-Content -LiteralPath sample -Value 'bad '
    Git-Run @('commit','-qam','committed whitespace defect')
    Check-Case 'committed-whitespace' $base
    Set-Content -LiteralPath sample -Value 'fixed'
    Git-Run @('commit','-qam','fixed')
    Check-Case 'fixed-branch' $base $true
    $fixed = Git-Run @('rev-parse','HEAD')
    Git-Run @('commit','--allow-empty','-qm','empty delta')
    Check-Case 'empty-delta' $fixed
    Check-Case 'missing-base' ''
    Check-Case 'invalid-base' ('f' * 40)
    Check-Case 'head-equal' (Git-Run @('rev-parse','HEAD'))
    Git-Run @('branch','ambiguous')
    Git-Run @('tag','ambiguous')
    Check-Case 'ambiguous-ref' 'ambiguous'
    $tree = Git-Run @('rev-parse','HEAD^{tree}')
    $orphan = 'unrelated' | & git commit-tree $tree
    if ($LASTEXITCODE) { throw 'Cannot create synthetic nonancestor' }
    Check-Case 'nonancestor-base' $orphan
    Check-Case 'existing-report' $base $false $true
    Set-Content -LiteralPath sample -Value 'dirty'
    Check-Case 'dirty-source' $base
    @{ status = 'PASS'; platform = 'Windows'; proof = 'Actual helper and native Git; Cargo function stub for control-flow only'; cases = $results } | ConvertTo-Json -Depth 6
} finally { Pop-Location; Restore-Environment $originalEnvironment; Remove-Item -LiteralPath $root -Recurse -Force }
