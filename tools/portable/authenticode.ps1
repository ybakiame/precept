# Reports embedded Authenticode status of the exact staged files. Read-only; no trust-store change.
param([Parameter(Mandatory)][string]$Directory)
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
$item = Get-Item -LiteralPath $Directory -Force
if (-not $item.PSIsContainer -or ($item.Attributes -band [IO.FileAttributes]::ReparsePoint)) { throw 'Expected regular staged directory' }
# Pipe, never pass the array as -InputObject: that would nest it one level deeper.
@(Get-ChildItem -LiteralPath $Directory -File -Force | Sort-Object Name | ForEach-Object {
    $signature = Get-AuthenticodeSignature -LiteralPath $_.FullName
    [ordered]@{
        file = $_.Name
        status = [string]$signature.Status
        signer = $(if ($signature.SignerCertificate) { $signature.SignerCertificate.Subject } else { $null })
        thumbprint = $(if ($signature.SignerCertificate) { $signature.SignerCertificate.Thumbprint } else { $null })
        file_version = $_.VersionInfo.FileVersion
    }
}) | ConvertTo-Json -Depth 3 -AsArray
