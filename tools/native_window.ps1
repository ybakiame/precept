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
 [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
 [DllImport("user32.dll")] public static extern void keybd_event(byte k, byte s, uint f, UIntPtr e);
 [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern short VkKeyScan(char c);
 [StructLayout(LayoutKind.Sequential)] public struct RECT { public int left,top,right,bottom; }
 [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
 [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr h, IntPtr dc, uint flags);
 [DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr h, IntPtr after, int x,int y,int w,int z,uint flags);
 [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr h,uint m,IntPtr w,IntPtr l);
}
'@
$nonce = $null
$expected = 0
$remote = $false
$images = $null
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
:requests while ($null -ne ($line = [Console]::ReadLine())) {
    try {
        $command = $line | ConvertFrom-Json
        if ($command.action -eq 'init') {
            if ($nonce -or $command.nonce -notmatch '^[0-9a-f]{16}$') { throw 'Invalid one-time nonce' }
            $nonce = $command.nonce; $expected = [uint32]$command.pid; $remote = [bool]$command.remote
            if (-not $remote -and $expected -eq 0) { throw 'Explicit native PID required' }
            $images = Join-Path (Split-Path -Parent $PSScriptRoot) "artifacts/local-p1-m2/s5-window-$nonce"
            if (Test-Path -LiteralPath $images) { throw 'Window capture directory must be new' }
            New-Item -ItemType Directory -Path $images | Out-Null
            @{status='ok';images=$images} | ConvertTo-Json -Compress
            continue
        }
        if (-not $nonce) { throw 'Initialize owned window first' }
        $h = Owned-Window
        $active = [AcpWindow]::GetForegroundWindow() -eq $h
        if ($command.action -eq 'observe') { @{status='ok';foreground=$active} | ConvertTo-Json -Compress; continue }
        if ($command.action -in @('key','text','capture') -and -not $active) { @{status='foreground_required';nonce=$nonce} | ConvertTo-Json -Compress; continue }
        switch ($command.action) {
            'key' {
                if ($command.key -notmatch '^(alt\+shift\+[umerxlsvynzi]|alt\+[opkwft1-6abc]|alt\+(right|left)|ctrl\+[alrpq]|escape|enter|tab)$') { throw 'Key outside fixed synthetic vocabulary' }
                $parts = $command.key -split '\+'
                $last = $parts[-1]
                $code = switch ($last) { 'escape' {27}; 'tab' {9}; 'enter' {13}; 'right' {39}; 'left' {37}; default { if ($last -notmatch '^[a-z0-9]$') { throw 'Unsupported synthetic key' }; [int][char]$last.ToUpperInvariant() } }
                $mods = @(); if ($parts -contains 'ctrl') {$mods += 17}; if ($parts -contains 'alt') {$mods += 18}; if ($parts -contains 'shift') {$mods += 16}
                if ($command.key -eq 'ctrl+1' -or ($parts -contains 'win')) { throw 'Reserved key refused' }
                try { foreach($m in $mods) { [AcpWindow]::keybd_event($m,0,0,[UIntPtr]::Zero) }; [AcpWindow]::keybd_event($code,0,0,[UIntPtr]::Zero); [AcpWindow]::keybd_event($code,0,2,[UIntPtr]::Zero) }
                finally { [array]::Reverse($mods); foreach($m in $mods) { [AcpWindow]::keybd_event($m,0,2,[UIntPtr]::Zero) } }
            }
            'text' {
                if ($command.text.Length -gt 1024 -or $command.text.Contains("`n") -or $command.text.Contains("`r")) { throw 'Synthetic field bound exceeded' }
                foreach($c in $command.text.ToCharArray()) {
                    $current = Owned-Window; if ($current -ne $h -or [AcpWindow]::GetForegroundWindow() -ne $h) { throw 'Foreground changed during native input' }
                    $scan = [int][AcpWindow]::VkKeyScan($c)
                    if ([int]$c -lt 32 -or [int]$c -gt 126 -or $scan -lt 0 -or ($scan -shr 8) -gt 1) { throw 'Only fixed ASCII synthetic fields supported' }
                    $shift = ($scan -band 256) -ne 0
                    try { if ($shift) { [AcpWindow]::keybd_event(16,0,0,[UIntPtr]::Zero) }; [AcpWindow]::keybd_event(($scan -band 255),0,0,[UIntPtr]::Zero); [AcpWindow]::keybd_event(($scan -band 255),0,2,[UIntPtr]::Zero) }
                    finally { if ($shift) { [AcpWindow]::keybd_event(16,0,2,[UIntPtr]::Zero) } }
                    Start-Sleep -Milliseconds 4
                }
            }
            'resize' {
                $w=[int]$command.width;$height=[int]$command.height
                if ($w -lt 640 -or $w -gt 1600 -or $height -lt 480 -or $height -gt 1000) { throw 'Synthetic resize out of bounds' }
                if (-not [AcpWindow]::SetWindowPos($h,[IntPtr]::Zero,0,0,$w,$height,0x16)) { throw 'Owned resize refused' }
            }
            'capture' {
                if ($command.name -notmatch '^[a-z0-9-]+\.png$') { throw 'Invalid capture name' }
                $rect=[AcpWindow+RECT]::new(); if (-not [AcpWindow]::GetWindowRect($h,[ref]$rect)) {throw 'Owned bounds unavailable'}
                $w=$rect.right-$rect.left;$height=$rect.bottom-$rect.top
                if ($w -lt 100 -or $w -gt 2000 -or $height -lt 100 -or $height -gt 1600) {throw 'Capture bounds refused'}
                $bitmap=[Drawing.Bitmap]::new($w,$height);$graphics=[Drawing.Graphics]::FromImage($bitmap);$dc=$graphics.GetHdc()
                try { if (-not [AcpWindow]::PrintWindow($h,$dc,2)) {throw 'Owned render capture failed'} }
                finally {$graphics.ReleaseHdc($dc);$graphics.Dispose()}
                $file=Join-Path $images $command.name
                try {$bitmap.Save($file,[Drawing.Imaging.ImageFormat]::Png)}finally{$bitmap.Dispose()}
                @{status='ok';file=$file} | ConvertTo-Json -Compress
                continue requests
            }
            'close' { [AcpWindow]::PostMessage($h,0x10,[IntPtr]::Zero,[IntPtr]::Zero) | Out-Null }
            default { throw 'Unsupported bounded window action' }
        }
        @{status='ok'} | ConvertTo-Json -Compress
    } catch { @{status='refused';reason=$_.Exception.Message} | ConvertTo-Json -Compress }
}
