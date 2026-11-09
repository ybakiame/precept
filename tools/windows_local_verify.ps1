param(
    [switch]$Core, [switch]$Service, [switch]$UI, [switch]$All,
    [switch]$LocalProject, [switch]$WSLg, [switch]$UIFull, [string]$ReportDir, [string]$Python = 'python'
)
$ErrorActionPreference = 'Stop'
$project = Split-Path -Parent $PSScriptRoot
# Process-local toolchain setup. No installations, user config or profile edits.
$rust = Join-Path $env:USERPROFILE '.cargo\bin'
if (Test-Path $rust) { $env:Path = "$rust;$env:Path" }
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
$arguments = @('tools/local_verify.py')
foreach ($entry in @(@($Core,'--core'),@($Service,'--service'),@($UI,'--ui'),@($All,'--all'),@($LocalProject,'--local-project'),@($WSLg,'--wslg'),@($UIFull,'--ui-full'))) {
    if ($entry[0]) { $arguments += $entry[1] }
}
if ($ReportDir) { $arguments += @('--report-dir', $ReportDir) }
Push-Location -LiteralPath $project
try {
    & $Python @arguments
    exit $LASTEXITCODE
} finally { Pop-Location }
