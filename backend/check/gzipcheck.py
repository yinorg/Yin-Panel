#!/usr/bin/env python3
"""Compare the HTTP compression behaviour of a Go and a Rust deployment.

For every endpoint it sends one request that offers gzip and one that does not,
decodes the gzip body, and requires:

  * the decoded body to equal that side's plain body;
  * the decoded bodies to be byte-identical between Go and Rust;
  * the same presence of Content-Encoding and Vary on both sides;
  * error statuses to stay uncompressed, as gin-contrib/gzip does.

Usage: gzipcheck.py GO_BASE RUST_BASE TOKEN
"""

import gzip
import http.client
import json
import os
import sys


def request(base, path, token, accept_gzip):
    host_port = base.split("://", 1)[1]
    host, port = host_port.split(":")
    connection = http.client.HTTPConnection(host, int(port), timeout=20)
    headers = {"Cookie": f"yin_token={token}"}
    if accept_gzip:
        headers["Accept-Encoding"] = "gzip"
    connection.request("GET", path, headers=headers)
    response = connection.getresponse()
    body = response.read()
    header_map = {name.lower(): value for name, value in response.getheaders()}
    connection.close()
    return response.status, header_map, body


def decoded(headers, body):
    if headers.get("content-encoding") == "gzip":
        return gzip.decompress(body)
    return body


def main():
    go_base, rust_base, token = sys.argv[1:4]

    status, _, body = request(go_base, "/api/theme/v2/current", token, False)
    revision = json.loads(body)["data"]["revision"]

    status, _, body = request(go_base, "/api/theme/v2/wallpaper/library", token, False)
    library = json.loads(body)["data"]
    wallpaper_asset = ""
    assets = library.get("assets") or library.get("versions") or []
    if isinstance(assets, list) and assets:
        first = assets[0]
        if isinstance(first, dict):
            wallpaper_asset = first.get("url") or first.get("path") or ""
            if wallpaper_asset.startswith("/api"):
                pass
            elif wallpaper_asset:
                wallpaper_asset = f"/api/theme/v2/wallpaper/library{wallpaper_asset}"

    web = os.path.join(
        os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__)))), "backend", "web"
    )
    first_js = sorted(
        name for name in os.listdir(os.path.join(web, "assets", "js")) if name.endswith(".js")
    )[0]

    paths = [
        "/api/about",
        "/api/spaces",
        "/api/theme/v2/current",
        "/api/theme/v2/wallpaper/library",
        f"/api/theme/v2/assets/{revision}/views/home.mjs",
        f"/api/theme/v2/assets/{revision}/styles/home.css",
        f"/api/theme/v2/assets/{revision}/does-not-exist.css",
        f"/assets/js/{first_js}",
        "/",
    ]
    if wallpaper_asset.startswith("/api/theme"):
        paths.append(wallpaper_asset)

    problems = []
    print(f"{'endpoint':64} {'status':>6} {'gzip':>5} {'vary':>5} {'plain':>7} {'gz':>7} {'decoded==plain':>15}")
    for path in paths:
        go_status, go_headers, go_plain = request(go_base, path, token, False)
        go_gz_headers, go_gz = (None, b"")
        go_status_gz, go_gz_headers, go_gz = request(go_base, path, token, True)
        rust_status, rust_headers, rust_plain = request(rust_base, path, token, False)
        rust_status_gz, rust_gz_headers, rust_gz = request(rust_base, path, token, True)

        go_encoded = go_gz_headers.get("content-encoding", "")
        rust_encoded = rust_gz_headers.get("content-encoding", "")
        go_vary = "Accept-Encoding" in go_gz_headers.get("vary", "")
        rust_vary = "Accept-Encoding" in rust_gz_headers.get("vary", "")

        print(
            f"{path[:64]:64} {go_status:>6} {go_encoded or '-':>5} {('yes' if go_vary else '-'):>5} "
            f"{len(go_plain):>7} {len(go_gz):>7}"
        )

        if go_encoded != rust_encoded:
            problems.append(f"{path}: Content-Encoding go={go_encoded!r} rust={rust_encoded!r}")
        if go_vary != rust_vary:
            problems.append(f"{path}: Vary go={go_vary} rust={rust_vary}")
        if decoded(go_gz_headers, go_gz) != go_plain:
            problems.append(f"{path}: Go labelled gzip but the body does not decode to its plain body")
        if decoded(rust_gz_headers, rust_gz) != rust_plain:
            problems.append(f"{path}: Rust labelled gzip but the body does not decode to its plain body")
        if go_plain != rust_plain:
            problems.append(f"{path}: plain bodies differ ({len(go_plain)} vs {len(rust_plain)} bytes)")
        if go_status != rust_status:
            problems.append(f"{path}: status go={go_status} rust={rust_status}")

    if problems:
        for problem in problems:
            print(f"FAIL: {problem}", file=sys.stderr)
        return 1
    print(f"  {len(paths)} endpoints agree on encoding, Vary, status and decoded bytes")
    return 0


if __name__ == "__main__":
    sys.exit(main())
