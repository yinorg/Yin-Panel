#!/usr/bin/env python3
"""Compare the three monitor handlers with monitoring enabled."""

import http.client
import json
import os
import socket
import subprocess
import tempfile
import time
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
INPUTS = ROOT / "rust/record/inputs.jsonl"


def free_port():
    with socket.socket() as listener:
        listener.bind(("127.0.0.1", 0))
        return listener.getsockname()[1]


def auth_cookie():
    for line in INPUTS.read_text().splitlines():
        item = json.loads(line)
        if item.get("name") == "b0-post-api-system-monitor-getsnapshot-success-1":
            return item["headers"]["Cookie"]
    raise RuntimeError("monitor auth fixture is missing")


def start(command, port, work, conf, log):
    env = os.environ.copy()
    env.update(PORT=str(port), YIN_PANEL_RECORD_DIR=str(work), YIN_PANEL_RECORD_CONF=str(conf))
    process = subprocess.Popen(command, cwd=ROOT, env=env, stdout=log,
                               stderr=subprocess.STDOUT)
    for _ in range(200):
        if process.poll() is not None:
            raise RuntimeError(f"server exited early: {command}")
        try:
            status, _ = request(port, "GET", "/api/about", "")
            if status < 500:
                return process
        except OSError:
            pass
        time.sleep(0.1)
    process.terminate()
    raise RuntimeError(f"server did not become ready: {command}")


def request(port, method, path, cookie, body=b""):
    connection = http.client.HTTPConnection("127.0.0.1", port, timeout=10)
    headers = {"Cookie": cookie, "Content-Type": "application/json"}
    connection.request(method, path, body=body, headers=headers)
    response = connection.getresponse()
    status, raw = response.status, response.read()
    connection.close()
    return status, raw


def stop(process):
    if process and process.poll() is None:
        process.terminate()
        try:
            process.wait(timeout=5)
        except subprocess.TimeoutExpired:
            process.kill()
            process.wait()


def require_success(label, response):
    status, raw = response
    if status != 200:
        raise AssertionError(f"{label}: HTTP {status}: {raw[:500]!r}")
    value = json.loads(raw)
    if value.get("code") != 0:
        raise AssertionError(f"{label}: handler returned {value!r}")
    return value["data"]


def same_shape(left, right, path="data"):
    if isinstance(left, dict) and isinstance(right, dict):
        if left.keys() != right.keys():
            raise AssertionError(f"{path}: keys differ: {left.keys()} != {right.keys()}")
        for key in left:
            same_shape(left[key], right[key], f"{path}.{key}")
        return
    if isinstance(left, list) and isinstance(right, list):
        if len(left) != len(right):
            raise AssertionError(f"{path}: lengths differ: {len(left)} != {len(right)}")
        for index, (a, b) in enumerate(zip(left, right)):
            same_shape(a, b, f"{path}[{index}]")
        return
    if isinstance(left, bool) or isinstance(right, bool):
        if type(left) is not type(right):
            raise AssertionError(f"{path}: types differ: {type(left).__name__} != {type(right).__name__}")
        return
    if isinstance(left, (int, float)) and isinstance(right, (int, float)):
        if type(left) is not type(right):
            raise AssertionError(f"{path}: numeric types differ: {type(left).__name__} != {type(right).__name__}")
        return
    if type(left) is not type(right):
        raise AssertionError(f"{path}: types differ: {type(left).__name__} != {type(right).__name__}")
    if left != right:
        raise AssertionError(f"{path}: {left!r} != {right!r}")


def main():
    cookie = auth_cookie()
    with tempfile.TemporaryDirectory(prefix="yin-monitor-sequence-") as temp:
        base = Path(temp)
        config = (ROOT / "rust/record/conf.yaml").read_text()
        config = config.replace("enable_monitor: false", "enable_monitor: true")
        if "enable_monitor: true" not in config:
            raise RuntimeError("recorder monitor config did not enable monitoring")
        config_path = base / "monitor-enabled.yaml"
        config_path.write_text(config)
        with (base / "go.log").open("wb") as go_log, (base / "rust.log").open("wb") as rust_log:
            go_port, rust_port = free_port(), free_port()
            go = rust = None
            try:
                go = start(["sh", "rust/record/run-original.sh", str(go_port)],
                           go_port, base / "go", config_path, go_log)
                rust = start(["sh", "rust/record/run-rust.sh"],
                             rust_port, base / "rust", config_path, rust_log)
                routes = [
                    ("mountpoints", "/api/system/monitor/getDiskMountpoints", b"{}"),
                    ("disk state", "/api/system/monitor/getDiskStateByPath", b'{"path":"/"}'),
                    ("snapshot", "/api/system/monitor/getSnapshot", b"{}"),
                ]
                outputs = {}
                for label, path, body in routes:
                    go_data = require_success(label + " Go", request(go_port, "POST", path, cookie, body))
                    rust_data = require_success(label + " Rust", request(rust_port, "POST", path, cookie, body))
                    if label != "snapshot" and go_data != rust_data:
                        raise AssertionError(
                            f"{label} differs:\nGo={json.dumps(go_data, sort_keys=True)}\n"
                            f"Rust={json.dumps(rust_data, sort_keys=True)}"
                        )
                    if label == "snapshot":
                        same_shape(go_data, rust_data)
                        if go_data.get("CPU_INFO", {}).get("model") != rust_data.get("CPU_INFO", {}).get("model"):
                            raise AssertionError("snapshot CPU model differs")
                    outputs[label] = len(go_data) if isinstance(go_data, list) else len(go_data)
                print("Go/Rust monitor success parity passed with monitoring enabled")
                for label, count in outputs.items():
                    print(f"{label}: HTTP 200 code=0; top-level entries={count}")
                print("snapshot dynamic numeric values compared by shape and type")
            finally:
                stop(rust)
                stop(go)


if __name__ == "__main__":
    main()
