# Clean-target acceptance of an assembled ZIP inside a disposable Windows Sandbox.
# Host side: no install/uninstall, no system change. Requires Windows Sandbox to be enabled already.
param([Parameter(Mandatory)][string]$AttemptDir, [Parameter(Mandatory)][string]$Name, [int]$TimeoutMinutes = 25)
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
if ($PSVersionTable.PSVersion.Major -lt 7 -or -not $IsWindows) { throw 'Native Windows PowerShell 7 required' }
if ($Name -notmatch '^sandbox-[0-9]+$') { throw 'Expected sandbox-NN name' }
$repo = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
if (& git -C $repo status --porcelain) { throw 'Clean committed acceptance tooling required' }
$sandboxExe = Join-Path $env:WINDIR 'System32\WindowsSandbox.exe'
if (-not (Test-Path -LiteralPath $sandboxExe)) { throw 'Windows Sandbox is not enabled; nothing is installed by this script' }
if (Get-Process -Name 'WindowsSandboxRemoteSession' -ErrorAction SilentlyContinue) { throw 'A Windows Sandbox is already running; only one may run' }

$attempt = (Resolve-Path -LiteralPath $AttemptDir).Path
$result = Get-Content -LiteralPath (Join-Path $attempt 'package-result.json') -Raw | ConvertFrom-Json
$zip = Join-Path $attempt $result.archive
if ((Get-FileHash -LiteralPath $zip -Algorithm SHA256).Hash.ToLowerInvariant() -ne $result.archive_sha256) { throw 'ZIP hash differs from package-result.json' }
$manifest = Get-Content -LiteralPath (Join-Path $attempt 'package/ACP/runtime/manifest.json') -Raw | ConvertFrom-Json

# The redistributable installer shipped with the same Visual Studio that built the binaries.
$vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio\Installer\vswhere.exe'
$vs = & $vswhere -latest -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
$redist = Join-Path $vs "VC\Redist\MSVC\$($manifest.builder.vc_tools)\vc_redist.x64.exe"
$signature = Get-AuthenticodeSignature -LiteralPath $redist
if ($signature.Status -ne 'Valid' -or $signature.SignerCertificate.Subject -notmatch '(^|, )O=Microsoft Corporation(,|$)') { throw 'vc_redist.x64.exe lacks a valid Microsoft signature' }

# Exchange folders live beside, not inside, the owner-only attempt directory so the
# sandbox mapping service can read them. Synthetic, non-secret content only.
$exchange = Join-Path (Split-Path -Parent $attempt) ((Split-Path -Leaf $attempt) + "-$Name")
if (Test-Path -LiteralPath $exchange) { throw 'Exchange directory collision refused' }
$inDir = Join-Path $exchange 'input'; $outDir = Join-Path $exchange 'output'
New-Item -ItemType Directory -Path $exchange, $inDir, $outDir | Out-Null
Copy-Item -LiteralPath $zip, $redist, (Join-Path $PSScriptRoot 'sandbox-acceptance.ps1') -Destination $inDir
[ordered]@{ archive_sha256 = $result.archive_sha256; vc_redist_sha256 = (Get-FileHash -LiteralPath $redist -Algorithm SHA256).Hash.ToLowerInvariant() } |
    ConvertTo-Json | Set-Content -LiteralPath (Join-Path $inDir 'expected.json')

$wsb = Join-Path $exchange 'acceptance.wsb'
@"
<Configuration>
  <Networking>Disable</Networking>
  <ClipboardRedirection>Disable</ClipboardRedirection>
  <PrinterRedirection>Disable</PrinterRedirection>
  <AudioInput>Disable</AudioInput>
  <VideoInput>Disable</VideoInput>
  <MappedFolders>
    <MappedFolder><HostFolder>$inDir</HostFolder><SandboxFolder>C:\acp-input</SandboxFolder><ReadOnly>true</ReadOnly></MappedFolder>
    <MappedFolder><HostFolder>$outDir</HostFolder><SandboxFolder>C:\acp-output</SandboxFolder><ReadOnly>false</ReadOnly></MappedFolder>
  </MappedFolders>
  <LogonCommand>
    <Command>powershell.exe -NoProfile -ExecutionPolicy Bypass -File C:\acp-input\sandbox-acceptance.ps1</Command>
  </LogonCommand>
</Configuration>
"@ | Set-Content -LiteralPath $wsb -Encoding UTF8

$run = [ordered]@{ tooling_sha = (& git -C $repo rev-parse HEAD); package_source_sha = $manifest.source_sha; archive_sha256 = $result.archive_sha256
    vc_redist = $manifest.builder.vc_tools; vc_redist_signer = $signature.SignerCertificate.Subject; networking = 'Disable'; host_os = [Environment]::OSVersion.Version.ToString() }
Start-Process -FilePath $sandboxExe -ArgumentList "`"$wsb`""
$deadline = (Get-Date).AddMinutes($TimeoutMinutes)
$resultFile = Join-Path $outDir 'result.json'
while (-not (Test-Path -LiteralPath $resultFile) -and (Get-Date) -lt $deadline) { Start-Sleep -Seconds 5 }
$run.completed = Test-Path -LiteralPath $resultFile
# The in-sandbox script shuts the sandbox down; wait for that instead of killing it.
$closeBy = (Get-Date).AddMinutes(3)
while ((Get-Process -Name 'WindowsSandboxRemoteSession' -ErrorAction SilentlyContinue) -and (Get-Date) -lt $closeBy) { Start-Sleep -Seconds 3 }
$run.sandbox_closed = -not (Get-Process -Name 'WindowsSandboxRemoteSession' -ErrorAction SilentlyContinue)
if ($run.completed -and -not $run.sandbox_closed) {
    # Only this run's sandbox can exist (checked at start); close it after the result is safe.
    Stop-Process -Name 'WindowsSandboxRemoteSession' -ErrorAction SilentlyContinue
    $run.sandbox_closed_by_launcher = $true
}
$run | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $exchange 'sandbox-run.json')
if (-not $run.completed) { throw 'Sandbox acceptance produced no result before the timeout; sandbox left for inspection' }
$cells = Get-Content -LiteralPath $resultFile -Raw | ConvertFrom-Json
"passed=$($cells.passed) failed=$($cells.failed) sandbox_closed=$($run.sandbox_closed)"
if ($cells.failed -ne 0) { exit 1 }
