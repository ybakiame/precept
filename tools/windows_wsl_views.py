"""Windows-side UI acceptance worker, launched explicitly by the WSL test driver.

Only fixed result markers cross stdio. No live screen captures, raw data logs,
credential copies, DB discovery, model turns or daemon management.
"""
import argparse
import json
from pathlib import Path
import sys
from local_console_probe import TerminalProbe, native_start, native_close, exe
from test_local_service import stop


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ['distribution', 'user', 'helper', 'runtime', 'endpoint', 'project']:
        parser.add_argument('--' + name, required=True)
    args = parser.parse_args()
    location = ['--wsl-distribution', args.distribution, '--wsl-user', args.user,
                '--wsl-snapshot', args.helper, '--wsl-runtime-dir', args.runtime,
                '--wsl-endpoint', args.endpoint, '--project', args.project, '--task-id', 'missing']
    terminal = TerminalProbe([str(exe('acp-nsh-tui')), *location])
    desktop = None
    try:
        terminal.wait('LIVE_READ_ONLY')
        terminal.wait('Project: ' + args.project)
        terminal.wait('Empty — no handoff for task missing')
        desktop, window, _ = native_start([str(exe('acp-nsh-native')), *location])
        print('WINDOWS_VIEWS_READY', flush=True)
        for line in sys.stdin:
            if line.strip() == 'disconnected':
                terminal.send(b'\x12')
                terminal.wait('Unavailable')
                print('WINDOWS_DISCONNECTED', flush=True)
            elif line.strip() == 'reconnected':
                terminal.send(b'\x12')
                terminal.wait('Project: ' + args.project)
                print('WINDOWS_RECONNECTED', flush=True)
            elif line.strip() == 'close':
                break
        native_close(window)
        desktop.wait(timeout=10)
        terminal.close()
        terminal = None
        print('WINDOWS_VIEWS_CLOSED', flush=True)
    finally:
        if desktop:
            stop(desktop, hard=True)
        if terminal:
            terminal.close()


if __name__ == '__main__':
    main()
