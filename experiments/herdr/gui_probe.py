#!/usr/bin/env python3
"""Local X11 fixture driver. Targets only the experiment's named window."""
import ctypes as c
import json
import os
from pathlib import Path
import socket
import subprocess
import time
from PIL import Image

ROOT = Path(os.environ.get("PANTHEON_HERDR_ROOT", "/tmp/pantheon-herdr-demo"))
APP = Path(os.environ.get("CARGO_TARGET_DIR", str(Path.home() / ".cache/pantheon-herdr-target"))) / "debug/pantheon"
CODEX = Path(os.environ.get("PANTHEON_HERDR_CODEX", str(Path.home() / ".local/bin/codex"))).resolve()
X = c.CDLL("libX11.so.6")
X.XOpenDisplay.restype = c.c_void_p
D = X.XOpenDisplay(None)
if not D:
    raise RuntimeError("X11 unavailable")
# A window may disappear between QueryTree and FetchName during close/reopen.
class XError(c.Structure):
    _fields_ = [("type", c.c_int), ("display", c.c_void_p),
                ("resource", c.c_ulong), ("serial", c.c_ulong),
                ("code", c.c_ubyte), ("request", c.c_ubyte), ("minor", c.c_ubyte)]


@c.CFUNCTYPE(c.c_int, c.c_void_p, c.POINTER(XError))
def x_error(_display, event):
    if event.contents.code != 3:  # BadWindow is expected only while discovering windows.
        print(f"X11 error {event.contents.code}", flush=True)
    return 0


X.XSetErrorHandler(x_error)
for name in ("XDefaultRootWindow", "XQueryTree", "XFetchName", "XGetGeometry",
             "XTranslateCoordinates", "XRaiseWindow", "XSetInputFocus", "XFlush",
             "XKeysymToKeycode", "XInternAtom", "XSendEvent"):
    getattr(X, name).argtypes = [c.c_void_p]  # remaining C arguments are variadic here
X.XDefaultRootWindow.restype = c.c_ulong
X.XStringToKeysym.argtypes = [c.c_char_p]
X.XStringToKeysym.restype = c.c_ulong
X.XKeysymToKeycode.restype = c.c_uint
X.XInternAtom.restype = c.c_ulong
DESKTOP = X.XDefaultRootWindow(D)


def rpc(method, params):
    with socket.socket(socket.AF_UNIX) as s:
        s.settimeout(5)
        s.connect(str(ROOT / "core.sock"))
        s.sendall((json.dumps(dict(jsonrpc="2.0", id=1, method=method, params=params)) + "\n").encode())
        answer = json.loads(s.makefile("rb").readline())
        if "error" in answer:
            raise RuntimeError(answer["error"])
        return answer["result"]


def find_window(parent=DESKTOP):
    name = c.c_char_p()
    X.XFetchName(D, c.c_ulong(parent), c.byref(name))
    title = name.value.decode(errors="replace") if name.value else ""
    if name:
        X.XFree(name)
    if title == "Pantheon | Herdr experiment":
        return parent
    root, ancestor = c.c_ulong(), c.c_ulong()
    children, count = c.POINTER(c.c_ulong)(), c.c_uint()
    if X.XQueryTree(D, c.c_ulong(parent), c.byref(root), c.byref(ancestor), c.byref(children), c.byref(count)):
        ids = list(children[:count.value])
        if children:
            X.XFree(children)
        for child in ids:
            found = find_window(child)
            if found:
                return found
    return None


def wait_for(test, timeout=15):
    end = time.monotonic() + timeout
    while time.monotonic() < end:
        value = test()
        if value:
            return value
        time.sleep(0.1)
    raise AssertionError("Condition timed out")


def launch(mode):
    env = dict(os.environ, GDK_BACKEND="x11", PANTHEON_HERDR_ROOT=str(ROOT),
               PANTHEON_HERDR_BIN="/tmp/pantheon-herdr-dist/bin/herdr-core",
               PANTHEON_HERDR_CODEX=str(CODEX), PANTHEON_HERDR_PROBE=mode,
               XDG_DATA_HOME=str(ROOT / "gui-data"), XDG_CACHE_HOME=str(ROOT / "gui-cache"))
    log = (ROOT / "gui.log").open("ab")
    process = subprocess.Popen([str(APP)], env=env, stdout=log, stderr=log)
    log.close()
    window = wait_for(find_window)
    X.XRaiseWindow(D, c.c_ulong(window))
    X.XSetInputFocus(D, c.c_ulong(window), 1, 0)
    X.XFlush(D)
    return process, window


def geometry(window):
    root, x, y = c.c_ulong(), c.c_int(), c.c_int()
    width, height, border, depth = c.c_uint(), c.c_uint(), c.c_uint(), c.c_uint()
    X.XGetGeometry(D, c.c_ulong(window), c.byref(root), c.byref(x), c.byref(y),
                   c.byref(width), c.byref(height), c.byref(border), c.byref(depth))
    child = c.c_ulong()
    X.XTranslateCoordinates(D, c.c_ulong(window), c.c_ulong(DESKTOP), 0, 0,
                            c.byref(x), c.byref(y), c.byref(child))
    return x.value, y.value, width.value, height.value


class XImage(c.Structure):
    _fields_ = [("width", c.c_int), ("height", c.c_int), ("xoffset", c.c_int),
                ("format", c.c_int), ("data", c.c_void_p), ("byte_order", c.c_int),
                ("bitmap_unit", c.c_int), ("bitmap_bit_order", c.c_int),
                ("bitmap_pad", c.c_int), ("depth", c.c_int), ("bytes_per_line", c.c_int),
                ("bits_per_pixel", c.c_int), ("red_mask", c.c_ulong),
                ("green_mask", c.c_ulong), ("blue_mask", c.c_ulong)]


def screenshot(window, name):
    _, _, width, height = geometry(window)
    path = ROOT / name
    X.XGetImage.argtypes = [c.c_void_p, c.c_ulong, c.c_int, c.c_int, c.c_uint,
                           c.c_uint, c.c_ulong, c.c_int]
    X.XGetImage.restype = c.POINTER(XImage)
    image = X.XGetImage(D, window, 0, 0, width, height, c.c_ulong(-1), 2)
    if not image:
        raise RuntimeError("Cannot capture experiment window")
    data = image.contents
    if data.bits_per_pixel != 32 or data.byte_order != 0:
        raise RuntimeError("Unsupported fixture screenshot format")
    Image.frombytes("RGB", (width, height),
                    c.string_at(data.data, data.bytes_per_line * height),
                    "raw", "BGRX", data.bytes_per_line).save(path)
    X.XDestroyImage(image)
    print(path, flush=True)


class ClientMessage(c.Structure):
    _fields_ = [("type", c.c_int), ("serial", c.c_ulong), ("send_event", c.c_int),
                ("display", c.c_void_p), ("window", c.c_ulong), ("message_type", c.c_ulong),
                ("format", c.c_int), ("data", c.c_long * 5), ("padding", c.c_long * 12)]


def close(process, window):
    event = ClientMessage(type=33, display=D, window=window, format=32,
                          message_type=X.XInternAtom(D, b"WM_PROTOCOLS", 0))
    event.data[0] = X.XInternAtom(D, b"WM_DELETE_WINDOW", 0)
    X.XSendEvent(D, c.c_ulong(window), 0, 0, c.byref(event))
    X.XFlush(D)
    process.wait(timeout=15)
    wait_for(lambda: find_window() is None)


if __name__ == "__main__":
    def screen(address):
        return rpc("terminal.read", {"terminal": address})["text"]

    process, window = launch("shell-start")
    wait_for(lambda: (ROOT / "shell.json").exists())
    first = json.loads((ROOT / "shell.json").read_text())
    wait_for(lambda: "GUI λ界🙂" in screen(first["address"]))
    screenshot(window, "unicode.png")
    assert "OUTPUT_WHILE_CLOSED" not in screen(first["address"]).splitlines()
    close(process, window)
    live = rpc("terminal.get", {"terminal": first["address"]})
    assert live["pid"] == first["pid"] and live["state"] != "finished"
    wait_for(lambda: "OUTPUT_WHILE_CLOSED" in screen(first["address"]).splitlines())
    process, window = launch("shell-resume")
    wait_for(lambda: "PERSISTED=original" in screen(first["address"]))
    time.sleep(1)
    screenshot(window, "identity.png")
    live = rpc("terminal.get", {"terminal": first["address"]})
    assert (live["pid"], live["created_ms"]) == (first["pid"], first["created_ms"])
    close(process, window)

    process, window = launch("codex")
    wait_for(lambda: (ROOT / "codex.json").exists())
    time.sleep(3)
    screenshot(window, "codex.png")
    agent = json.loads((ROOT / "codex.json").read_text())
    agent_text = screen(agent["address"])
    assert "Codex" in agent_text
    close(process, window)
    live = rpc("terminal.get", {"terminal": agent["address"]})
    assert live["pid"] == agent["pid"] and live["state"] != "finished"
    process, window = launch("codex")
    time.sleep(3)
    screenshot(window, "codex-reattached.png")
    live = rpc("terminal.get", {"terminal": agent["address"]})
    assert (live["pid"], live["created_ms"]) == (agent["pid"], agent["created_ms"])
    assert "Codex" in screen(agent["address"])
    close(process, window)
    result = {"shell_pid": first["pid"], "shell_created_ms": first["created_ms"],
              "codex_pid": agent["pid"], "codex_created_ms": agent["created_ms"],
              "desktop_restart_identity": "passed", "input": "synthetic DOM paste and Enter",
              "agent_execution": "unauthenticated TUI only"}
    (ROOT / "gui-result.json").write_text(json.dumps(result, indent=2) + "\n")
    print(json.dumps(result), flush=True)
