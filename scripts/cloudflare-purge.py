#!/usr/bin/env python3
"""Purge Yin-Panel's Cloudflare control-plane files by URL."""

import json
import os
import sys
import urllib.error
import urllib.request


API_BASE = "https://api.cloudflare.com/client/v4"
FILES = [
    "https://panel.yiniot.com/",
    "https://panel.yiniot.com/index.html",
    "https://panel.yiniot.com/sw.js",
    "https://panel.yiniot.com/registerSW.js",
    "https://panel.yiniot.com/manifest.webmanifest",
]


def main() -> int:
    token = os.environ.get("CF_API_TOKEN")
    zone_id = os.environ.get("CF_ZONE_ID")
    if not token or not zone_id:
        print("CF_API_TOKEN and CF_ZONE_ID are required", file=sys.stderr)
        return 2

    request = urllib.request.Request(
        f"{API_BASE}/zones/{zone_id}/purge_cache",
        data=json.dumps({"files": FILES}).encode("utf-8"),
        headers={
            "Authorization": f"Bearer {token}",
            "Content-Type": "application/json",
        },
        method="POST",
    )

    try:
        with urllib.request.urlopen(request, timeout=30) as response:
            payload = json.load(response)
    except urllib.error.HTTPError as error:
        try:
            payload = json.loads(error.read().decode("utf-8"))
        except (json.JSONDecodeError, UnicodeDecodeError):
            payload = {}
        errors = payload.get("errors") or [{"message": error.reason}]
        print(f"Cloudflare purge failed: {errors[0].get('message', 'unknown error')}", file=sys.stderr)
        return 1
    except (urllib.error.URLError, TimeoutError) as error:
        print(f"Cloudflare purge request failed: {error}", file=sys.stderr)
        return 1

    if not payload.get("success"):
        errors = payload.get("errors") or [{"message": "unknown error"}]
        print(f"Cloudflare purge failed: {errors[0].get('message', 'unknown error')}", file=sys.stderr)
        return 1

    print(f"Purged {len(FILES)} panel.yiniot.com URLs")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
