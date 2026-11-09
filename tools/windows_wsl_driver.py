"""WSL-side orchestration for explicit Windows-owner views; no data forwarding."""
import os
from pathlib import Path
import subprocess
from test_local_service import collect, stop


def views(fixture, label, helper):
    required = ['ACP_WINDOWS_TEST_ROOT', 'ACP_WINDOWS_PYTHON', 'ACP_WSL_DISTRIBUTION', 'ACP_WSL_USER']
    if any(not os.environ.get(k) for k in required):
        return dict(status='NOT_RUN', reason='explicit Windows root/Python and WSL distro/user not supplied')
    root = Path(os.environ['ACP_WINDOWS_TEST_ROOT'])
    python = Path(os.environ['ACP_WINDOWS_PYTHON'])
    assert root.is_absolute() and (root / '.local-acp-test-root').is_file(), 'owned Windows ACP test root required'
    assert python.is_absolute() and python.is_file(), 'explicit Windows Python executable required'
    # Windows Python accepts this script's Windows path; WSL args stay literal Linux strings.
    script = subprocess.run(['wslpath', '-w', str(root / 'tools/windows_wsl_views.py')], capture_output=True, text=True, check=True).stdout.strip()
    deps = subprocess.run(['wslpath', '-w', str(root / 'artifacts/local-p1-e2/python')], capture_output=True, text=True, check=True).stdout.strip()
    command = [str(python), script, '--distribution', os.environ['ACP_WSL_DISTRIBUTION'], '--user', os.environ['ACP_WSL_USER'],
               '--helper', str(helper), '--runtime', str(fixture.runtime), '--endpoint', fixture.endpoint, '--project', label]
    # Windows Python consumes PYTHONPATH with semicolon separators. WSLENV explicitly
    # forwards this test-only environment value across interop without path translation.
    source = subprocess.run(['wslpath', '-w', str(root / 'src')], capture_output=True, text=True, check=True).stdout.strip()
    env = {**os.environ, 'PYTHONPATH': deps + ';' + source, 'WSLENV': 'PYTHONPATH'}
    proc = subprocess.Popen(command, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE, env=env)
    lines = collect(proc.stdout)
    errors = []
    collect(proc.stderr, errors)
    def expected(marker):
        line = lines.get(timeout=60)
        assert line and line.decode().strip() == marker, 'Windows owner UI acceptance failed; inspect private local diagnostics'
    def command_word(word):
        proc.stdin.write((word + '\n').encode()); proc.stdin.flush()
    try:
        expected('WINDOWS_VIEWS_READY')
        stop(fixture.proc)
        command_word('disconnected'); expected('WINDOWS_DISCONNECTED')
        fixture.start()
        command_word('reconnected'); expected('WINDOWS_RECONNECTED')
        command_word('close'); expected('WINDOWS_VIEWS_CLOSED')
        proc.wait(timeout=12)
        assert proc.returncode == 0 and fixture.cli('health')['ready']
        return dict(status='PASS', desktop='actual Windows native window with scoped WSL model reached layout',
                    tui='actual Windows ConPTY rendered scoped context/Empty; live disconnect/reconnect',
                    independent_lifetime=True, real_data_capture='NOT_RUN: no screenshots/clipboard/raw output', transport='shell-free WSL owner helper; no TCP')
    finally:
        stop(proc, hard=True)
        (root / 'artifacts/local-p1-e2/windows-wsl-worker.log').write_text(''.join(errors), encoding='utf-8')
