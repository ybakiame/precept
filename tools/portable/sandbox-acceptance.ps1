# Runs INSIDE a disposable Windows Sandbox (Windows PowerShell 5.1). Synthetic state only.
# Input  C:\acp-input  (read-only): the ZIP, vc_redist.x64.exe, expected.json, this script.
# Output C:\acp-output (writable):  result.json, log.txt, hwnd.png.
# Ends by shutting the sandbox down; nothing here touches the host.
$ErrorActionPreference = 'Stop'
$in = 'C:\acp-input'; $out = 'C:\acp-output'
$log = Join-Path $out 'log.txt'
$expected = Get-Content -LiteralPath (Join-Path $in 'expected.json') -Raw | ConvertFrom-Json
$cells = New-Object System.Collections.ArrayList
function Log([string]$m) { Add-Content -LiteralPath $log -Value ("{0:HH:mm:ss} {1}" -f (Get-Date), $m) }
function Cell([string]$id, [scriptblock]$body) {
    try { $detail = & $body; [void]$cells.Add([ordered]@{ id = $id; status = 'PASS'; detail = "$detail" }); Log "PASS $id $detail" }
    catch { [void]$cells.Add([ordered]@{ id = $id; status = 'FAIL'; detail = $_.Exception.Message }); Log "FAIL $id $($_.Exception.Message)" }
}
function Need([bool]$ok, [string]$m) { if (-not $ok) { throw $m } }
function Owner([string]$path) { $acl = Get-Acl -LiteralPath $path; "owner=$($acl.Owner) rules=" + (($acl.Access | ForEach-Object { "$($_.IdentityReference):$($_.FileSystemRights)" }) -join ';') }

Add-Type -AssemblyName System.Drawing
Add-Type -TypeDefinition @'
using System; using System.ComponentModel; using System.Runtime.InteropServices; using System.Text;
public static class AcpNative {
  [DllImport("kernel32.dll")] public static extern uint SetErrorMode(uint mode);
  [DllImport("kernel32.dll", SetLastError=true)] public static extern bool GenerateConsoleCtrlEvent(uint ev, uint group);
  [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  [StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left, Top, Right, Bottom; }
  [StructLayout(LayoutKind.Sequential, CharSet=CharSet.Unicode)] struct STARTUPINFO {
    public int cb; public string reserved, desktop, title; public int x, y, xs, ys, xc, yc, fill, flags;
    public short show, reserved2; public IntPtr reserved3, stdin, stdout, stderr; }
  [StructLayout(LayoutKind.Sequential)] struct PROCESS_INFORMATION { public IntPtr process, thread; public int pid, tid; }
  [DllImport("kernel32.dll", SetLastError=true, CharSet=CharSet.Unicode)]
  static extern bool CreateProcessW(string app, StringBuilder cmd, IntPtr pa, IntPtr ta, bool inherit, uint flags, IntPtr env, string cwd, ref STARTUPINFO si, out PROCESS_INFORMATION pi);
  [DllImport("kernel32.dll")] static extern bool CloseHandle(IntPtr h);
  [DllImport("kernel32.dll", SetLastError=true, CharSet=CharSet.Unicode)]
  static extern uint GetFinalPathNameByHandleW(IntPtr h, StringBuilder path, uint size, uint flags);
  // Same final path that Rust's std::fs::canonicalize returns on Windows (\\?\ form).
  public static string FinalPath(string path) {
    IntPtr h = CreateFileW(path, 0, 7, IntPtr.Zero, 3, 0x02000000, IntPtr.Zero);
    if (h == new IntPtr(-1)) throw new Win32Exception();
    var sb = new StringBuilder(1024); uint n = GetFinalPathNameByHandleW(h, sb, 1024, 0); CloseHandle(h);
    if (n == 0 || n >= 1024) throw new Win32Exception();
    return sb.ToString();
  }
  [DllImport("kernel32.dll", SetLastError=true)] static extern bool FreeConsole();
  [DllImport("kernel32.dll", SetLastError=true)] static extern bool AttachConsole(uint pid);
  [DllImport("kernel32.dll", SetLastError=true)] static extern bool SetConsoleCtrlHandler(IntPtr handler, bool add);
  // Equivalent of pressing Ctrl+C in the daemon's own console. This process ignores the
  // event and leaves its original console (later steps log to a file only).
  public static string CtrlCInConsoleOf(uint pid) {
    FreeConsole();
    if (!AttachConsole(pid)) return "AttachConsole error " + Marshal.GetLastWin32Error();
    SetConsoleCtrlHandler(IntPtr.Zero, true);
    if (!GenerateConsoleCtrlEvent(0, 0)) return "GenerateConsoleCtrlEvent error " + Marshal.GetLastWin32Error();
    return null;
  }
  [StructLayout(LayoutKind.Explicit, CharSet=CharSet.Unicode)] struct INPUT_RECORD {
    [FieldOffset(0)] public ushort EventType; [FieldOffset(4)] public int KeyDown; [FieldOffset(8)] public ushort Repeat;
    [FieldOffset(10)] public ushort VirtualKey; [FieldOffset(12)] public ushort ScanCode; [FieldOffset(14)] public char Char;
    [FieldOffset(16)] public uint ControlState; }
  [DllImport("kernel32.dll", SetLastError=true, CharSet=CharSet.Unicode)]
  static extern IntPtr CreateFileW(string name, uint access, uint share, IntPtr sa, uint disposition, uint flags, IntPtr template);
  [DllImport("kernel32.dll", SetLastError=true, CharSet=CharSet.Unicode)]
  static extern bool WriteConsoleInputW(IntPtr h, INPUT_RECORD[] records, uint count, out uint written);
  static INPUT_RECORD Key(bool down, ushort vk, char ch, uint state) {
    var r = new INPUT_RECORD(); r.EventType = 1; r.KeyDown = down ? 1 : 0; r.Repeat = 1; r.VirtualKey = vk; r.Char = ch; r.ControlState = state; return r; }
  // Types Ctrl+Q into this (shared) console's input buffer for the TUI that is reading it.
  public static void SendCtrlQ() {
    IntPtr h = CreateFileW("CONIN$", 0xC0000000, 3, IntPtr.Zero, 3, 0, IntPtr.Zero);
    if (h == new IntPtr(-1)) throw new Win32Exception();
    var records = new[] { Key(true, 0x11, '\0', 8), Key(true, 0x51, '\u0011', 8), Key(false, 0x51, '\u0011', 8), Key(false, 0x11, '\0', 0) };
    uint written; bool ok = WriteConsoleInputW(h, records, (uint)records.Length, out written); CloseHandle(h);
    if (!ok || written != records.Length) throw new Win32Exception();
  }
}
'@
[void][AcpNative]::SetProcessDPIAware()
# Loader failures must return an exit code, never block on a hard-error dialog.
[void][AcpNative]::SetErrorMode(0x8003)

function Run([string]$exe, [string[]]$arguments, [int]$seconds = 15) {
    $psi = New-Object Diagnostics.ProcessStartInfo $exe
    $psi.Arguments = ($arguments | ForEach-Object { if ($_ -match '[\s"]') { '"' + $_.Replace('"', '\"') + '"' } else { $_ } }) -join ' '
    $psi.UseShellExecute = $false; $psi.CreateNoWindow = $true
    $psi.RedirectStandardOutput = $true; $psi.RedirectStandardError = $true
    $p = [Diagnostics.Process]::Start($psi)
    $stdout = $p.StandardOutput.ReadToEndAsync(); $stderr = $p.StandardError.ReadToEndAsync()
    if (-not $p.WaitForExit($seconds * 1000)) { $p.Kill(); throw "timeout: $(Split-Path -Leaf $exe)" }
    $p.WaitForExit()
    $json = $null; try { $json = $stdout.Result | ConvertFrom-Json } catch { }
    [pscustomobject]@{ code = $p.ExitCode; json = $json; err = $stderr.Result }
}
function VcModules([Diagnostics.Process]$p) {
    $p.Refresh()
    @($p.Modules | Where-Object { $_.ModuleName -match '^vcruntime140' } | ForEach-Object {
        $where = if ($_.FileName.StartsWith("$env:WINDIR\System32\", [StringComparison]::OrdinalIgnoreCase)) { 'System32' } else { 'OTHER' }
        "$($_.ModuleName)@$where" })
}

$os = Get-CimInstance Win32_OperatingSystem
$result = [ordered]@{ environment = [ordered]@{ kind = 'Windows Sandbox (disposable, networking disabled)'; os = $os.Caption; build = $os.BuildNumber; user = $env:USERNAME
    gpu = @(Get-CimInstance Win32_VideoController | ForEach-Object { $_.Name }) }; cells = $cells }
Log "start build=$($os.BuildNumber)"
$product = Join-Path $env:LOCALAPPDATA 'acp-product'
$state = Join-Path $env:LOCALAPPDATA 'acp-sandbox-state'
$acp = Join-Path $product 'ACP'; $bin = Join-Path $acp 'bin'
$manifest = $null; $daemonPid = 0; $daemon = $null

Cell 'clean-target-has-no-vc-runtime' {
    $key = Get-ItemProperty 'HKLM:\SOFTWARE\Microsoft\VisualStudio\14.0\VC\Runtimes\x64' -ErrorAction SilentlyContinue
    Need ($null -eq $key) 'VC++ v14 runtime registry key already present'
    Need (-not (Test-Path "$env:WINDIR\System32\vcruntime140.dll")) 'vcruntime140.dll already in System32'
    'registry key and System32 DLL absent'
}
Cell 'zip-hash-extract-new-dir-manifest-verify' {
    $zip = Get-ChildItem -LiteralPath $in -Filter 'acp-windows-x86_64-*.zip'
    Need (@($zip).Count -eq 1) 'expected one ZIP'
    Need ((Get-FileHash -LiteralPath $zip.FullName -Algorithm SHA256).Hash.ToLowerInvariant() -eq $expected.archive_sha256) 'ZIP hash mismatch'
    Need (-not (Test-Path $product)) 'extraction collision'
    # .NET extraction: the Archive module is not loadable in every sandbox image.
    Add-Type -AssemblyName System.IO.Compression.FileSystem
    $archive = [IO.Compression.ZipFile]::OpenRead($zip.FullName)
    try { foreach ($e in $archive.Entries) { Need ($e.FullName -cmatch '^ACP/([A-Za-z0-9][A-Za-z0-9._-]*/)*[A-Za-z0-9][A-Za-z0-9._-]*$' -and -not $e.FullName.Contains('..')) "unsafe entry $($e.FullName)" } }
    finally { $archive.Dispose() }
    [IO.Compression.ZipFile]::ExtractToDirectory($zip.FullName, $product)
    Need (((@(Get-ChildItem -LiteralPath $product -Force) | ForEach-Object { $_.Name }) -join ',') -eq 'ACP') 'archive must create only ACP\'
    $script:manifest = Get-Content -LiteralPath (Join-Path $acp 'runtime\manifest.json') -Raw | ConvertFrom-Json
    foreach ($f in $manifest.files) {
        Need ((Get-FileHash -LiteralPath (Join-Path $acp $f.file) -Algorithm SHA256).Hash.ToLowerInvariant() -eq $f.sha256) "hash mismatch $($f.file)"
    }
    Need (@(Get-ChildItem -LiteralPath $bin -File).Count -eq 5) 'bin must hold exactly five executables'
    "$(@($manifest.files).Count) files verified"
}
Cell 'missing-prerequisite-fails-to-load-all-five' {
    $codes = foreach ($b in $manifest.binaries) {
        $exe = Join-Path $acp $b.file
        # Direct CreateProcess so the no-dialog error mode is inherited.
        $psi = New-Object Diagnostics.ProcessStartInfo -ArgumentList $exe
        $psi.Arguments = '--help'
        $psi.UseShellExecute = $false; $psi.CreateNoWindow = $true
        $p = [Diagnostics.Process]::Start($psi)
        if (-not $p.WaitForExit(15000)) { $p.Kill(); throw "$($b.file) did not exit" }
        Need ($p.ExitCode -eq -1073741515) "$($b.file) exit $($p.ExitCode), expected STATUS_DLL_NOT_FOUND"
        "$(Split-Path -Leaf $b.file)=0xC0000135"
    }
    Need (@($codes).Count -eq 5) "checked $(@($codes).Count) of 5 executables"
    $codes -join ' '
}
Cell 'install-vc-redist-inside-sandbox-only' {
    $setup = Join-Path $in 'vc_redist.x64.exe'
    Need ((Get-FileHash -LiteralPath $setup -Algorithm SHA256).Hash.ToLowerInvariant() -eq $expected.vc_redist_sha256) 'installer hash mismatch'
    $p = Start-Process -FilePath $setup -ArgumentList '/install', '/quiet', '/norestart' -PassThru -Wait
    Need ($p.ExitCode -in 0, 3010) "installer exit $($p.ExitCode)"
    $v = (Get-ItemProperty 'HKLM:\SOFTWARE\Microsoft\VisualStudio\14.0\VC\Runtimes\x64').Version
    $installed = [version]($v.TrimStart('v') -replace '\.00$', '')
    $minimum = [version]$manifest.prerequisites[0].minimum_version
    Need ($installed -ge $minimum) "installed $installed below $minimum"
    "installed $v >= $minimum (exit $($p.ExitCode))"
}

$setupFailed = @($cells | Where-Object { $_.status -ne 'PASS' }).Count -ne 0
if ($setupFailed) { Log 'setup failed; functional cells NOT_RUN' }
else {
# Exactly the README "private state directory" snippet: owner = this user, one ACE.
$d = New-Item -ItemType Directory -Path $state
$me = [Security.Principal.WindowsIdentity]::GetCurrent().User
$acl = New-Object Security.AccessControl.DirectorySecurity
$acl.SetOwner($me); $acl.SetAccessRuleProtection($true, $false)
$acl.AddAccessRule((New-Object Security.AccessControl.FileSystemAccessRule($me, 'FullControl', 'ContainerInherit,ObjectInherit', 'None', 'Allow')))
Set-Acl -LiteralPath $d.FullName -AclObject $acl
Log "state $(Owner $state)"
$synthetic = Join-Path $state 'synthetic'
New-Item -ItemType Directory -Path $synthetic | Out-Null
Set-Content -LiteralPath (Join-Path $synthetic 'AGENTS.md') -Value 'Synthetic sandbox acceptance only.'
Set-Content -LiteralPath (Join-Path $state 'unknown-retain.txt') -Value 'SYNTHETIC RETAIN'
$runtime = Join-Path $state 'runtime'
$endpoint = 'acp-sandbox-' + [guid]::NewGuid().ToString('N').Substring(0, 12)
$loc = @('--runtime-dir', $runtime, '--endpoint', $endpoint)
$query = Join-Path $bin 'acp-query.exe'
function Q([string[]]$a) { Run $query ($loc + $a) }
function Why($r) { "exit $($r.code) error=$($r.json.error) stderr=$(($r.err -split "`n" | Select-Object -First 2) -join ' | ')" }


Cell 'daemon-start-no-implicit-profile' {
    # Its OWN console window, as README instructs; the Process object keeps the handle.
    $script:daemon = Start-Process -FilePath (Join-Path $bin 'acp-daemon.exe') -PassThru -ArgumentList @('--runtime-dir', ('"' + $runtime + '"'), '--endpoint', $endpoint, '--allow-project', 'synthetic', '--allow-root', ('"synthetic=' + $synthetic + '"'))
    $script:daemonPid = $daemon.Id
    $deadline = (Get-Date).AddSeconds(20)
    do { Start-Sleep -Milliseconds 200; $info = Q @('profile', 'info') } while ($info.code -ne 0 -and (Get-Date) -lt $deadline)
    Need ($info.code -eq 0) 'daemon not ready'
    Need ($info.json.selected -eq $false -and $info.json.write_enabled -eq $false) 'implicit Profile selection or write grant'
    Need ((Q @('health')).code -eq 0) 'health failed'
    "pid $daemonPid; selected=false write_enabled=false; health ok; $((VcModules (Get-Process -Id $daemonPid)) -join ',')"
}
$db = Join-Path $state 'profile.sqlite3'
Cell 'explicit-synthetic-profile-grant-register' {
    $r = Q @('profile', 'create', $db)
    Need ($r.code -eq 0) "profile create failed: $(Why $r); state $(Owner $state)"
    Need ((Q @('profile', 'create', $db)).code -ne 0) 'repeat create must refuse'
    $b = (Q @('profile', 'info')).json.binding
    $r = Q @('profile', 'write', 'enable', '--profile-id', $b.profile_id, '--generation', $b.generation, '--confirm')
    Need ($r.code -eq 0) "write enable failed: $(Why $r)"
    # The mutation binds to the CURRENT binding, read after the write grant (as the host smoke does).
    $before = "$($b.profile_id)/$($b.generation)/$($b.owner_epoch)"
    $b = (Q @('profile', 'info')).json.binding
    $op = -join ((1..16) | ForEach-Object { '{0:x2}' -f (Get-Random -Maximum 256) })
    $canonical = [AcpNative]::FinalPath($synthetic)
    $r = Q @('project-register', 'synthetic', $canonical, '--profile-id', $b.profile_id, '--generation', $b.generation, '--owner-epoch', "$($b.owner_epoch)", '--operation-id', $op)
    Need ($r.code -eq 0) "register failed: $(Why $r); binding before=$before after=$($b.profile_id)/$($b.generation)/$($b.owner_epoch); root=$canonical"
    Need (Test-Path (Join-Path $runtime 'agent-synthetic.cap')) 'agent credential missing'
    "created, repeat refused, write enabled, registered $canonical"
}
Cell 'mcp-exactly-two-read-tools-and-context' {
    $psi = New-Object Diagnostics.ProcessStartInfo (Join-Path $bin 'acp-mcp.exe')
    $psi.Arguments = '--runtime-dir "' + $runtime + '" --endpoint ' + $endpoint + ' --project synthetic --capability-file "' + (Join-Path $runtime 'agent-synthetic.cap') + '"'
    $psi.UseShellExecute = $false; $psi.CreateNoWindow = $true; $psi.RedirectStandardInput = $true; $psi.RedirectStandardOutput = $true; $psi.RedirectStandardError = $true
    $p = [Diagnostics.Process]::Start($psi); $mcpErr = $p.StandardError.ReadToEndAsync()
    $meta = @{ 'io.modelcontextprotocol/protocolVersion' = '2026-07-28' }
    function Ask($method, $params) {
        $params['_meta'] = $meta
        # Raw BOM-less UTF-8: the .NET Framework stdin writer would prepend a BOM.
        $bytes = (New-Object Text.UTF8Encoding $false).GetBytes((@{ jsonrpc = '2.0'; id = 1; method = $method; params = $params } | ConvertTo-Json -Compress -Depth 6) + "`n")
        $p.StandardInput.BaseStream.Write($bytes, 0, $bytes.Length); $p.StandardInput.BaseStream.Flush()
        $t = $p.StandardOutput.ReadLineAsync(); Need ($t.Wait(10000)) "MCP $method timeout"
        if ($null -eq $t.Result) { Start-Sleep -Milliseconds 500; throw "MCP closed stdout on $method; exited=$($p.HasExited) stderr=$($mcpErr.Result)" }
        Log "MCP $method -> $($t.Result.Substring(0, [Math]::Min(300, $t.Result.Length)))"
        $t.Result | ConvertFrom-Json
    }
    $tools = @((Ask 'tools/list' @{}).result.tools | ForEach-Object { $_.name })
    Need (($tools -join ',') -eq 'get_effective_context,get_handoff') "tools: $($tools -join ',')"
    Need ((Ask 'tools/call' @{ name = 'get_effective_context'; arguments = @{} }).result.isError -eq $false) 'context failed'
    Need ($null -ne (Ask 'tools/call' @{ name = 'confirm_preference'; arguments = @{} }).error) 'write tool not refused'
    $p.StandardInput.Close(); if (-not $p.WaitForExit(10000)) { $p.Kill(); throw 'MCP did not exit on EOF' }
    "tools=$($tools -join ','); context ok; write tool refused"
}
Cell 'write-grant-revoked' {
    Need ((Q @('profile', 'write', 'disable')).code -eq 0) 'disable failed'
    Need ((Q @('profile', 'info')).json.write_enabled -eq $false) 'still write-enabled'
    'write_enabled=false'
}
Cell 'hwnd-desktop-render-close-daemon-survives' {
    $psi = New-Object Diagnostics.ProcessStartInfo (Join-Path $bin 'acp-nsh-native.exe')
    $psi.Arguments = '--runtime-dir "' + $runtime + '" --endpoint ' + $endpoint + ' --operator'
    $psi.UseShellExecute = $false
    $p = [Diagnostics.Process]::Start($psi)
    $deadline = (Get-Date).AddSeconds(30)
    do { Start-Sleep -Milliseconds 250; $p.Refresh() } while ($p.MainWindowHandle -eq [IntPtr]::Zero -and -not $p.HasExited -and (Get-Date) -lt $deadline)
    Need (-not $p.HasExited -and $p.MainWindowHandle -ne [IntPtr]::Zero) "no window (exited=$($p.HasExited))"
    Start-Sleep -Seconds 4
    [void][AcpNative]::SetForegroundWindow($p.MainWindowHandle); Start-Sleep -Milliseconds 500
    $r = New-Object AcpNative+RECT; [void][AcpNative]::GetWindowRect($p.MainWindowHandle, [ref]$r)
    $bmp = New-Object Drawing.Bitmap ($r.Right - $r.Left), ($r.Bottom - $r.Top)
    $g = [Drawing.Graphics]::FromImage($bmp); $g.CopyFromScreen($r.Left, $r.Top, 0, 0, $bmp.Size); $g.Dispose()
    $bmp.Save((Join-Path $out 'hwnd.png'), [Drawing.Imaging.ImageFormat]::Png); $bmp.Dispose()
    $vc = VcModules $p
    Need (($vc -join ',') -eq 'VCRUNTIME140.dll@System32,VCRUNTIME140_1.dll@System32') "VC modules: $($vc -join ',')"
    $title = $p.MainWindowTitle
    [void]$p.CloseMainWindow()
    Need ($p.WaitForExit(15000)) 'window close did not exit'
    Need ($p.ExitCode -eq 0) "exit $($p.ExitCode)"
    Need ((Q @('health')).code -eq 0) 'daemon stopped with UI'
    "title='$title'; $($vc -join ','); normal close exit 0; daemon healthy"
}
Cell 'conpty-tui-render-ctrl-q-exit' {
    # Shares this console: types Ctrl+Q into the input buffer the TUI reads.
    $psi = New-Object Diagnostics.ProcessStartInfo (Join-Path $bin 'acp-nsh-tui.exe')
    $psi.Arguments = '--runtime-dir "' + $runtime + '" --endpoint ' + $endpoint + ' --operator'
    $psi.UseShellExecute = $false
    $p = [Diagnostics.Process]::Start($psi)
    Start-Sleep -Seconds 5
    Need (-not $p.HasExited) "TUI exited early ($($p.ExitCode))"
    $vc = VcModules $p
    Need (($vc -join ',') -eq 'VCRUNTIME140.dll@System32') "VC modules: $($vc -join ',')"
    [AcpNative]::SendCtrlQ()
    Need ($p.WaitForExit(10000)) 'TUI did not exit on Ctrl+Q'
    Need ($p.ExitCode -eq 0) "exit $($p.ExitCode)"
    "$($vc -join ','); Ctrl+Q exit 0"
}
Cell 'graceful-stop-cleans-credentials' {
    Need ($daemonPid -ne 0) 'no daemon'
    $p = $daemon
    $err = [AcpNative]::CtrlCInConsoleOf([uint32]$daemonPid)
    Need ($null -eq $err) "Ctrl+C failed: $err"
    Need ($p.WaitForExit(15000)) 'daemon did not exit'
    Need ($p.ExitCode -eq 0) "daemon exit $($p.ExitCode)"
    foreach ($cap in 'operator.cap', 'agent-synthetic.cap', 'operator-write.cap') { Need (-not (Test-Path (Join-Path $runtime $cap))) "$cap left behind" }
    Need ((Q @('health')).code -ne 0) 'endpoint still answers'
    $script:daemonPid = 0
    'exited; capabilities removed; endpoint closed'
}
Cell 'exact-removal-retains-state-and-unknown' {
    $before = (Get-FileHash -LiteralPath $db -Algorithm SHA256).Hash
    Set-Content -LiteralPath (Join-Path $bin 'unknown-retain.txt') -Value 'SYNTHETIC RETAIN'
    $list = @($manifest.files | ForEach-Object { $_ }) + @([pscustomobject]@{ file = 'runtime/manifest.json'; sha256 = (Get-FileHash -LiteralPath (Join-Path $acp 'runtime\manifest.json') -Algorithm SHA256).Hash.ToLowerInvariant() })
    foreach ($f in $list) {
        $path = Join-Path $acp $f.file
        $item = Get-Item -LiteralPath $path -Force
        Need (-not ($item.Attributes -band [IO.FileAttributes]::ReparsePoint)) "alias $($f.file)"
        Need ((Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash.ToLowerInvariant() -eq $f.sha256) "changed $($f.file)"
    }
    foreach ($f in $list) { Remove-Item -LiteralPath (Join-Path $acp $f.file) -Force }
    foreach ($d in 'runtime', 'notices', 'bin', '.') { $dir = Join-Path $acp $d; if (@(Get-ChildItem -LiteralPath $dir -Force).Count -eq 0) { [IO.Directory]::Delete($dir) } }
    Need (-not (Test-Path (Join-Path $acp 'runtime')) -and -not (Test-Path (Join-Path $acp 'notices'))) 'empty product dirs remain'
    Need ((Get-Content -LiteralPath (Join-Path $bin 'unknown-retain.txt')) -eq 'SYNTHETIC RETAIN') 'unknown product-dir file lost'
    Need ((Get-FileHash -LiteralPath $db -Algorithm SHA256).Hash -eq $before) 'Profile changed'
    Need ((Get-Content -LiteralPath (Join-Path $state 'unknown-retain.txt')) -eq 'SYNTHETIC RETAIN') 'state unknown file lost'
    "$($list.Count) files removed; bin kept for unknown file; Profile/state retained"
}

}
if ($daemonPid -ne 0) { try { Stop-Process -Id $daemonPid -Force; Log 'forced daemon cleanup after failure' } catch { } }
$result.setup_failed = $setupFailed
$result.passed = @($cells | Where-Object { $_.status -eq 'PASS' }).Count
$result.failed = @($cells | Where-Object { $_.status -ne 'PASS' }).Count
$result | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath (Join-Path $out 'result.json') -Encoding UTF8
Log "done passed=$($result.passed) failed=$($result.failed)"
Stop-Computer -Force
