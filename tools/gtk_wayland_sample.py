"""Minimal GTK4 positive control (stdlib ctypes; no NSH, user data or writes)."""

import ctypes as C
import json
import time


def main():
    gtk = C.CDLL("libgtk-4.so.1")
    glib = C.CDLL("libglib-2.0.so.0")
    obj = C.CDLL("libgobject-2.0.so.0")

    def bind(lib, name, result, args):
        f = getattr(lib, name)
        f.restype = result
        f.argtypes = args
        return f

    ptr = C.c_void_p
    init = bind(gtk, "gtk_init_check", C.c_int, [])
    assert init(), "GTK cannot open the selected display"
    window = bind(gtk, "gtk_window_new", ptr, [])()
    bind(gtk, "gtk_window_set_title", None, [ptr, C.c_char_p])(
        window, b"ACP synthetic GTK Wayland positive control"
    )
    label = bind(gtk, "gtk_label_new", ptr, [C.c_char_p])(
        b"GTK4 native backend positive control - synthetic only"
    )
    bind(gtk, "gtk_window_set_child", None, [ptr, ptr])(window, label)
    bind(gtk, "gtk_window_set_default_size", None, [ptr, C.c_int, C.c_int])(
        window, 640, 480
    )
    bind(gtk, "gtk_window_present", None, [ptr])(window)
    iterate = bind(glib, "g_main_context_iteration", C.c_int, [ptr, C.c_int])
    mapped = bind(gtk, "gtk_widget_get_mapped", C.c_int, [ptr])
    display = bind(gtk, "gtk_widget_get_display", ptr, [ptr])
    renderer = bind(gtk, "gtk_native_get_renderer", ptr, [ptr])
    type_name = bind(obj, "g_type_name_from_instance", C.c_char_p, [ptr])
    deadline = time.monotonic() + 5
    while time.monotonic() < deadline:
        iterate(None, False)
        time.sleep(0.005)
    report = dict(
        backend=type_name(display(window)).decode(),
        renderer=type_name(renderer(window)).decode(),
        mapped=bool(mapped(window)),
    )
    bind(gtk, "gtk_window_destroy", None, [ptr])(window)
    print(json.dumps(report))
    assert (
        report["backend"] == "GdkWaylandDisplay" and report["mapped"]
    ), "positive control was not native Wayland"


if __name__ == "__main__":
    main()
