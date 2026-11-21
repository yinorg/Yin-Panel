#!/usr/bin/env python3
"""Probe every route in the Go inventory and fail on unimplemented handlers.

`rust/app/src/go_routes.rs` lists the `(method, path)` pairs the Go build
registers. The Rust front door answers anything it does not implement through
`go_fallback`; with `YIN_UNIMPLEMENTED_MARKER=1` that response carries
`x-yin-unimplemented: 1`. Replay never sets the variable, so its bytes stay
identical to the recording.

Usage: rust/check/route-parity.py [--port N] [--bin PATH] [--list]
Exit status: 0 when every inventoried route is implemented, 1 otherwise.

The unimplemented list is written to `rust/parity-unimplemented.txt`.
"""

from __future__ import annotations

import argparse
import os
import re
import shutil
import signal
import subprocess
import sys
import time
import urllib.error
import urllib.request
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
GO_ROUTES = REPO_ROOT / "rust/app/src/go_routes.rs"
RUNNER = REPO_ROOT / "rust/record/run-rust.sh"
REPORT = REPO_ROOT / "rust/parity-unimplemented.txt"
ENTRY = re.compile(r'\(\s*"([A-Z]+)"\s*,\s*"([^"]+)"\s*\)')


def inventory() -> list[tuple[str, str]]:
    routes = []
    for match in ENTRY.finditer(GO_ROUTES.read_text(encoding="utf-8")):
        routes.append((match.group(1), match.group(2)))
    return routes


def concrete_path(pattern: str) -> str:
    """Turn `/api/spaces/:spaceId/items/:itemId` into a requestable path."""
    segments = []
    for segment in pattern.split("/"):
        if segment.startswith(":"):
            segments.append("1")
        elif segment.startswith("*"):
            segments.append("missing.bin")
        else:
            segments.append(segment)
    return "/".join(segments) or "/"


def wait_ready(base: str, deadline: float) -> bool:
    while time.time() < deadline:
        try:
            with urllib.request.urlopen(f"{base}/api/about", timeout=1) as response:
                if response.status == 200:
                    return True
        except Exception:
            time.sleep(0.2)
    return False


def probe(method: str, path: str, base: str) -> tuple[bool, str]:
    request = urllib.request.Request(f"{base}{path}", method=method)
    try:
        with urllib.request.urlopen(request, timeout=10) as response:
            marker = response.headers.get("x-yin-unimplemented")
            return marker is None, f"{response.status}"
    except urllib.error.HTTPError as error:
        marker = error.headers.get("x-yin-unimplemented")
        return marker is None, f"{error.code}"
    except Exception as error:  # connection refused, timeout, malformed URL
        return False, f"error: {error}"


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--port", type=int, default=18090)
    parser.add_argument("--bin", default=None)
    parser.add_argument("--list", action="store_true", help="print paths, do not probe")
    args = parser.parse_args()

    routes = inventory()
    if not routes:
        print("no routes parsed from rust/app/src/go_routes.rs", file=sys.stderr)
        return 2
    if args.list:
        for method, pattern in routes:
            print(f"{method:6} {pattern}")
        return 0

    binary = Path(args.bin) if args.bin else REPO_ROOT / "rust/app/target/release/yin-panel"
    if not os.access(binary, os.X_OK):
        print(
            f"missing binary {binary}; run: cargo build --release --manifest-path rust/app/Cargo.toml",
            file=sys.stderr,
        )
        return 2

    work = Path(os.environ.get("TMPDIR", "/tmp")) / "yin-panel-parity-probe"
    shutil.rmtree(work, ignore_errors=True)
    env = dict(os.environ)
    env.update(
        {
            "PORT": str(args.port),
            "YIN_PANEL_RECORD_DIR": str(work),
            "YIN_PANEL_RUST_BIN": str(binary),
            "YIN_UNIMPLEMENTED_MARKER": "1",
        }
    )
    server = subprocess.Popen(
        ["sh", str(RUNNER)],
        env=env,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.PIPE,
        start_new_session=True,
    )
    base = f"http://127.0.0.1:{args.port}"
    try:
        if not wait_ready(base, time.time() + 45):
            error = b""
            server.terminate()
            try:
                error = server.stderr.read() if server.stderr else b""
            except Exception:
                pass
            print(f"server did not start on {base}\n{error.decode(errors='replace')[:800]}", file=sys.stderr)
            return 2

        missing: list[tuple[str, str, str]] = []
        for method, pattern in routes:
            ok, detail = probe(method, concrete_path(pattern), base)
            if not ok:
                missing.append((method, pattern, detail))
    finally:
        if server.poll() is None:
            server.send_signal(signal.SIGTERM)
            try:
                server.wait(timeout=10)
            except subprocess.TimeoutExpired:
                server.kill()

    REPORT.write_text(
        "".join(f"{method} {pattern} ({detail})\n" for method, pattern, detail in missing),
        encoding="utf-8",
    )
    print(f"routes inventoried: {len(routes)}")
    print(f"unimplemented:      {len(missing)} (see {REPORT.relative_to(REPO_ROOT)})")
    for method, pattern, _ in missing[:20]:
        print(f"  {method:6} {pattern}")
    if len(missing) > 20:
        print(f"  ... {len(missing) - 20} more")
    return 0 if not missing else 1


if __name__ == "__main__":
    sys.exit(main())
