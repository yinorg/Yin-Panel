#!/usr/bin/env python3
"""Compare Range and conditional-request behaviour between Go and Rust.

Cases cover the three `http.ServeContent` call sites: the built frontend
(modification time available), theme assets (zero modification time, ETag on
HTML) and video uploads. A case passes when both sides agree on the status,
`Content-Range`, byte count and body bytes.

Usage: rangecheck.py GO_BASE RUST_BASE TOKEN
"""

import http.client
import json
import os
import sys
import uuid


def request(base, path, token, extra=None):
    host_port = base.split("://", 1)[1]
    host, port = host_port.split(":")
    connection = http.client.HTTPConnection(host, int(port), timeout=20)
    headers = {"Cookie": f"yin_token={token}"}
    headers.update(extra or {})
    connection.request("GET", path, headers=headers)
    response = connection.getresponse()
    body = response.read()
    header_map = {name.lower(): value for name, value in response.getheaders()}
    connection.close()
    return response.status, header_map, body


def upload(base, token):
    boundary = "----rangecheck" + uuid.uuid4().hex
    payload = b"\x00\x00\x00\x20ftypisom" + bytes(range(256)) * 8
    body = (
        f"--{boundary}\r\n"
        'Content-Disposition: form-data; name="imgfile"; filename="clip.mp4"\r\n'
        "Content-Type: video/mp4\r\n\r\n"
    ).encode() + payload + f"\r\n--{boundary}--\r\n".encode()
    host_port = base.split("://", 1)[1]
    host, port = host_port.split(":")
    connection = http.client.HTTPConnection(host, int(port), timeout=20)
    connection.request(
        "POST",
        "/api/file/uploadImg",
        body=body,
        headers={
            "Cookie": f"yin_token={token}",
            "Content-Type": f"multipart/form-data; boundary={boundary}",
        },
    )
    response = connection.getresponse()
    parsed = json.loads(response.read())
    connection.close()
    return parsed["data"]["fileName"]


def main():
    go_base, rust_base, token = sys.argv[1:4]
    web = os.path.join(
        os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__)))), "backend", "web"
    )
    first_js = sorted(
        name for name in os.listdir(os.path.join(web, "assets", "js")) if name.endswith(".js")
    )[0]
    asset = f"/assets/js/{first_js}"

    _, go_headers, _ = request(go_base, asset, token)
    last_modified = go_headers.get("last-modified", "")
    _, _, go_library = request(go_base, "/api/theme/v2/wallpaper/library", token)
    revision = json.loads(
        request(go_base, "/api/theme/v2/current", token)[2]
    )["data"]["revision"]
    theme_asset = f"/api/theme/v2/assets/{revision}/views/home.mjs"

    go_name = upload(go_base, token)
    rust_name = upload(rust_base, token)
    if go_name != rust_name:
        print(f"FAIL: content-addressed upload names differ: {go_name} vs {rust_name}", file=sys.stderr)
        return 1
    video = f"/uploads/{go_name}"

    cases = [
        ("static first 100", asset, {"Range": "bytes=0-99"}),
        ("static suffix 50", asset, {"Range": "bytes=-50"}),
        ("static open range", asset, {"Range": "bytes=100-"}),
        ("static unsatisfiable", asset, {"Range": "bytes=999999999-"}),
        ("static If-Modified-Since", asset, {"If-Modified-Since": last_modified}),
        ("theme range", theme_asset, {"Range": "bytes=100-199"}),
        ("theme suffix", theme_asset, {"Range": "bytes=-100"}),
        ("theme If-Range miss", theme_asset, {"Range": "bytes=0-9", "If-Range": '"nope"'}),
        ("video range", video, {"Range": "bytes=0-49"}),
        ("video open range", video, {"Range": "bytes=1000-"}),
        ("video unsatisfiable", video, {"Range": "bytes=999999-"}),
        ("video If-Range by date", video, {"Range": "bytes=0-49", "If-Range": "Sun, 06 Nov 1994 08:49:37 GMT"}),
    ]

    problems = []
    print(f"{'case':30} {'go':>4} {'rust':>4} {'go bytes':>9} {'rust bytes':>10} {'ranges':>8}")
    for label, path, headers in cases:
        go_status, go_headers, go_body = request(go_base, path, token, headers)
        rust_status, rust_headers, rust_body = request(rust_base, path, token, headers)
        go_range = go_headers.get("content-range", "-")
        rust_range = rust_headers.get("content-range", "-")
        print(f"{label:30} {go_status:>4} {rust_status:>4} {len(go_body):>9} {len(rust_body):>10} "
              f"{('yes' if go_range != '-' else '-'):>8}")
        if go_status != rust_status:
            problems.append(f"{label}: status go={go_status} rust={rust_status}")
        if go_range != rust_range:
            problems.append(f"{label}: Content-Range go={go_range!r} rust={rust_range!r}")
        if go_body != rust_body:
            problems.append(f"{label}: bodies differ ({len(go_body)} vs {len(rust_body)} bytes)")

    if problems:
        for problem in problems:
            print(f"FAIL: {problem}", file=sys.stderr)
        return 1
    print(f"  {len(cases)} range/conditional cases agree on status, Content-Range and bytes")
    return 0


if __name__ == "__main__":
    sys.exit(main())
