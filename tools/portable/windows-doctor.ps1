#Requires -Version 7
<#
Read-only verification of an extracted Precept package. This script never starts
preceptd, creates or selects a Profile, enables writes, installs an integration,
scans projects, repairs files or writes reports. It prints one redacted JSON
document. Native verification is the default; -WslOnly is for WSL deployments.

Exit codes: 0 = ok, 1 = attention, 2 = problem, 3 = incomplete/unknown/not-run.
#>
param(
  [Parameter(Mandatory)] [string] $PreceptDir,
  [string] $RuntimeDir = '',
  [string] $Endpoint = '',
  [string] $Project = '',
  [switch] $VerifyWsl,
  [string] $Distribution = '',
  [string] $User = '',
  [string] $WslRuntimeDir = '',
  [string] $WslEndpoint = '',
  [string] $WslProject = '',
  [switch] $WslOnly
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$script:KnownCheckIds = [Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
foreach ($id in ('executable.precept executable.preceptd executable.precept_mcp executable.precept_hook executable.precept_bridge ' +
  'runtime.location endpoint.location credential.operator daemon.health daemon.api daemon.product profile.schema ' +
  'profile.selected profile.writes project.count project.binding project.state project.sharing project.propose ' +
  'project.gate project.mcp project.credential integration.mcp.availability integration.mcp.claude ' +
  'integration.mcp.codex integration.hooks.availability integration.hooks.claude integration.hooks.codex ' +
  'integration.gate.claude integration.gate.codex integration.pointer package.manifest package.version ' +
  'package.inventory report.render') -split '\s+') {
  if ($id) { [void]$script:KnownCheckIds.Add($id) }
}

$script:KnownReasons = [Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
foreach ($reason in ('executable_ok executable_missing executable_unavailable executable_unsafe runtime_ok runtime_unsafe ' +
  'runtime_not_initialized endpoint_ok endpoint_invalid endpoint_unknown endpoint_collision credential_ok ' +
  'credential_not_issued credential_invalid credential_unsafe credential_wrong_role ' +
  'daemon_ok daemon_unavailable daemon_authorization_failed daemon_api_incompatible daemon_not_ready ' +
  'daemon_service_mismatch daemon_request_failed daemon_not_checked api_compatible api_incompatible api_unknown ' +
  'product_compatible product_incompatible product_unknown profile_selected profile_not_selected ' +
  'profile_startup_failed profile_state_mismatch profile_state_unknown schema_compatible schema_incompatible ' +
  'schema_unselected schema_unknown writes_enabled writes_disabled writes_unknown projects_listed ' +
  'projects_list_unavailable project_bound project_not_requested project_name_invalid project_not_found ' +
  'project_ready project_unregistered project_unavailable project_state_unknown project_mcp_available ' +
  'project_mcp_unavailable project_mcp_mismatch sharing_enabled sharing_disabled sharing_unknown ' +
  'propose_enabled propose_disabled propose_requires_writes propose_unknown gate_configured gate_disabled ' +
  'gate_unknown agent_credential_ok agent_credential_not_issued agent_credential_withheld agent_credential_invalid ' +
  'agent_credential_unsafe agent_credential_wrong_role agent_credential_unavailable mcp_available mcp_unavailable ' +
  'mcp_current mcp_not_installed mcp_outdated mcp_malformed mcp_state_unknown hook_available hook_unavailable ' +
  'hook_current hook_not_installed hook_outdated hook_malformed hook_state_unknown pointer_current ' +
  'pointer_outdated pointer_absent pointer_malformed pointer_state_unknown manifest_present manifest_absent ' +
  'manifest_unreadable manifest_invalid manifest_unavailable package_version_compatible ' +
  'package_version_incompatible package_version_unknown package_metadata_consistent package_inventory_mismatch ' +
  'package_size_mismatch package_hash_metadata_invalid package_metadata_unknown response_invalid') -split '\s+') {
  if ($reason) { [void]$script:KnownReasons.Add($reason) }
}

$script:KnownRemediations = [Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
foreach ($remediation in ('none reinstall_package use_absolute_private_runtime correct_endpoint start_daemon_explicitly ' +
  'restart_daemon_explicitly repair_operator_credential reinstall_matching_release select_profile_explicitly ' +
  'review_startup_profile review_write_grant register_project_explicitly restore_project_path review_project ' +
  'review_project_mcp review_project_sharing review_propose review_gate review_agent_credential ' +
  'review_mcp_integration review_hook_integration review_pointer reextract_package use_supported_profile ' +
  'review_installation') -split '\s+') {
  if ($remediation) { [void]$script:KnownRemediations.Add($remediation) }
}

function ConvertTo-SafeDoctorReport([string]$Text) {
  try {
    if (-not $Text -or $Text.Length -gt 524288) { return $null }
    $raw = $Text | ConvertFrom-Json -AsHashtable -Depth 32
    if ($raw -isnot [Collections.IDictionary]) { return $null }
    foreach ($key in @('format', 'version', 'product_version', 'platform', 'checks')) {
      if (-not $raw.ContainsKey($key)) { return $null }
    }
    if ($raw.format -cne 'precept-doctor' -or $raw.version -ne 1) { return $null }
    if ($raw.platform -notin @('windows', 'linux')) { return $null }
    if ($raw.product_version -isnot [string] -or $raw.product_version.Length -gt 64 -or
        $raw.product_version -notmatch '^[0-9]+\.[0-9]+\.[0-9]+(?:-[0-9A-Za-z.-]+)?(?:\+[0-9A-Za-z.-]+)?$') {
      return $null
    }
    if ($raw.checks -isnot [array] -or $raw.checks.Count -lt 1 -or $raw.checks.Count -gt 512) { return $null }

    $rows = @()
    $counts = @{ ok = 0; attention = 0; problem = 0; off = 0; unknown = 0 }
    foreach ($row in $raw.checks) {
      if ($row -isnot [Collections.IDictionary]) { return $null }
      foreach ($key in @('id', 'status', 'reason', 'remediation')) {
        if (-not $row.ContainsKey($key) -or $row[$key] -isnot [string]) { return $null }
      }
      $id = [string]$row.id
      $status = [string]$row.status
      $reason = [string]$row.reason
      $remediation = [string]$row.remediation
      if ($id.Length -gt 96 -or -not $script:KnownCheckIds.Contains($id)) { return $null }
      if ($status -notin @('ok', 'attention', 'problem', 'off', 'unknown')) { return $null }
      if (-not $script:KnownReasons.Contains($reason) -or -not $script:KnownRemediations.Contains($remediation)) {
        return $null
      }
      $counts[$status]++
      # Facts are deliberately omitted. Rebuild every emitted field from fixed
      # tokens instead of forwarding daemon, manifest or process-controlled JSON.
      $rows += [ordered]@{ id = $id; status = $status; reason = $reason; remediation = $remediation }
    }
    $overall = if ($counts.problem -gt 0) { 'problem' }
      elseif ($counts.unknown -gt 0) { 'incomplete' }
      elseif ($counts.attention -gt 0) { 'attention' }
      else { 'ok' }
    return [ordered]@{
      format = 'precept-doctor'
      version = 1
      product_version = [string]$raw.product_version
      platform = [string]$raw.platform
      summary = [ordered]@{
        status = $overall
        ok = $counts.ok
        attention = $counts.attention
        problem = $counts.problem
        off = $counts.off
        unknown = $counts.unknown
      }
      checks = @($rows)
    }
  } catch {
    return $null
  }
}

function Invoke-BoundedProcess([string]$FileName, [string[]]$Arguments, [string]$WorkingDirectory) {
  $process = $null
  $stdoutTask = $null
  $stderrTask = $null
  $output = [Text.StringBuilder]::new()
  try {
    $start = [Diagnostics.ProcessStartInfo]::new()
    $start.FileName = $FileName
    $start.WorkingDirectory = $WorkingDirectory
    $start.UseShellExecute = $false
    $start.CreateNoWindow = $true
    $start.RedirectStandardOutput = $true
    $start.RedirectStandardError = $true
    foreach ($argument in $Arguments) { [void]$start.ArgumentList.Add([string]$argument) }
    $process = [Diagnostics.Process]::Start($start)
    if (-not $process) { return [pscustomobject]@{ State = 'unavailable'; Json = $null } }

    $stdoutBuffer = New-Object char[] 4096
    $stderrBuffer = New-Object char[] 4096
    $stdoutTask = $process.StandardOutput.ReadAsync($stdoutBuffer, 0, $stdoutBuffer.Length)
    $stderrTask = $process.StandardError.ReadAsync($stderrBuffer, 0, $stderrBuffer.Length)
    $outputLength = 0
    $limited = $false
    $timedOut = $false
    $watch = [Diagnostics.Stopwatch]::StartNew()

    while ($null -ne $stdoutTask -or $null -ne $stderrTask) {
      if ($watch.Elapsed.TotalSeconds -ge 22) { $timedOut = $true; break }
      if (-not $process.HasExited -and $watch.Elapsed.TotalSeconds -ge 20) {
        $timedOut = $true
        try { $process.Kill($true) } catch { }
      }
      if ($limited) {
        try { if (-not $process.HasExited) { $process.Kill($true) } } catch { }
      }

      $pending = @()
      if ($null -ne $stdoutTask) { $pending += $stdoutTask }
      if ($null -ne $stderrTask) { $pending += $stderrTask }
      $completed = [Threading.Tasks.Task]::WaitAny([Threading.Tasks.Task[]]$pending, 50)
      if ($completed -lt 0) { continue }
      $task = $pending[$completed]
      $count = 0
      try { $count = $task.GetAwaiter().GetResult() } catch { }
      if ($task -eq $stdoutTask) {
        if ($count -eq 0) {
          $stdoutTask = $null
        } else {
          if (-not $limited) {
            if ($outputLength + $count -gt 524288) {
              $limited = $true
              [void]$output.Clear()
            } else {
              [void]$output.Append($stdoutBuffer, 0, $count)
              $outputLength += $count
            }
          }
          $stdoutTask = $process.StandardOutput.ReadAsync($stdoutBuffer, 0, $stdoutBuffer.Length)
        }
      } elseif ($count -eq 0) {
        $stderrTask = $null
      } else {
        # Stderr is drained only to prevent pipe deadlock and is never retained.
        $stderrTask = $process.StandardError.ReadAsync($stderrBuffer, 0, $stderrBuffer.Length)
      }
    }

    if (-not $process.WaitForExit(2000)) {
      $timedOut = $true
      try { $process.Kill($true) } catch { }
    }
    if ($watch.Elapsed.TotalSeconds -ge 22) { $timedOut = $true }
    if ($timedOut) { return [pscustomobject]@{ State = 'timeout'; Json = $null } }
    if ($limited) { return [pscustomobject]@{ State = 'limit'; Json = $null } }
    return [pscustomobject]@{
      State = if ($process.ExitCode -eq 0) { 'ok' } else { 'nonzero' }
      Json = $output.ToString()
    }
  } catch {
    return [pscustomobject]@{ State = 'unavailable'; Json = $null }
  } finally {
    foreach ($task in @($stdoutTask, $stderrTask)) {
      if ($null -ne $task) {
        try { [void]$task.AsyncWaitHandle } catch { }
      }
    }
    if ($null -ne $process) { $process.Dispose() }
  }
}

function New-NotRunTarget([string]$Target, [string]$Reason) {
  return [pscustomobject]@{
    target = $Target
    status = 'not_run'
    reason = $Reason
    remediation = 'none'
    report = $null
  }
}

function New-FailedTarget([string]$Target, [string]$Reason, [string]$Remediation) {
  return [pscustomobject]@{
    target = $Target
    status = 'unknown'
    reason = $Reason
    remediation = $Remediation
    report = $null
  }
}

function New-ReportTarget([string]$Target, $Report) {
  $reasons = @{
    ok = 'doctor_report_ok'
    attention = 'doctor_report_attention'
    problem = 'doctor_report_problem'
    incomplete = 'doctor_report_incomplete'
  }
  $remediations = @{
    ok = 'none'
    attention = 'review_installation'
    problem = 'review_installation'
    incomplete = 'review_installation'
  }
  return [pscustomobject]@{
    target = $Target
    status = [string]$Report.summary.status
    reason = $reasons[$Report.summary.status]
    remediation = $remediations[$Report.summary.status]
    report = $Report
  }
}

function Test-ProjectName([string]$Name) {
  return $Name -match '^[A-Za-z0-9_.-]{1,64}$' -and $Name -ne '.' -and $Name -ne '..'
}

function Test-LinuxPath([string]$Path) {
  return $Path -match '^/[^\x00-\x1f]*$' -and ($Path -split '/') -notcontains '..'
}

$targets = @()
try {
  if (-not $IsWindows) { throw 'platform' }
  if (-not $WslOnly) {
    if (-not [IO.Path]::IsPathFullyQualified($RuntimeDir) -or $RuntimeDir.Contains([char]0)) { throw 'runtime' }
    if ($Endpoint -notmatch '^[A-Za-z0-9_-]{1,48}$') { throw 'endpoint' }
  }
  if ($Project -and -not (Test-ProjectName $Project)) { throw 'project' }
  if ($VerifyWsl) {
    if ($Distribution -notmatch '^[A-Za-z0-9][A-Za-z0-9._ -]{0,63}$' -or
        $User -notmatch '^[A-Za-z0-9_][A-Za-z0-9_-]{0,63}$') { throw 'wsl identity' }
    if (-not (Test-LinuxPath $WslRuntimeDir) -or -not (Test-LinuxPath $WslEndpoint)) { throw 'wsl location' }
    if ($WslEndpoint -ne "$WslRuntimeDir/control.sock") { throw 'wsl endpoint' }
    if ($WslProject -and -not (Test-ProjectName $WslProject)) { throw 'wsl project' }
  }

  $root = (Resolve-Path -LiteralPath $PreceptDir).Path
  $rootItem = Get-Item -LiteralPath $root -Force
  if ($rootItem.LinkType -or -not $rootItem.PSIsContainer) { throw 'package root' }

  if (-not $WslOnly) {
    $executable = Join-Path $root 'bin/precept.exe'
    $executableItem = Get-Item -LiteralPath $executable -Force
    if ($executableItem.LinkType -or $executableItem.PSIsContainer) { throw 'native executable' }
    $arguments = @('--runtime-dir', $RuntimeDir, '--endpoint', $Endpoint, 'doctor', '--json')
    if ($Project) { $arguments += @('--project', $Project) }
    $invocation = Invoke-BoundedProcess $executable $arguments $root
    $safeReport = if ($invocation.Json) { ConvertTo-SafeDoctorReport $invocation.Json } else { $null }
    if ($null -ne $safeReport) {
      $targets += New-ReportTarget 'native' $safeReport
    } else {
      $reason = switch ($invocation.State) {
        'timeout' { 'native_doctor_timeout' }
        'limit' { 'native_doctor_response_too_large' }
        'unavailable' { 'native_doctor_unavailable' }
        default {
          if ([string]::IsNullOrWhiteSpace($invocation.Json)) { 'native_doctor_unavailable' }
          else { 'native_doctor_response_invalid' }
        }
      }
      $targets += New-FailedTarget 'native' $reason 'review_installation'
    }
  } else {
    $targets += New-NotRunTarget 'native' 'native_not_requested'
  }

  if (-not $VerifyWsl) {
    $targets += New-NotRunTarget 'wsl' 'wsl_not_requested'
  } else {
    try { $wsl = (Get-Command -Name 'wsl.exe' -CommandType Application).Source } catch { $wsl = $null }
    if (-not $wsl) {
      $targets += New-FailedTarget 'wsl' 'wsl_unavailable' 'review_wsl_configuration'
    } else {
      $arguments = @(
        '--distribution', $Distribution, '--user', $User, '--exec', 'precept',
        '--runtime-dir', $WslRuntimeDir, '--endpoint', $WslEndpoint, 'doctor', '--json'
      )
      if ($WslProject) { $arguments += @('--project', $WslProject) }
      $invocation = Invoke-BoundedProcess $wsl $arguments $root
      $safeReport = if ($invocation.Json) { ConvertTo-SafeDoctorReport $invocation.Json } else { $null }
      if ($null -ne $safeReport) {
        $targets += New-ReportTarget 'wsl' $safeReport
      } else {
        $reason = switch ($invocation.State) {
          'timeout' { 'wsl_doctor_timeout' }
          'limit' { 'wsl_doctor_response_too_large' }
          'unavailable' { 'wsl_doctor_unavailable' }
          default {
            if ([string]::IsNullOrWhiteSpace($invocation.Json)) { 'wsl_doctor_unavailable' }
            else { 'wsl_doctor_response_invalid' }
          }
        }
        $targets += New-FailedTarget 'wsl' $reason 'review_wsl_configuration'
      }
    }
  }
} catch {
  # Parameter and process exception text can contain paths or OS diagnostics.
  # Discard it and emit only fixed tokens.
  $targets = @(
    $(if ($WslOnly) { New-NotRunTarget 'native' 'native_not_requested' }
      else { New-FailedTarget 'native' 'verification_input_invalid' 'correct_arguments' }),
    $(if ($VerifyWsl) { New-FailedTarget 'wsl' 'verification_input_invalid' 'correct_arguments' }
      else { New-NotRunTarget 'wsl' 'wsl_not_requested' })
  )
}

$counts = @{ ok = 0; attention = 0; problem = 0; incomplete = 0; unknown = 0; not_run = 0 }
foreach ($target in $targets) { $counts[$target.status]++ }
$overall = if ($counts.problem -gt 0) { 'problem' }
  elseif ($counts.incomplete + $counts.unknown + $counts.not_run -gt 0) { 'incomplete' }
  elseif ($counts.attention -gt 0) { 'attention' }
  else { 'ok' }
$combined = [ordered]@{
  format = 'precept-doctor-verification'
  version = 1
  platform = 'windows'
  summary = [ordered]@{
    status = $overall
    targets = $targets.Count
    ok = $counts.ok
    attention = $counts.attention
    problem = $counts.problem
    incomplete = $counts.incomplete
    unknown = $counts.unknown
    not_run = $counts.not_run
  }
  targets = @($targets)
}
$combined | ConvertTo-Json -Depth 12 -Compress
if ($overall -eq 'problem') { exit 2 }
if ($overall -eq 'incomplete') { exit 3 }
if ($overall -eq 'attention') { exit 1 }
exit 0
