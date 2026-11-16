# Fixed synthetic S5 window driver. Never changes foreground or captures desktop.
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing
Add-Type -TypeDefinition @'
using System;
using System.Text;
using System.Runtime.InteropServices;
public static class AcpWindow {
 public delegate bool EnumProc(IntPtr h, IntPtr p);
 [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc f, IntPtr p);
 [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetWindowText(IntPtr h, StringBuilder s, int n);
 [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
 [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint p);
 [DllImport("user32.dll")] public static extern IntPtr GetKeyboardLayout(uint thread);
 [DllImport("user32.dll")] public static extern int GetKeyboardLayoutList(int count, [Out] IntPtr[] layouts);
 [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
 [DllImport("user32.dll")] public static extern void keybd_event(byte k, byte s, uint f, UIntPtr e);
 [DllImport("user32.dll", EntryPoint="MapVirtualKeyW")] public static extern uint MapVirtualKey(uint code, uint mapType);
 public static void SendKey(byte key, bool released) {
  // WSLg/RDP forwards scan codes: zero loses key identity, and E0 distinguishes
  // dedicated navigation keys from the numeric keypad.
  uint scan = MapVirtualKey(key, 0); // MAPVK_VK_TO_VSC
  // MapVirtualKey does not retain E0 for these generic navigation VKs.
  bool extended = key == 0x23 || key == 0x24 || key == 0x25 || key == 0x27;
  uint flags = (extended ? 1u : 0u) | (released ? 2u : 0u);
  keybd_event(key, (byte)(scan & 0xff), flags, UIntPtr.Zero);
 }
 [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern short VkKeyScan(char c);
 [StructLayout(LayoutKind.Sequential)] public struct RECT { public int left,top,right,bottom; }
 [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
 [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr h, IntPtr dc, uint flags);
 [DllImport("dwmapi.dll")] public static extern int DwmFlush();
 public static bool HasColorVariation(IntPtr pixels, int stride, int width, int height) {
  int first = Marshal.ReadInt32(pixels) & 0x00ffffff;
  for (int y = 0; y < height; y++)
   for (int x = 0; x < width; x++)
    if ((Marshal.ReadInt32(pixels, y * stride + x * 4) & 0x00ffffff) != first) return true;
  return false;
 }
 [DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr h, IntPtr after, int x,int y,int w,int z,uint flags);
 [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr h,uint m,IntPtr w,IntPtr l);
 [StructLayout(LayoutKind.Sequential)] public struct POINT { public int x,y; }
 [DllImport("user32.dll")] public static extern bool GetCursorPos(out POINT p);
 [DllImport("user32.dll")] public static extern bool SetCursorPos(int x,int y);
 [DllImport("user32.dll")] public static extern IntPtr WindowFromPoint(POINT p);
 [DllImport("user32.dll")] public static extern IntPtr GetAncestor(IntPtr h,uint flags);
 [DllImport("user32.dll")] public static extern void mouse_event(uint flags,uint dx,uint dy,uint data,UIntPtr extra);
 [StructLayout(LayoutKind.Sequential)] public struct MONITORINFO { public uint size; public RECT monitor,work; public uint flags; }
 [DllImport("user32.dll")] public static extern IntPtr MonitorFromWindow(IntPtr h,uint flags);
 [DllImport("user32.dll", EntryPoint="GetMonitorInfoW")] public static extern bool GetMonitorInfo(IntPtr h,ref MONITORINFO info);
 public static RECT WorkArea(IntPtr window) {
  var info = new MONITORINFO(); info.size = (uint)Marshal.SizeOf(typeof(MONITORINFO));
  if (!GetMonitorInfo(MonitorFromWindow(window,2),ref info)) throw new InvalidOperationException("Owned monitor unavailable");
  return info.work;
 }
}
'@
$nonce = $null
$expected = 0
$remote = $false
$images = $null
$originalLayout = [IntPtr]::Zero
$layoutWindow = [IntPtr]::Zero
function Owned-Window {
    $script:ownedHandles = [System.Collections.Generic.List[IntPtr]]::new()
    [AcpWindow]::EnumWindows({ param($h,$unused)
        $text = [System.Text.StringBuilder]::new(512)
        [void][AcpWindow]::GetWindowText($h,$text,512)
        $title = $text.ToString()
        $owned = $title.StartsWith('Agent Control Plane ', [StringComparison]::Ordinal) -and $(if ($remote) { $title.Contains(" [$nonce]") } else { $title.EndsWith(" [$nonce]", [StringComparison]::Ordinal) })
        if ([AcpWindow]::IsWindowVisible($h) -and $owned) { $script:ownedHandles.Add($h) }
        return $true
    },[IntPtr]::Zero) | Out-Null
    if ($ownedHandles.Count -ne 1) { throw 'Owned nonce window unavailable or ambiguous' }
    $h = $ownedHandles[0]
    [uint32]$ownerId = 0
    [void][AcpWindow]::GetWindowThreadProcessId($h,[ref]$ownerId)
    if ($remote) {
        if ((Get-Process -Id $ownerId).ProcessName -ne 'msrdc') { throw 'Unexpected WSLg host process' }
    } elseif ($ownerId -ne $expected) { throw 'Window process ownership changed' }
    return $h
}
function Ensure-English([IntPtr]$Window) {
    if ($remote) { return $null }
    [uint32]$ownerId=0
    $thread=[AcpWindow]::GetWindowThreadProcessId($Window,[ref]$ownerId)
    $current=[AcpWindow]::GetKeyboardLayout($thread)
    $switched=$false
    if (($current.ToInt64() -band 0x3ff) -ne 9) {
        $count=[AcpWindow]::GetKeyboardLayoutList(0,$null)
        if ($count -lt 1 -or $count -gt 64) { throw 'Installed keyboard layout list unavailable' }
        $layouts=[IntPtr[]]::new($count)
        [void][AcpWindow]::GetKeyboardLayoutList($count,$layouts)
        $english=@($layouts | Where-Object { ($_.ToInt64() -band 0x3ff) -eq 9 })
        if ($english.Count -eq 0) { throw 'An already-installed English layout is required; none will be installed' }
        if ($originalLayout -eq [IntPtr]::Zero) { $script:originalLayout=$current; $script:layoutWindow=$Window }
        if (-not [AcpWindow]::PostMessage($Window,0x50,[IntPtr]::Zero,$english[0])) { throw 'Owned window input-layout request refused' }
        $deadline=[DateTime]::UtcNow.AddSeconds(3)
        while (([AcpWindow]::GetKeyboardLayout($thread).ToInt64() -band 0x3ff) -ne 9) {
            if ([DateTime]::UtcNow -gt $deadline) { throw 'Owned window did not acknowledge English layout' }
            Start-Sleep -Milliseconds 20
        }
        $switched=$true
    }
    if ((Owned-Window) -ne $Window -or [AcpWindow]::GetForegroundWindow() -ne $Window) { throw 'Foreground changed during input-layout check' }
    return @{english=$true;switched=$switched}
}
function Invoke-OwnedDrag([IntPtr]$Window, [AcpWindow+POINT]$Point, [int]$Dx, [int]$Dy) {
    $saved=[AcpWindow+POINT]::new()
    if (-not [AcpWindow]::GetCursorPos([ref]$saved)) { throw 'Pointer unavailable' }
    $pressed=$false
    try {
        if (-not [AcpWindow]::SetCursorPos($Point.x,$Point.y)) { throw 'Owned pointer move refused' }
        if ((Owned-Window) -ne $Window -or [AcpWindow]::GetForegroundWindow() -ne $Window) { throw 'Foreground changed before drag' }
        if ([AcpWindow]::GetAncestor([AcpWindow]::WindowFromPoint($Point),2) -ne $Window) { throw 'Owned drag target is occluded' }
        [AcpWindow]::mouse_event(2,0,0,0,[UIntPtr]::Zero); $pressed=$true
        Start-Sleep -Milliseconds 100
        for($step=1; $step -le 12; $step++) {
            if ((Owned-Window) -ne $Window -or [AcpWindow]::GetForegroundWindow() -ne $Window) { throw 'Foreground changed during drag' }
            if (-not [AcpWindow]::SetCursorPos(($Point.x+[int]($Dx*$step/12)),($Point.y+[int]($Dy*$step/12)))) { throw 'Owned drag refused' }
            Start-Sleep -Milliseconds 20
        }
        Start-Sleep -Milliseconds 100
    } finally {
        if ($pressed) { [AcpWindow]::mouse_event(4,0,0,0,[UIntPtr]::Zero); Start-Sleep -Milliseconds 100 }
        [void][AcpWindow]::SetCursorPos($saved.x,$saved.y)
    }
}
try {
:requests while ($null -ne ($line = [Console]::ReadLine())) {
    # Every reply must name the request it answers; the driver refuses otherwise.
    $rid = $null
    try {
        $command = $line | ConvertFrom-Json
        $rid = $command.rpc_id
        if ($command.action -eq 'init') {
            if ($nonce -or $command.nonce -notmatch '^[0-9a-f]{16}$') { throw 'Invalid one-time nonce' }
            $nonce = $command.nonce; $expected = [uint32]$command.pid; $remote = [bool]$command.remote
            if (-not $remote -and $expected -eq 0) { throw 'Explicit native PID required' }
            $images = Join-Path (Split-Path -Parent $PSScriptRoot) "artifacts/local-p1-m2/s5-window-$nonce"
            if (Test-Path -LiteralPath $images) { throw 'Window capture directory must be new' }
            New-Item -ItemType Directory -Path $images | Out-Null
            @{status='ok';rpc_id=$rid;images=$images} | ConvertTo-Json -Compress
            continue
        }
        if (-not $nonce) { throw 'Initialize owned window first' }
        $h = Owned-Window
        $active = [AcpWindow]::GetForegroundWindow() -eq $h
        if ($command.action -eq 'observe') { @{status='ok';rpc_id=$rid;foreground=$active} | ConvertTo-Json -Compress; continue }
        if ($command.action -in @('key','text','capture','resize') -and -not $active) { @{status='foreground_required';rpc_id=$rid;nonce=$nonce} | ConvertTo-Json -Compress; continue }
        $layout=$null
        if ($command.action -in @('key','text')) { $layout=Ensure-English $h }
        switch ($command.action) {
            'key' {
                if ($command.key -notmatch '^(alt\+shift\+[umerxlsvynzihwq]|alt\+[opkwft1-6abc]|alt\+(right|left)|ctrl\+[alrpq]|escape|enter|space|tab|right|left|home|end)$') { throw 'Key outside fixed synthetic vocabulary' }
                $parts = $command.key -split '\+'
                $last = $parts[-1]
                $code = switch ($last) { 'escape' {27}; 'tab' {9}; 'enter' {13}; 'space' {32}; 'right' {39}; 'left' {37}; 'home' {36}; 'end' {35}; default { if ($last -notmatch '^[a-z0-9]$') { throw 'Unsupported synthetic key' }; [int][char]$last.ToUpperInvariant() } }
                $mods = @(); if ($parts -contains 'ctrl') {$mods += 17}; if ($parts -contains 'alt') {$mods += 18}; if ($parts -contains 'shift') {$mods += 16}
                if ($command.key -eq 'ctrl+1' -or ($parts -contains 'win')) { throw 'Reserved key refused' }
                try { foreach($m in $mods) { [AcpWindow]::SendKey($m,$false) }; [AcpWindow]::SendKey($code,$false); [AcpWindow]::SendKey($code,$true) }
                finally { [array]::Reverse($mods); foreach($m in $mods) { [AcpWindow]::SendKey($m,$true) } }
            }
            'text' {
                if ($command.text.Length -gt 1024 -or $command.text.Contains("`n") -or $command.text.Contains("`r")) { throw 'Synthetic field bound exceeded' }
                foreach($c in $command.text.ToCharArray()) {
                    $current = Owned-Window; if ($current -ne $h -or [AcpWindow]::GetForegroundWindow() -ne $h) { throw 'Foreground changed during native input' }
                    $scan = [int][AcpWindow]::VkKeyScan($c)
                    if ([int]$c -lt 32 -or [int]$c -gt 126 -or $scan -lt 0 -or ($scan -shr 8) -gt 1) { throw 'Only fixed ASCII synthetic fields supported' }
                    $shift = ($scan -band 256) -ne 0
                    try { if ($shift) { [AcpWindow]::SendKey(16,$false) }; [AcpWindow]::SendKey(($scan -band 255),$false); [AcpWindow]::SendKey(($scan -band 255),$true) }
                    finally { if ($shift) { [AcpWindow]::SendKey(16,$true) } }
                    # Pace synthetic text across UI frames; do not resend input.
                    Start-Sleep -Milliseconds 25
                }
            }
            'resize' {
                $w=[int]$command.width;$height=[int]$command.height
                if ($w -lt 640 -or $w -gt 1600 -or $height -lt 480 -or $height -gt 1000) { throw 'Synthetic resize out of bounds' }
                if ($remote) {
                    # SetWindowPos resizes only WSLg's RDP host, not the GTK surface.
                    # Drive the app's native bottom-right resize grip instead.
                    $rect=[AcpWindow+RECT]::new()
                    if (-not [AcpWindow]::GetWindowRect($h,[ref]$rect)) { throw 'Owned bounds unavailable' }
                    $work=[AcpWindow]::WorkArea($h)
                    if ($w -gt ($work.right-$work.left-16) -or $height -gt ($work.bottom-$work.top-16)) { throw 'Owned monitor cannot fit required viewport' }
                    $left=[Math]::Min([Math]::Max($rect.left,$work.left+8),$work.right-$w-8)
                    $top=[Math]::Min([Math]::Max($rect.top,$work.top+8),$work.bottom-$height-8)
                    if ($left -ne $rect.left -or $top -ne $rect.top) {
                        # Move via the owned app title bar so the required grip
                        # destination is not silently clamped by the monitor edge.
                        $point=[AcpWindow+POINT]::new(); $point.x=$rect.left+80; $point.y=$rect.top+18
                        Invoke-OwnedDrag $h $point ($left-$rect.left) ($top-$rect.top)
                        if (-not [AcpWindow]::GetWindowRect($h,[ref]$rect)) { throw 'Owned moved bounds unavailable' }
                    }
                    $dw=$w-($rect.right-$rect.left); $dh=$height-($rect.bottom-$rect.top)
                    if ($dw -ne 0 -or $dh -ne 0) {
                        $point=[AcpWindow+POINT]::new(); $point.x=$rect.right-4; $point.y=$rect.bottom-4
                        Invoke-OwnedDrag $h $point $dw $dh
                    }
                } elseif (-not [AcpWindow]::SetWindowPos($h,[IntPtr]::Zero,0,0,$w,$height,0x16)) { throw 'Owned resize refused' }
            }
            'capture' {
                if ($command.name -notmatch '^[a-z0-9-]+\.png$') { throw 'Invalid capture name' }
                $rect=[AcpWindow+RECT]::new(); if (-not [AcpWindow]::GetWindowRect($h,[ref]$rect)) {throw 'Owned bounds unavailable'}
                $w=$rect.right-$rect.left;$height=$rect.bottom-$rect.top
                if ($w -lt 100 -or $w -gt 2000 -or $height -lt 100 -or $height -gt 1600) {throw 'Capture bounds refused'}
                $bitmap=[Drawing.Bitmap]::new($w,$height)
                try {
                    $graphics=[Drawing.Graphics]::FromImage($bitmap)
                    try {
                        $clock=[Diagnostics.Stopwatch]::StartNew();$attempts=0
                        do {
                            if ((Owned-Window) -ne $h -or [AcpWindow]::GetForegroundWindow() -ne $h) {throw 'Owned capture lost foreground'}
                            $now=[AcpWindow+RECT]::new()
                            if (-not [AcpWindow]::GetWindowRect($h,[ref]$now) -or $now.right-$now.left -ne $w -or $now.bottom-$now.top -ne $height) {throw 'Owned capture bounds changed'}
                            if ([AcpWindow]::DwmFlush() -ne 0) {throw 'Owned composition unavailable'}
                            $graphics.Clear([Drawing.Color]::Black);$dc=$graphics.GetHdc()
                            try { if (-not [AcpWindow]::PrintWindow($h,$dc,2)) {throw 'Owned render capture failed'} }
                            finally {$graphics.ReleaseHdc($dc)}
                            $data=$bitmap.LockBits([Drawing.Rectangle]::new(0,0,$w,$height),[Drawing.Imaging.ImageLockMode]::ReadOnly,[Drawing.Imaging.PixelFormat]::Format32bppArgb)
                            try {$rendered=[AcpWindow]::HasColorVariation($data.Scan0,$data.Stride,$w,$height)}
                            finally {$bitmap.UnlockBits($data)}
                            $attempts += 1
                            if ($rendered) {break}
                            if ($clock.ElapsedMilliseconds -ge 2000) {throw 'Owned capture did not render a nonuniform frame'}
                            Start-Sleep -Milliseconds 16
                        } while ($true)
                    } finally {$graphics.Dispose()}
                    if ((Owned-Window) -ne $h -or [AcpWindow]::GetForegroundWindow() -ne $h) {throw 'Owned capture lost foreground'}
                    $file=Join-Path $images $command.name
                    $bitmap.Save($file,[Drawing.Imaging.ImageFormat]::Png)
                } finally {$bitmap.Dispose()}
                @{status='ok';rpc_id=$rid;file=$file;frame_attempts=$attempts} | ConvertTo-Json -Compress
                continue requests
            }
            'close' { [AcpWindow]::PostMessage($h,0x10,[IntPtr]::Zero,[IntPtr]::Zero) | Out-Null }
            default { throw 'Unsupported bounded window action' }
        }
        @{status='ok';rpc_id=$rid;input_layout=$layout} | ConvertTo-Json -Compress
    } catch { @{status='refused';rpc_id=$rid;reason=$_.Exception.Message} | ConvertTo-Json -Compress }
}
} finally {
    if ($originalLayout -ne [IntPtr]::Zero) {
        try { if ((Owned-Window) -eq $layoutWindow) { [void][AcpWindow]::PostMessage($layoutWindow,0x50,[IntPtr]::Zero,$originalLayout) } } catch { }
    }
}
