param([Parameter(Mandatory)][int]$OwnedPid, [Parameter(Mandatory)][string]$ExpectedExe)
$ErrorActionPreference = 'Stop'
$p = Get-Process -Id $OwnedPid
if ([IO.Path]::GetFullPath($p.Path) -ne [IO.Path]::GetFullPath($ExpectedExe)) { throw 'Owned executable mismatch' }
$windowsRoot = [IO.Path]::GetFullPath($env:WINDIR).TrimEnd('\') + '\'
@($p.Modules | ForEach-Object {
    @{ name = $_.ModuleName; location = $(if ($_.FileName.StartsWith($windowsRoot, [StringComparison]::OrdinalIgnoreCase)) { 'windows-component-or-driver' } elseif ($_.FileName -eq $p.Path) { 'packaged-executable' } else { 'external-component-review-required' }); sha256 = (Get-FileHash -LiteralPath $_.FileName -Algorithm SHA256).Hash.ToLowerInvariant() }
}) | ConvertTo-Json -Depth 4
