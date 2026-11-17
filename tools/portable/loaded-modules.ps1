# Loaded-module classes for one owned process. Records basenames/hashes, never absolute paths.
param([Parameter(Mandatory)][int]$OwnedPid, [Parameter(Mandatory)][string]$ExpectedExe, [Parameter(Mandatory)][string]$PackageBin)
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
$p = Get-Process -Id $OwnedPid
function Local-Path([string]$Value) {
    $full = [IO.Path]::GetFullPath($Value)
    if ($full.StartsWith('\\?\', [StringComparison]::Ordinal)) { $full = $full.Substring(4) }
    return $full
}
$exe = Local-Path $p.Path
if ($exe -ne (Local-Path $ExpectedExe)) { throw 'Owned executable mismatch' }
$bin = (Local-Path $PackageBin).TrimEnd('\') + '\'
$windowsRoot = [IO.Path]::GetFullPath($env:WINDIR).TrimEnd('\') + '\'
ConvertTo-Json -Depth 4 -AsArray @($p.Modules | ForEach-Object {
    $file = Local-Path $_.FileName
    $location = if ($file -eq $exe) { 'packaged-executable' }
        elseif ($file.StartsWith($bin, [StringComparison]::OrdinalIgnoreCase) -and -not $file.Substring($bin.Length).Contains('\')) { 'packaged-app-local' }
        elseif ($file.StartsWith($windowsRoot, [StringComparison]::OrdinalIgnoreCase)) { 'windows-component-or-driver' }
        else { 'external-component-review-required' }
    [ordered]@{ name = $_.ModuleName; location = $location; sha256 = (Get-FileHash -LiteralPath $file -Algorithm SHA256).Hash.ToLowerInvariant() }
})
