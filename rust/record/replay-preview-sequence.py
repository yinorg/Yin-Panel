#!/usr/bin/env python3
"""Replay the generated-token theme preview flow against Go and Rust."""

import hashlib
import http.client
import io
import json
import difflib
import os
import socket
import subprocess
import tempfile
import time
import zipfile
from email import policy
from email.parser import BytesParser
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
INPUTS = ROOT / "rust/record/inputs.jsonl"
CASE_DIR = ROOT / "rust/app-cases/http"
PREVIEW_CASE = "b0-post-api-theme-v2-admin-preview-success-1"
ASSET_PATH = "assets/preview-probe.mjs"
ASSET_CONTENT = b"export default 'preview probe';\n"


def free_port():
    with socket.socket() as listener:
        listener.bind(("127.0.0.1", 0))
        return listener.getsockname()[1]


def source_request():
    request = next(json.loads(line) for line in INPUTS.read_text().splitlines()
                   if json.loads(line).get("name") == PREVIEW_CASE)
    content_type = request["headers"]["Content-Type"]
    raw = request["body"].encode("latin1")
    message = BytesParser(policy=policy.default).parsebytes(
        f"Content-Type: {content_type}\r\nMIME-Version: 1.0\r\n\r\n".encode() + raw
    )
    package_part = next(part for part in message.iter_parts()
                        if part.get_param("name", header="content-disposition") == "package")
    archive = package_part.get_payload(decode=True)
    files = {}
    with zipfile.ZipFile(io.BytesIO(archive)) as source:
        for name in source.namelist():
            files[name] = source.read(name)
    manifest = json.loads(files["manifest.json"])
    manifest["resources"] = [{
        "path": ASSET_PATH,
        "sha256": hashlib.sha256(ASSET_CONTENT).hexdigest(),
        "mediaType": "text/javascript",
    }]
    files["manifest.json"] = json.dumps(manifest, separators=(",", ":")).encode()
    files[ASSET_PATH] = ASSET_CONTENT
    output = io.BytesIO()
    with zipfile.ZipFile(output, "w", compression=zipfile.ZIP_DEFLATED) as target:
        for name, content in files.items():
            target.writestr(name, content)

    boundary = "yin-preview-sequence"
    body = (
        f"--{boundary}\r\n"
        'Content-Disposition: form-data; name="package"; filename="preview.zip"\r\n'
        "Content-Type: application/zip\r\n\r\n"
    ).encode() + output.getvalue() + f"\r\n--{boundary}--\r\n".encode()
    return request["headers"]["Cookie"], boundary, body


def request(port, method, path, cookie, body=None, content_type=None):
    connection = http.client.HTTPConnection("127.0.0.1", port, timeout=10)
    headers = {"Cookie": cookie}
    if content_type:
        headers["Content-Type"] = content_type
    connection.request(method, path, body=body, headers=headers)
    response = connection.getresponse()
    result = response.status, {key.lower(): value for key, value in response.getheaders()}, response.read()
    connection.close()
    return result


def start(command, port, work, log):
    env = os.environ.copy()
    env.update(PORT=str(port), YIN_PANEL_RECORD_DIR=str(work))
    process = subprocess.Popen(command, cwd=ROOT, env=env, stdout=log,
                               stderr=subprocess.STDOUT)
    for _ in range(200):
        if process.poll() is not None:
            raise RuntimeError(f"server exited early: {command}")
        try:
            status, _, _ = request(port, "GET", "/api/about", "")
            if status < 500:
                return process
        except OSError:
            pass
        time.sleep(0.1)
    process.terminate()
    raise RuntimeError(f"server did not become ready: {command}")


def flow(port, cookie, boundary, body):
    status, _, raw = request(port, "POST", "/api/theme/v2/admin/preview", cookie,
                             body, f"multipart/form-data; boundary={boundary}")
    if status != 200:
        raise AssertionError(f"preview upload returned HTTP {status}: {raw[:500]!r}")
    envelope = json.loads(raw)
    if envelope.get("code") != 0:
        raise AssertionError(f"preview upload failed: {raw[:500]!r}")
    token = envelope["data"]["token"]
    preview_status, _, preview_raw = request(
        port, "GET", f"/api/theme/v2/preview/{token}", cookie
    )
    asset_status, asset_headers, asset_raw = request(
        port, "GET", f"/api/theme/v2/preview/{token}/assets/{ASSET_PATH}", cookie
    )
    if preview_status != 200 or asset_status != 200 or asset_raw != ASSET_CONTENT:
        raise AssertionError(
            f"preview sequence failed: preview={preview_status}, asset={asset_status}, "
            f"asset_body={asset_raw[:200]!r}"
    )
    preview = json.loads(preview_raw)
    preview["data"]["token"] = "<generated-token>"
    preview["data"]["manifest"]["resources"][0]["url"] = "<generated-asset-url>"
    envelope["data"]["token"] = "<generated-token>"
    envelope["data"]["package"]["manifest"]["resources"][0]["url"] = "<generated-asset-url>"
    return {
        "upload": envelope,
        "preview": preview,
        "asset_status": asset_status,
        "asset_content_type": asset_headers.get("content-type", "").split(";")[0],
        "asset": asset_raw.decode("utf-8"),
    }


def stop(process):
    if process and process.poll() is None:
        process.terminate()
        try:
            process.wait(timeout=5)
        except subprocess.TimeoutExpired:
            process.kill()
            process.wait()


def main():
    cookie, boundary, body = source_request()
    with tempfile.TemporaryDirectory(prefix="yin-preview-sequence-") as temp:
        base = Path(temp)
        with (base / "go.log").open("wb") as go_log, (base / "rust.log").open("wb") as rust_log:
            go_port, rust_port = free_port(), free_port()
            go = rust = None
            try:
                go = start(["sh", "rust/record/run-original.sh", str(go_port)],
                           go_port, base / "go", go_log)
                rust = start(["sh", "rust/record/run-rust.sh"],
                             rust_port, base / "rust", rust_log)
                go_result = flow(go_port, cookie, boundary, body)
                rust_result = flow(rust_port, cookie, boundary, body)
                if go_result != rust_result:
                    before = json.dumps(go_result, sort_keys=True, indent=2).splitlines()
                    after = json.dumps(rust_result, sort_keys=True, indent=2).splitlines()
                    detail = "\n".join(difflib.unified_diff(before, after, fromfile="Go", tofile="Rust"))
                    raise AssertionError(f"Go/Rust generated-token preview responses differ:\n{detail}")
                print("Go/Rust preview upload, token GET, and resource GET match")
                print(f"asset bytes={len(ASSET_CONTENT)} content-type={go_result['asset_content_type']}")
            finally:
                stop(rust)
                stop(go)


if __name__ == "__main__":
    main()
