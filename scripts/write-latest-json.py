#!/usr/bin/env python3
"""Build latest.json from signed updater artifacts already on the GitHub release."""

import json
import os
import subprocess
from datetime import datetime, timezone
from pathlib import Path

tag = os.environ["GITHUB_REF_NAME"]
repo = os.environ["GITHUB_REPOSITORY"]
base = f"https://github.com/{repo}/releases/download/{tag}"
version = json.loads(Path("src-tauri/tauri.conf.json").read_text())["version"]

assets = json.loads(
    subprocess.check_output(
        ["gh", "release", "view", tag, "--json", "assets"],
        text=True,
    )
)
names = [asset["name"] for asset in assets["assets"]]
sig_dir = Path("/tmp/updater-sigs")
sig_dir.mkdir(parents=True, exist_ok=True)


def signature(filename: str) -> str:
    sig_name = f"{filename}.sig"
    if sig_name not in names:
        raise SystemExit(f"Missing signature for {filename}")
    subprocess.check_call(
        [
            "gh",
            "release",
            "download",
            tag,
            "--pattern",
            sig_name,
            "--dir",
            str(sig_dir),
            "--clobber",
        ]
    )
    return (sig_dir / sig_name).read_text().strip()


def choose(predicate, preference) -> str | None:
    found = [name for name in names if predicate(name) and f"{name}.sig" in names]
    for suffix in preference:
        for name in found:
            if name.endswith(suffix):
                return name
    return found[0] if found else None


platforms = {
    "linux-x86_64": choose(
        lambda name: name.endswith(".AppImage.tar.gz") or name.endswith(".AppImage"),
        (".AppImage.tar.gz", ".AppImage"),
    ),
    "windows-x86_64": choose(
        lambda name: name.endswith(("-setup.exe.zip", ".msi.zip", "-setup.exe")),
        ("-setup.exe.zip", ".msi.zip", "-setup.exe"),
    ),
    "darwin-aarch64": choose(
        lambda name: name.endswith(".app.tar.gz") and "aarch64" in name,
        (".app.tar.gz",),
    ),
    "darwin-x86_64": choose(
        lambda name: name.endswith(".app.tar.gz")
        and "aarch64" not in name
        and ("x64" in name or "x86_64" in name),
        (".app.tar.gz",),
    ),
}

selected = {key: value for key, value in platforms.items() if value}
if "linux-x86_64" not in selected:
    raise SystemExit(f"No signed Linux updater archive in release assets: {names}")

manifest = {
    "version": version,
    "notes": f"Release {version}",
    "pub_date": datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
    "platforms": {
        key: {"signature": signature(filename), "url": f"{base}/{filename}"}
        for key, filename in selected.items()
    },
}

Path("latest.json").write_text(json.dumps(manifest, indent=2) + "\n")
print(json.dumps({key: value for key, value in selected.items()}, indent=2))
