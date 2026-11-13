param(
    [ValidateSet('Core', 'UI', 'All')][string]$Suite = 'All',
    [string]$ReportDir = 'artifacts/local-p1-m2/s4-windows'
)
$ErrorActionPreference = 'Stop'
$project = Split-Path -Parent $PSScriptRoot
# Process-local toolchain only: no installations or user/system configuration edits.
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
Push-Location -LiteralPath $project
try {
    $dirty = & git status --porcelain
    if ($LASTEXITCODE -ne 0 -or $dirty) { throw 'Commit source changes before exact-SHA verification' }
    if (Test-Path -LiteralPath $ReportDir) { throw 'Report directory must be new' }
    $report = New-Item -ItemType Directory -Path $ReportDir
    $results = [System.Collections.Generic.List[object]]::new()
    $metadata = @{ git_sha = (& git rev-parse HEAD); worktree_dirty = $false; platform = 'Windows'; version = [System.Environment]::OSVersion.Version.ToString(); rustc = (& rustc --version); cargo = (& cargo --version); gates = $results }
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
        if ($Suite -ne 'UI') {
            Invoke-Gate 'workspace-fmt' @('cargo','fmt','--all','--','--check')
            Invoke-Gate 'workspace-build' @('cargo','build','--workspace','--bins','--locked')
            Invoke-Gate 'workspace-clippy' @('cargo','clippy','--workspace','--all-targets','--locked','--','-D','warnings')
            Invoke-Gate 'workspace-tests' @('cargo','test','--workspace','--all-targets','--locked')
        } else { Invoke-Gate 'process-binaries' @('cargo','build','--workspace','--bins','--locked') }
        if ($Suite -ne 'Core') {
            Invoke-Gate 'shared-fmt' @('cargo','fmt','--manifest-path','spikes/p1-e/shared/Cargo.toml','--','--check')
            Invoke-Gate 'shared-clippy' @('cargo','clippy','--manifest-path','spikes/p1-e/shared/Cargo.toml','--all-targets','--features','native','--locked','--','-D','warnings')
            Invoke-Gate 'shared-tests' @('cargo','test','--manifest-path','spikes/p1-e/shared/Cargo.toml','--features','native','--locked')
            Invoke-Gate 'nsh-fmt' @('cargo','fmt','--manifest-path','spikes/p1-e/nsh/Cargo.toml','--','--check')
            Invoke-Gate 'nsh-build' @('cargo','build','--manifest-path','spikes/p1-e/nsh/Cargo.toml','--features','native,tui','--bins','--locked')
            Invoke-Gate 'nsh-clippy' @('cargo','clippy','--manifest-path','spikes/p1-e/nsh/Cargo.toml','--all-targets','--features','native,tui','--locked','--','-D','warnings')
            Invoke-Gate 'nsh-tests' @('cargo','test','--manifest-path','spikes/p1-e/nsh/Cargo.toml','--all-targets','--features','native,tui','--locked')
        }
        Invoke-Gate 'diff' @('git','diff','--check')
    } finally { $metadata | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath (Join-Path $report.FullName 'report.json') }
} finally { Pop-Location }
