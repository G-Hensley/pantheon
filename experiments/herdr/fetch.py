#!/usr/bin/env python3
"""Fetch the pinned Linux binary once into a new private directory."""
import hashlib
import os
from pathlib import Path
import platform
import subprocess
import tarfile

ROOT = Path("/tmp/pantheon-herdr-dist")
URL = "https://releases.herdr.dev/core/0.1.21/herdr-core-linux-x86_64.tar.gz"
SHA256 = "78424755c10a40dde38a0bdb55c20ac19706e9ae05ed68fc2acf1fcff0d941c1"


def main():
    if (platform.system(), platform.machine()) != ("Linux", "x86_64"):
        raise SystemExit("This fixture download supports Linux x86_64 only")
    # No existing path, symlink, parent substitution or untrusted shared bin.
    # /tmp's sticky bit prevents a different user replacing our new directory.
    ROOT.mkdir(mode=0o700, exist_ok=True)
    info = ROOT.lstat()
    if ROOT.resolve() != ROOT or info.st_uid != os.getuid() or info.st_mode & 0o077:
        raise SystemExit("Unsafe download root")
    if any(ROOT.iterdir()):
        raise SystemExit("Download root is not empty; preserve the existing artifacts")
    archive = ROOT / "core.tar.gz"
    data = subprocess.run(["curl", "--fail", "--location", "--max-time", "60",
                           "--max-filesize", "16777216", URL],
                          check=True, capture_output=True).stdout
    if hashlib.sha256(data).hexdigest() != SHA256:
        raise SystemExit("Archive checksum mismatch; nothing extracted")
    with archive.open("xb") as output:
        output.write(data)
    binary_dir = ROOT / "bin"
    binary_dir.mkdir(mode=0o700)
    with tarfile.open(archive) as package:
        members = package.getmembers()
        expected = {"herdr-core", "LICENSE", "THIRD-PARTY-NOTICES"}
        if {m.name for m in members} != expected or not all(m.isfile() for m in members):
            raise SystemExit("Unexpected archive entries; nothing extracted")
        package.extractall(binary_dir, members=members, filter="data")
    print(binary_dir / "herdr-core")


if __name__ == "__main__":
    main()
