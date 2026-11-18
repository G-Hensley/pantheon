#!/usr/bin/env python3
"""Start/stop a private Herdr fixture without loading any user configuration."""
import argparse
import json
import os
from pathlib import Path
import socket
import stat
import struct
import subprocess


def environment(root: Path) -> dict[str, str]:
    return {
        "HOME": str(root / "home"), "PATH": "/usr/bin:/bin",
        "SHELL": "/bin/bash", "TERM": "xterm-256color", "LANG": "C.UTF-8",
        "HC_SOCKET": str(root / "core.sock"), "HC_SESSION": "pantheon-spike",
        "XDG_STATE_HOME": str(root / "state"),
        "XDG_CONFIG_HOME": str(root / "config"),
        "XDG_RUNTIME_DIR": str(root / "runtime"),
        "CODEX_HOME": str(root / "codex"),
    }


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("action", choices=("start", "stop", "status"))
    parser.add_argument("--root", type=Path, default=Path(os.environ.get("PANTHEON_HERDR_ROOT", "/tmp/pantheon-herdr-demo")))
    parser.add_argument("--binary", type=Path, default=Path("/tmp/pantheon-herdr-dist/bin/herdr-core"))
    args = parser.parse_args()
    root = args.root.absolute()
    for ancestor in root.parents:
        if not ancestor.exists():
            continue
        info = ancestor.stat()
        sticky_system_directory = info.st_uid == 0 and info.st_mode & stat.S_ISVTX
        if info.st_uid not in (0, os.getuid()) or (info.st_mode & 0o022 and not sticky_system_directory):
            parser.error("untrusted writable ancestor in fixture path")
    if args.action != "start":
        info = root.lstat()
        if not stat.S_ISDIR(info.st_mode) or root.resolve() != root or info.st_uid != os.getuid() or info.st_mode & 0o077:
            parser.error("unsafe fixture root")
        receipt = json.loads((root / "runtime.json").read_text())
        endpoint = root / "core.sock"
        info = endpoint.lstat()
        if not stat.S_ISSOCK(info.st_mode) or info.st_uid != os.getuid():
            parser.error("unsafe runtime socket")
        with socket.socket(socket.AF_UNIX) as client:
            client.settimeout(15)
            client.connect(str(endpoint))
            pid, uid, _gid = struct.unpack("3i", client.getsockopt(socket.SOL_SOCKET, socket.SO_PEERCRED, 12))
            if (pid, uid) != (receipt["pid"], os.getuid()):
                parser.error("runtime peer changed")
            reader = client.makefile("rb")

            def rpc(method, params):
                client.sendall((json.dumps(dict(jsonrpc="2.0", id=1, method=method, params=params)) + "\n").encode())
                reply = json.loads(reader.readline(1024 * 1024))
                if "error" in reply:
                    raise RuntimeError(reply["error"])
                return reply["result"]

            hello = rpc("hello", {"generation": 1})
            if hello["core"] != receipt["core"] or hello["version"] != "0.1.21":
                parser.error("runtime identity/version changed")
            print(json.dumps(rpc(f"server.{args.action}", {})))
        return
    if args.action == "start":
        # An existing fixture is inspected with status, never silently reused
        # as a fresh environment. In particular, do not import agent profiles.
        root.mkdir(mode=0o700, parents=True, exist_ok=False)
        if root.is_symlink() or root.resolve() != root or root.stat().st_uid != os.getuid() or root.stat().st_mode & 0o077:
            parser.error("root must be canonical, owner-only, and owned by this user")
        for name in ("home", "state", "config", "runtime", "codex", "work"):
            (root / name).mkdir(mode=0o700)
    result = subprocess.run([str(args.binary.resolve()), "server", args.action],
                            env=environment(root), cwd=root, check=True,
                            capture_output=True, text=True)
    if args.action == "start":
        status = json.loads(result.stdout)
        receipt = {"core": status["core"], "pid": status["pid"],
                   "binary": str(args.binary.resolve()), "environment": "empty-profile-v1"}
        (root / "runtime.json").write_text(json.dumps(receipt), encoding="utf-8")
    print(result.stdout, end="")


if __name__ == "__main__":
    main()
