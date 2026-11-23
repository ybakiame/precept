# Developer-only entrypoint; restore the caller's environment, no prerequisite install.
# VsDevShell supplies VCToolsRedistDir, the source of the app-local VC++ runtime DLLs.
# -Release <version>: a published build of that version (it must be the source's);
# without it, an unpublished prototype named by commit.
param([Parameter(Mandatory)][string]$OutputParent, [Parameter(Mandatory)][string]$Name, [string]$Release = '')
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
if ($PSVersionTable.PSVersion.Major -lt 7 -or -not $IsWindows) { throw 'Native Windows PowerShell 7 required' }
$repo = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
$originalEnvironment = [Environment]::GetEnvironmentVariables('Process')
Push-Location -LiteralPath $repo
try {
    # Select the existing STOCK pinned toolchain explicitly, bypass global Cargo forks.
    $toolchain = Join-Path $env:USERPROFILE '.rustup\toolchains\1.99.1-x86_64-pc-windows-msvc\bin'
    if (-not (Test-Path -LiteralPath (Join-Path $toolchain 'cargo.exe'))) { throw 'Pinned stock toolchain missing; no installation attempted' }
    $env:Path = "$toolchain;$env:Path"
    $vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio\Installer\vswhere.exe'
    $vs = & $vswhere -latest -prerelease -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
    if (-not $vs) { throw 'Existing x64 build tools required' }
    Import-Module (Join-Path $vs 'Common7\Tools\Microsoft.VisualStudio.DevShell.dll')
    Enter-VsDevShell -VsInstallPath $vs -SkipAutomaticLocation -DevCmdArguments '-arch=x64 -host_arch=x64' | Out-Null
    $env:Path = "$toolchain;$env:Path"
    # The recipe itself strips inherited RUSTFLAGS/wrapper/target/profile overrides for Cargo.
    $env:CARGO_NET_OFFLINE = 'true'
    $parent = (Resolve-Path -LiteralPath $OutputParent).Path
    $nodeArgs = @((Join-Path $PSScriptRoot 'windows-package.mjs'), $repo, $parent, $Name, (Join-Path $toolchain 'cargo.exe'))
    if ($Release) {
        if ($Release -notmatch '^\d+\.\d+\.\d+$') { throw 'Release must be a semantic version' }
        $nodeArgs += $Release
    }
    & node @nodeArgs
    if ($LASTEXITCODE -ne 0) { throw 'Assembly failed; retain private attempt directory' }
    $report = Join-Path $parent $Name
    $packageRoot = Join-Path $report 'package'
    $source = Get-Content -LiteralPath (Join-Path $packageRoot 'Precept/runtime/manifest.json') -Raw | ConvertFrom-Json
    $zip = if ($Release) {
        Join-Path $report ("precept-" + $Release + "-windows-x64.zip")
    } else {
        Join-Path $report ("precept-windows-x64-prototype-" + $source.source_sha.Substring(0,12) + ".zip")
    }
    Add-Type -AssemblyName System.IO.Compression.FileSystem
    # Entries are Precept/...; extraction always creates one product directory.
    [IO.Compression.ZipFile]::CreateFromDirectory($packageRoot, $zip)
    # Extract into a NEW directory only. Inspect entries before any extraction.
    $archive = [IO.Compression.ZipFile]::OpenRead($zip)
    try {
        $names = [Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
        foreach ($entry in $archive.Entries) {
            $n = $entry.FullName
            if ($n -cnotmatch '^Precept/([A-Za-z0-9][A-Za-z0-9._-]*/)*[A-Za-z0-9][A-Za-z0-9._-]*$' -or $n.Contains('..') -or -not $names.Add($n)) { throw 'Unsafe or colliding archive entry' }
        }
    } finally { $archive.Dispose() }
    $extracted = Join-Path $report 'extracted'
    if (Test-Path -LiteralPath $extracted) { throw 'Extraction collision refused' }
    [IO.Compression.ZipFile]::ExtractToDirectory($zip, $extracted)
    & node (Join-Path $PSScriptRoot 'windows-package.mjs') --verify $extracted (Join-Path $report 'archive-inventory.json')
    if ($LASTEXITCODE -ne 0) { throw 'Extracted archive mismatch' }
    [ordered]@{ archive = (Split-Path -Leaf $zip); archive_sha256 = (Get-FileHash -LiteralPath $zip -Algorithm SHA256).Hash.ToLowerInvariant(); archive_bytes = (Get-Item -LiteralPath $zip).Length; archive_entries = $names.Count; extract_inventory = 'PASS'; clean_target = 'NOT_RUN'; developer_host = $true } | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $report 'package-result.json')
    Write-Output 'PASS: ZIP assembled and extracted; clean-target acceptance NOT_RUN'
} finally {
    Pop-Location
    foreach ($key in @([Environment]::GetEnvironmentVariables('Process').Keys)) {
        if (-not $originalEnvironment.Contains($key)) { [Environment]::SetEnvironmentVariable($key, [NullString]::Value, 'Process') }
    }
    foreach ($key in $originalEnvironment.Keys) { [Environment]::SetEnvironmentVariable($key, $originalEnvironment[$key], 'Process') }
}
