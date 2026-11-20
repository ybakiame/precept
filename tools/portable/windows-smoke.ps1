# Developer host smoke of an already assembled package. No provisioning/install.
# PackageDir is the extraction root that contains Precept/.
param([Parameter(Mandatory)][string]$PackageDir, [Parameter(Mandatory)][string]$ReportDir)
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
if ($PSVersionTable.PSVersion.Major -lt 7 -or -not $IsWindows) { throw 'Native Windows PowerShell 7 required' }
$repo = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
$originalEnvironment = [Environment]::GetEnvironmentVariables('Process')
Push-Location -LiteralPath $repo
try {
    if (& git status --porcelain) { throw 'Clean committed harness required' }
    if (Test-Path -LiteralPath $ReportDir) { throw 'Report collision refused' }
    $package = (Resolve-Path -LiteralPath $PackageDir).Path
    $manifest = Get-Content -LiteralPath (Join-Path $package 'Precept/runtime/manifest.json') -Raw | ConvertFrom-Json
    $toolchain = Join-Path $env:USERPROFILE '.rustup\toolchains\1.99.1-x86_64-pc-windows-msvc\bin'
    $vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio\Installer\vswhere.exe'
    $vs = & $vswhere -latest -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
    if (-not $vs) { throw 'Existing build tools required' }
    Import-Module (Join-Path $vs 'Common7\Tools\Microsoft.VisualStudio.DevShell.dll')
    Enter-VsDevShell -VsInstallPath $vs -SkipAutomaticLocation -DevCmdArguments '-arch=x64 -host_arch=x64' | Out-Null
    $env:Path = "$toolchain;$env:Path"
    $env:CARGO_NET_OFFLINE = 'true'
    $cargo = Join-Path $toolchain 'cargo.exe'
    & node --test tools/portable/windows-package.test.mjs
    if ($LASTEXITCODE -ne 0) { throw 'Packaging security tests failed' }
    & $cargo fmt --manifest-path tools/native-acceptance/Cargo.toml -- --check
    if ($LASTEXITCODE -ne 0) { throw 'Harness format failed' }
    & $cargo clippy --manifest-path tools/native-acceptance/Cargo.toml --bin precept-portable-smoke --locked -- -D warnings
    if ($LASTEXITCODE -ne 0) { throw 'Harness Clippy failed' }
    & $cargo test --manifest-path tools/native-acceptance/Cargo.toml --bin precept-portable-smoke --locked
    if ($LASTEXITCODE -ne 0) { throw 'Harness terminal observer tests failed' }
    & $cargo build --manifest-path tools/native-acceptance/Cargo.toml --bin precept-portable-smoke --locked
    if ($LASTEXITCODE -ne 0) { throw 'Harness build failed' }
    $driver = Join-Path $repo 'tools/native-acceptance/target/debug/precept-portable-smoke.exe'
    & $driver $package ([IO.Path]::GetFullPath($ReportDir)) (Join-Path $repo 'tools/native_window.ps1')
    $smokeExit = $LASTEXITCODE
    if (Test-Path -LiteralPath $ReportDir) {
        @{ harness_sha = (& git rev-parse HEAD); harness_sha256 = (Get-FileHash -LiteralPath $driver -Algorithm SHA256).Hash.ToLowerInvariant(); package_source_sha = $manifest.source_sha; os_build = [Environment]::OSVersion.Version.ToString(); actor = 'automated owned synthetic smoke'; environment = 'Developer host; not isolated clean target'; exit_code = $smokeExit } | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $ReportDir 'harness.json')
    }
    if ($smokeExit -ne 0) { throw 'Packaged process smoke failed; preserve owned state and partial reports' }
} finally {
    Pop-Location
    foreach ($key in @([Environment]::GetEnvironmentVariables('Process').Keys)) {
        if (-not $originalEnvironment.Contains($key)) { [Environment]::SetEnvironmentVariable($key, [NullString]::Value, 'Process') }
    }
    foreach ($key in $originalEnvironment.Keys) { [Environment]::SetEnvironmentVariable($key, $originalEnvironment[$key], 'Process') }
}
