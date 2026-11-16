# Developer-only entrypoint; restore the caller's environment, no prerequisite install.
param([Parameter(Mandatory)][string]$OutputParent, [Parameter(Mandatory)][string]$Name)
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
    $vs = & $vswhere -latest -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
    if (-not $vs) { throw 'Existing x64 build tools required' }
    Import-Module (Join-Path $vs 'Common7\Tools\Microsoft.VisualStudio.DevShell.dll')
    Enter-VsDevShell -VsInstallPath $vs -SkipAutomaticLocation -DevCmdArguments '-arch=x64 -host_arch=x64' | Out-Null
    $env:Path = "$toolchain;$env:Path"
    $env:CARGO_NET_OFFLINE = 'true'
    Remove-Item Env:CARGO_TARGET_DIR -ErrorAction SilentlyContinue
    $parent = (Resolve-Path -LiteralPath $OutputParent).Path
    & node (Join-Path $PSScriptRoot 'windows-package.mjs') $repo $parent $Name (Join-Path $toolchain 'cargo.exe')
    if ($LASTEXITCODE -ne 0) { throw 'Assembly failed; retain private attempt directory' }
    $report = Join-Path $parent $Name
    $stage = Join-Path $report 'acp-windows-x86_64-prototype'
    $zip = Join-Path $report 'acp-windows-x86_64-prototype.zip'
    Add-Type -AssemblyName System.IO.Compression.FileSystem
    [IO.Compression.ZipFile]::CreateFromDirectory($stage, $zip)
    # Extract into a NEW directory only. Inspect entries before any extraction.
    $archive = [IO.Compression.ZipFile]::OpenRead($zip)
    try {
        $names = [Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
        foreach ($entry in $archive.Entries) {
            if ($entry.FullName -notmatch '^[A-Za-z0-9][A-Za-z0-9._/-]*$' -or $entry.FullName.Contains('..') -or $entry.FullName.Contains('\') -or -not $names.Add($entry.FullName)) { throw 'Unsafe or colliding archive entry' }
        }
    } finally { $archive.Dispose() }
    $extracted = Join-Path $report 'extracted'
    if (Test-Path -LiteralPath $extracted) { throw 'Extraction collision refused' }
    [IO.Compression.ZipFile]::ExtractToDirectory($zip, $extracted)
    & node --input-type=module -e 'import {verifyInventory} from "./tools/portable/windows-package.mjs"; import fs from "node:fs"; verifyInventory(process.argv[1],JSON.parse(fs.readFileSync(process.argv[2])));' $extracted (Join-Path $report 'archive-inventory.json')
    if ($LASTEXITCODE -ne 0) { throw 'Extracted archive mismatch' }
    @{ archive_sha256 = (Get-FileHash -LiteralPath $zip -Algorithm SHA256).Hash.ToLowerInvariant(); archive_bytes = (Get-Item -LiteralPath $zip).Length; extract_inventory = 'PASS'; clean_target = 'NOT_RUN'; developer_host = $true } | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $report 'package-result.json')
    Write-Output 'PASS: unpublished ZIP assembled and extracted; clean-target acceptance NOT_RUN'
} finally {
    Pop-Location
    foreach ($key in @([Environment]::GetEnvironmentVariables('Process').Keys)) {
        if (-not $originalEnvironment.Contains($key)) { [Environment]::SetEnvironmentVariable($key, [NullString]::Value, 'Process') }
    }
    foreach ($key in $originalEnvironment.Keys) { [Environment]::SetEnvironmentVariable($key, $originalEnvironment[$key], 'Process') }
}
