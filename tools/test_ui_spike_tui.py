"""Real NSH terminal process, PTY input and terminal-emulator screen assertions.
No ACP agent PTY/session orchestration is involved.
"""
import argparse
import codecs
import fcntl
import json
import os
from pathlib import Path
import pty
import select
import signal
import struct
import subprocess
import termios
import time
import pyte

p=argparse.ArgumentParser();p.add_argument("--binary",type=Path,required=True);p.add_argument("--out",type=Path,required=True);args=p.parse_args()
args.out.mkdir(parents=True,exist_ok=True)
master,slave=pty.openpty()
fcntl.ioctl(slave,termios.TIOCSWINSZ,struct.pack("HHHH",50,160,0,0))
screen=pyte.Screen(160,50);stream=pyte.Stream(screen);decode=codecs.getincrementaldecoder("utf-8")("replace")
started=time.monotonic()
proc=subprocess.Popen([str(args.binary.resolve())],stdin=slave,stdout=slave,stderr=slave,env={**os.environ,"TERM":"xterm-256color","COLORTERM":"truecolor"},start_new_session=True)
os.close(slave)
def wait_for(text,timeout=15):
    deadline=time.monotonic()+timeout
    while time.monotonic()<deadline:
        if select.select([master],[],[],.05)[0]:
            raw=os.read(master,65536)
            stream.feed(decode.decode(raw))
        visible="\n".join(screen.display)
        if text in visible:return visible
        assert proc.poll() is None,"terminal app stopped before expected UI state"
    raise AssertionError("terminal screen did not reach expected state: "+text)
try:
    visible=wait_for("Project: alpha")
    startup=time.monotonic()-started
    os.write(master,b"\t\t\r")
    visible=wait_for("Project: beta")
    (args.out/"nsh-tui-large.txt").write_text(visible)
    fcntl.ioctl(master,termios.TIOCSWINSZ,struct.pack("HHHH",32,100,0,0));screen.resize(32,100)
    os.kill(proc.pid,signal.SIGWINCH)
    time.sleep(.2)
    os.write(master,b"\t\r")
    visible=wait_for("Project: beta")
    (args.out/"nsh-tui-small.txt").write_text(visible)
    rss=next((int(s.split()[1]) for s in Path(f"/proc/{proc.pid}/status").read_text().splitlines() if s.startswith("VmRSS:")),None)
    os.write(master,b"q");proc.wait(timeout=8);assert proc.returncode==0
    result={"status":"TEST_PASS","backend":"real nsh-platform-tui/ratatui process in PTY","keyboard":"Tab Tab Enter selected beta","resize":[160,50,100,32],"startup_to_expected_screen_seconds":round(startup,4),"rss_kib_at_probe":rss,"binary_bytes":args.binary.stat().st_size,"quit_exit":proc.returncode}
    (args.out/"tui-result.json").write_text(json.dumps(result,indent=2));print(json.dumps(result))
finally:
    if proc.poll() is None:proc.kill();proc.wait()
    os.close(master)
