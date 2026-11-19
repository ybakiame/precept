#Requires -Version 7
<#
Deploys a locally built ACP Windows portable ZIP (tools/portable/windows-package.ps1)
into a portable product directory for private dogfooding, and creates one shortcut
inside it that starts the Desktop against the ACP daemon in WSL2.

Portable mode: no installer, service, PATH, registry, Start menu or startup entry.
State stays in WSL (the daemon is the only Profile write owner); nothing here creates,
selects or write-enables a Profile.

  ./tools/dogfood/windows-deploy.ps1 -AttemptDir <.tmp\artifacts\local-p1-m5\NAME> `
      -Distribution <wsl-distro> -User <linux-user> [-Destination <dir>] [-Replace]

-Replace removes only the files listed in the installed runtime\manifest.json (after
checking that no ACP process runs from the destination) and keeps unknown files.
#>
param(
  [Parameter(Mandatory)] [string] $AttemptDir,
  [Parameter(Mandatory)] [ValidatePattern('^[A-Za-z0-9][A-Za-z0-9._ -]{0,63}$')] [string] $Distribution,
  [Parameter(Mandatory)] [ValidatePattern('^[A-Za-z0-9_][A-Za-z0-9_-]{0,63}$')] [string] $User,
  [string] $Destination = (Join-Path $env:LOCALAPPDATA 'Programs\ACP'),
  [string] $WslBridge = '/usr/bin/acp-wsl-bridge',
  [string] $WslStateDir,
  [ValidateSet('en', 'zh-CN')] [string] $Locale = 'zh-CN',
  [switch] $Replace
)
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
function Fail($m) { throw "windows-deploy: $m" }

$AttemptDir = [IO.Path]::GetFullPath($AttemptDir)
$Destination = [IO.Path]::GetFullPath($Destination)
if (-not $WslStateDir) { $WslStateDir = "/home/$User/.local/state/acp" }
foreach ($p in @($WslBridge, $WslStateDir)) {
  if ($p -notmatch '^/[^\x00-\x1f]*$' -or ($p -split '/') -contains '..') { Fail "Linux path must be absolute: $p" }
}

# 1. The ZIP and its recorded hash from the packaging attempt.
$result = Get-Content -LiteralPath (Join-Path $AttemptDir 'package-result.json') -Raw | ConvertFrom-Json
if ($result.extract_inventory -ne 'PASS') { Fail 'packaging attempt did not pass its extracted inventory' }
$zip = Join-Path $AttemptDir $result.archive
if ((Get-FileHash -LiteralPath $zip -Algorithm SHA256).Hash.ToLower() -ne $result.archive_sha256) { Fail 'ZIP hash mismatch' }

# 2. Destination: empty/new, or -Replace of a previous ACP deployment.
if (Test-Path -LiteralPath $Destination) {
  $item = Get-Item -LiteralPath $Destination -Force
  if ($item.LinkType) { Fail 'destination is a link' }
  $existing = @(Get-ChildItem -LiteralPath $Destination -Force)
  if ($existing.Count -gt 0) {
    $old = Join-Path $Destination 'runtime\manifest.json'
    if (-not $Replace -or -not (Test-Path -LiteralPath $old)) { Fail 'destination is not empty (use -Replace for a previous ACP deployment)' }
    $running = Get-Process | Where-Object { $_.Path -and $_.Path.StartsWith("$Destination\", [StringComparison]::OrdinalIgnoreCase) }
    if ($running) { Fail "ACP processes still run from the destination: $($running.Name -join ', ')" }
    $m = Get-Content -LiteralPath $old -Raw | ConvertFrom-Json
    foreach ($f in $m.files) {
      if ($f.file -notmatch '^(bin|notices)/[A-Za-z0-9._-]+$|^(README|LICENSE)\.txt$') { Fail "unexpected manifest entry $($f.file)" }
      $path = Join-Path $Destination $f.file
      if (Test-Path -LiteralPath $path) { Remove-Item -LiteralPath $path -Force }
    }
    Remove-Item -LiteralPath $old -Force
    $lnk = Join-Path $Destination 'ACP Desktop (WSL).lnk'
    if (Test-Path -LiteralPath $lnk) { Remove-Item -LiteralPath $lnk -Force }
    foreach ($d in 'bin', 'notices', 'runtime') {
      $dir = Join-Path $Destination $d
      if ((Test-Path -LiteralPath $dir) -and -not (Get-ChildItem -LiteralPath $dir -Force)) { Remove-Item -LiteralPath $dir }
    }
  }
} else {
  New-Item -ItemType Directory -Path $Destination | Out-Null
}

# 3. Extract into a fresh staging directory next to the destination, verify, then move.
$stage = Join-Path (Split-Path $Destination) ".acp-deploy-$([guid]::NewGuid().ToString('N'))"
New-Item -ItemType Directory -Path $stage | Out-Null
try {
  Expand-Archive -LiteralPath $zip -DestinationPath $stage
  $root = Join-Path $stage 'ACP'
  $manifest = Get-Content -LiteralPath (Join-Path $root 'runtime\manifest.json') -Raw | ConvertFrom-Json
  foreach ($f in $manifest.files) {
    if ((Get-FileHash -LiteralPath (Join-Path $root $f.file) -Algorithm SHA256).Hash.ToLower() -ne $f.sha256) { Fail "file hash mismatch: $($f.file)" }
  }
  foreach ($entry in Get-ChildItem -LiteralPath $root -Force) {
    $target = Join-Path $Destination $entry.Name
    if ($entry.PSIsContainer -and (Test-Path -LiteralPath $target)) {
      foreach ($child in Get-ChildItem -LiteralPath $entry.FullName -Force) {
        $t = Join-Path $target $child.Name
        if (Test-Path -LiteralPath $t) { Fail "refusing to overwrite unknown file $t" }
        Move-Item -LiteralPath $child.FullName -Destination $t
      }
    } else {
      if (Test-Path -LiteralPath $target) { Fail "refusing to overwrite unknown file $target" }
      Move-Item -LiteralPath $entry.FullName -Destination $target
    }
  }
} finally {
  if (Test-Path -LiteralPath $stage) { Remove-Item -LiteralPath $stage -Recurse -Force }
}

# 4. One shortcut inside the product directory (portable; nothing outside it).
$arguments = @(
  '--locale', $Locale,
  '--wsl-distribution', $Distribution, '--wsl-user', $User, '--wsl-bridge', $WslBridge,
  '--wsl-runtime-dir', "$WslStateDir/runtime", '--wsl-endpoint', "$WslStateDir/runtime/control.sock"
) | ForEach-Object { if ($_ -match '\s') { '"' + $_ + '"' } else { $_ } }
$shell = New-Object -ComObject WScript.Shell
$link = $shell.CreateShortcut((Join-Path $Destination 'ACP Desktop (WSL).lnk'))
$link.TargetPath = Join-Path $Destination 'bin\acp-nsh-native.exe'
$link.Arguments = $arguments -join ' '
$link.WorkingDirectory = $Destination
$link.Description = 'ACP Desktop connected to the ACP daemon in WSL2'
$link.Save()

[pscustomobject]@{
  destination_files = (Get-ChildItem -LiteralPath $Destination -Recurse -File).Count
  source_sha = $manifest.source_sha
  archive_sha256 = $result.archive_sha256
  shortcut = 'ACP Desktop (WSL).lnk'
  status = 'DEPLOYED'
} | ConvertTo-Json
