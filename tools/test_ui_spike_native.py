"""Native OS window smoke; synthetic daemon and screenshots only.

Window enumeration establishes actual native execution. Build results alone never
become RUN_PASS. Hosted runners lacking a visible desktop record NOT_RUN explicitly.
"""
import argparse
import ctypes
import json
import os
from pathlib import Path
import platform
import subprocess
import time
import collections
import threading
import psutil
from test_local_service import Fixture, stop

p=argparse.ArgumentParser();p.add_argument("--binary",type=Path,required=True);p.add_argument("--candidate",required=True);p.add_argument("--out",type=Path,required=True);p.add_argument("--allow-no-desktop",action="store_true");args=p.parse_args()
args.out.mkdir(parents=True,exist_ok=True)
def window(pid):
    if platform.system()=="Linux":
        r=subprocess.run(["xdotool","search","--onlyvisible","--pid",str(pid)],capture_output=True,text=True,timeout=3)
        return r.stdout.splitlines()[0] if r.returncode==0 and r.stdout.strip() else None
    if platform.system()=="Windows":
        from ctypes import wintypes
        user=ctypes.WinDLL("user32",use_last_error=True);found=[]
        user.IsWindowVisible.argtypes=[wintypes.HWND]
        user.GetWindowThreadProcessId.argtypes=[wintypes.HWND,ctypes.POINTER(wintypes.DWORD)]
        cb=ctypes.WINFUNCTYPE(wintypes.BOOL,wintypes.HWND,wintypes.LPARAM)
        def visit(hwnd,_):
            owner=wintypes.DWORD();user.GetWindowThreadProcessId(hwnd,ctypes.byref(owner))
            if owner.value==pid and user.IsWindowVisible(hwnd):found.append(int(hwnd))
            return True
        user.EnumWindows(cb(visit),0)
        return found[0] if found else None
    # CoreGraphics enumerates on-screen windows without AppleScript control grants.
    cf=ctypes.CDLL("/System/Library/Frameworks/CoreFoundation.framework/CoreFoundation")
    cg=ctypes.CDLL("/System/Library/Frameworks/CoreGraphics.framework/CoreGraphics")
    cg.CGWindowListCopyWindowInfo.argtypes=[ctypes.c_uint32,ctypes.c_uint32];cg.CGWindowListCopyWindowInfo.restype=ctypes.c_void_p
    cf.CFArrayGetCount.argtypes=[ctypes.c_void_p];cf.CFArrayGetCount.restype=ctypes.c_long
    cf.CFArrayGetValueAtIndex.argtypes=[ctypes.c_void_p,ctypes.c_long];cf.CFArrayGetValueAtIndex.restype=ctypes.c_void_p
    cf.CFStringCreateWithCString.argtypes=[ctypes.c_void_p,ctypes.c_char_p,ctypes.c_uint32];cf.CFStringCreateWithCString.restype=ctypes.c_void_p
    cf.CFDictionaryGetValue.argtypes=[ctypes.c_void_p,ctypes.c_void_p];cf.CFDictionaryGetValue.restype=ctypes.c_void_p
    cf.CFNumberGetValue.argtypes=[ctypes.c_void_p,ctypes.c_int,ctypes.c_void_p]
    cf.CFRelease.argtypes=[ctypes.c_void_p]
    array=cg.CGWindowListCopyWindowInfo(1,0)
    if not array:return None
    key=cf.CFStringCreateWithCString(None,b"kCGWindowOwnerPID",0x08000100)
    try:
        for i in range(cf.CFArrayGetCount(array)):
            item=cf.CFArrayGetValueAtIndex(array,i);value=cf.CFDictionaryGetValue(item,key);owner=ctypes.c_int()
            if value and cf.CFNumberGetValue(value,9,ctypes.byref(owner)) and owner.value==pid:return i+1
    finally:cf.CFRelease(key);cf.CFRelease(array)
    return None

with Fixture() as f:
    before=f.db.read_bytes();started=time.monotonic()
    proc=subprocess.Popen([str(args.binary.resolve()),*f.location],stdout=subprocess.PIPE,stderr=subprocess.PIPE)
    diagnostics=collections.deque(maxlen=100)
    def drain(pipe):
        for line in iter(pipe.readline,b""):diagnostics.append(line.decode("utf-8","replace")[:1000])
    for pipe in [proc.stdout,proc.stderr]:threading.Thread(target=drain,args=(pipe,),daemon=True).start()
    f.children.append(proc)
    try:
        wid=None;deadline=time.monotonic()+45
        while time.monotonic()<deadline:
            if proc.poll() is not None:
                raise AssertionError("native app exited before visible window: "+"".join(diagnostics)[-1500:])
            wid=window(proc.pid)
            if wid:break
            time.sleep(.1)
        if not wid:
            if args.allow_no_desktop:
                result={"status":"NOT_RUN","reason":"hosted runner has no enumerable on-screen window/interactive desktop","os":platform.platform(),"candidate":args.candidate}
            else:raise AssertionError("No native visible window before timeout")
        else:
            startup=time.monotonic()-started
            markers=["ACP_UI_READY","ACP_NSH_READY"] if args.candidate=="hybrid" else ["ACP_NSH_READY" if args.candidate=="nsh" else "ACP_UI_READY"]
            while not all(marker in "".join(diagnostics) for marker in markers):
                assert proc.poll() is None,"native UI stopped before loaded-model render"
                if time.monotonic()>deadline:raise AssertionError("Native window exists but model never became ready")
                time.sleep(.1)
            model_ready=time.monotonic()-started
            time.sleep(.8)
            result={"status":"RUN_PASS","candidate":args.candidate,"os":platform.platform(),"window_probe":platform.system()+" native visible-window enumeration","startup_to_visible_window_seconds":round(startup,4),"rss_bytes_at_probe":psutil.Process(proc.pid).memory_info().rss,"binary_bytes":args.binary.stat().st_size,"data":"authenticated synthetic daemon; no UI SQLite access"}
            result["startup_to_model_ready_seconds"]=round(model_ready,4)
            import mss
            import mss.tools
            with mss.MSS() as capture:
                frame_deadline=time.monotonic()+10
                while True:
                    image=capture.grab(capture.monitors[0])
                    if len(set(image.rgb[::3]))>8:break
                    if time.monotonic()>frame_deadline:raise AssertionError("native window exists but screenshot remains blank")
                    time.sleep(.1)
                mss.tools.to_png(image.rgb,image.size,output=str(args.out/(args.candidate+"-native.png")))
            if platform.system()=="Linux":
                subprocess.run(["xdotool","windowsize",str(wid),"640","480"],check=True,timeout=5);time.sleep(.4)
                with mss.MSS() as capture:
                    image=capture.grab(capture.monitors[0]);mss.tools.to_png(image.rgb,image.size,output=str(args.out/(args.candidate+"-native-small.png")))
                result["resize"]=[1080,760,640,480]
        stop(proc,hard=True)
        assert f.cli("health")["ready"],"window termination stopped daemon"
        assert f.db.read_bytes()==before
        result["daemon_outlives_ui"]=True;result["synthetic_db_unchanged"]=True
        (args.out/(args.candidate+"-native-result.json")).write_text(json.dumps(result,indent=2));print(json.dumps(result))
    finally:
        stop(proc,hard=True)
        (args.out/(args.candidate+"-native-diagnostics.txt")).write_text("".join(diagnostics))
