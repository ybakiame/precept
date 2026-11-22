param(
    [ValidateSet('Core', 'UI', 'All')][string]$Suite = 'All',
    [string]$ReportDir = '.tmp/artifacts/local-p1-m2/s5-windows',
    [string]$DiffBase
)
$ErrorActionPreference = 'Stop'
$project = Split-Path -Parent $PSScriptRoot
$originalEnvironment = [Environment]::GetEnvironmentVariables('Process')
Push-Location -LiteralPath $project
try {
    # Process-local toolchain only; restore caller environment even on setup failure.
    $rustBin = Join-Path $env:USERPROFILE '.cargo\bin'
    if (Test-Path $rustBin) { $env:Path = "$rustBin;$env:Path" }
    $vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio\Installer\vswhere.exe'
    if (Test-Path $vswhere) {
        $visualStudio = & $vswhere -latest -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
        if ($visualStudio) {
            $module = Join-Path $visualStudio 'Common7\Tools\Microsoft.VisualStudio.DevShell.dll'
            if (Test-Path $module) {
                Import-Module $module
                Enter-VsDevShell -VsInstallPath $visualStudio -SkipAutomaticLocation -DevCmdArguments '-arch=x64 -host_arch=x64' | Out-Null
            }
        }
    }
    $dirty = & git status --porcelain
    if ($LASTEXITCODE -ne 0 -or $dirty) { throw 'Commit source changes before exact-SHA verification' }
    if ($DiffBase -notmatch '^[0-9a-fA-F]{40}$') { throw 'Baseline must be one existing full commit SHA' }
    $kind = & git cat-file -t $DiffBase 2>$null
    if ($LASTEXITCODE -ne 0 -or $kind -ne 'commit') { throw 'Baseline must be one existing full commit SHA' }
    $baseline = & git rev-parse --verify $DiffBase
    $headSha = & git rev-parse HEAD
    & git merge-base --is-ancestor $baseline HEAD
    if ($LASTEXITCODE -ne 0 -or $baseline -eq $headSha) { throw 'Baseline must be an ancestor with a nonempty branch delta' }
    & git diff --quiet $baseline HEAD
    if ($LASTEXITCODE -eq 0) { throw 'Baseline must have a nonempty branch delta' }
    if ($LASTEXITCODE -ne 1) { throw 'Cannot inspect branch delta' }
    if (Test-Path -LiteralPath $ReportDir) { throw 'Report directory must be new' }
    $report = New-Item -ItemType Directory -Path $ReportDir
    $results = [System.Collections.Generic.List[object]]::new()
    $metadata = @{ git_sha = $headSha; diff_base = $baseline; worktree_dirty = $false; platform = 'Windows'; version = [System.Environment]::OSVersion.Version.ToString(); gates = $results }
    function Invoke-Gate([string]$Name, [string[]]$Arguments) {
        $program = $Arguments[0]
        $gateArguments = $Arguments[1..($Arguments.Count-1)]
        & $program @gateArguments *> (Join-Path $report.FullName "$Name.log")
        $code = $LASTEXITCODE
        $results.Add(@{ name = $Name; command = $Arguments; exit_code = $code; status = $(if ($code -eq 0) { 'PASS' } else { 'FAIL' }) })
        Write-Host "$Name $($results[-1].status)"
        if ($code -ne 0) { throw "Gate failed: $Name ($code)" }
    }
    try {
        Invoke-Gate 'branch-diff' @('git','diff','--check',$baseline,'HEAD')
        $metadata.rustc = & rustc --version
        $metadata.cargo = & cargo --version
        if ($Suite -ne 'UI') {
            Invoke-Gate 'workspace-fmt' @('cargo','fmt','--all','--','--check')
            Invoke-Gate 'workspace-build' @('cargo','build','--workspace','--bins','--locked')
            Invoke-Gate 'workspace-clippy' @('cargo','clippy','--workspace','--all-targets','--locked','--','-D','warnings')
            Invoke-Gate 'workspace-tests' @('cargo','test','--workspace','--all-targets','--locked')
        } else { Invoke-Gate 'process-binaries' @('cargo','build','--workspace','--bins','--locked') }
        if ($Suite -ne 'Core') {
            Invoke-Gate 'apps-fmt' @('cargo','fmt','--manifest-path','apps/Cargo.toml','--all','--','--check')
            Invoke-Gate 'apps-build' @('cargo','build','--manifest-path','apps/Cargo.toml','--workspace','--features','precept-ui/native','--bins','--locked')
            Invoke-Gate 'apps-clippy' @('cargo','clippy','--manifest-path','apps/Cargo.toml','--workspace','--all-targets','--features','precept-ui/native','--locked','--','-D','warnings')
            Invoke-Gate 'apps-tests' @('cargo','test','--manifest-path','apps/Cargo.toml','--workspace','--all-targets','--features','precept-ui/native','--locked')
        }
        Invoke-Gate 'diff' @('git','diff','--check')
    } finally { $metadata | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath (Join-Path $report.FullName 'report.json') }
} finally {
    Pop-Location
    foreach ($key in @([Environment]::GetEnvironmentVariables('Process').Keys)) {
        if (-not $originalEnvironment.Contains($key)) {
            # PowerShell coerces $null to ""; .NET 9+ preserves empty variables.
            [Environment]::SetEnvironmentVariable($key, [NullString]::Value, 'Process')
        }
    }
    foreach ($key in $originalEnvironment.Keys) {
        [Environment]::SetEnvironmentVariable($key, $originalEnvironment[$key], 'Process')
    }
}
