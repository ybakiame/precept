"""Windows GUI target editor: WSL -> native Windows -> WSL, synthetic fixtures only."""

from local_console_probe import native_wait_marker, native_wait_state
from native_window_input import NativeInput
from test_local_service import Fixture


def switch_targets(process, window):
    from local_console_probe import windows_api
    import time

    user = windows_api()
    assert user.SetWindowPos(window, None, 80, 80, 1080, 760, 0x40)
    time.sleep(0.2)
    editor = NativeInput(process, window)
    # The initial explicit WSL target has read alpha (two scoped records).
    editor.chord(0x11, ord("R"))
    native_wait_marker(process, "target=Wsl")
    native_wait_state(process, rows=2)
    with Fixture(allow=("beta",)) as local:
        before = local.db.read_bytes()
        editor.click(400, 256)  # Local daemon, in the actual connection editor.
        native_wait_marker(process, "draft=Local")
        time.sleep(0.3)
        editor.type_at(450, 341, str(local.runtime))
        editor.type_at(450, 401, local.endpoint)
        editor.click(550, 466)
        native_wait_marker(process, "target=Local")
        line = native_wait_state(process, selected=0, rows=3)
        assert "queried_chars=0" in line
        # WSL fields remain session drafts; no credential or context transfer.
        editor.click(520, 256)
        native_wait_marker(process, "draft=Wsl")
        time.sleep(0.3)
        # Enter in a connection field explicitly applies the complete valid draft.
        editor.click(450, 580)
        editor.chord(0x0D)
        native_wait_marker(process, "target=Wsl")
        native_wait_state(process, selected=0, rows=2)
        assert local.db.read_bytes() == before and local.cli("health")["ready"]
    return dict(
        status="PASS",
        sequence=["WSL/alpha", "Windows/beta", "WSL/alpha"],
        task_cleared=True,
        clipboard="NOT_RUN",
    )
