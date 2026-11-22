#!/usr/bin/env python3
"""Mints the fixed session token the recording sends as a cookie.

The secret is the one in `backend/record/conf.yaml`, which exists only for
recording. Run this only when the recording fixture itself changes.
"""
import base64
import hashlib
import hmac
import json
import time


def b64(raw: bytes) -> str:
    return base64.urlsafe_b64encode(raw).rstrip(b"=").decode()


def main() -> None:
    now = int(time.time())
    header = {"alg": "HS256", "typ": "JWT"}
    claims = {
        "user_id": 1,
        "token_version": 0,
        "sub": "user_access",
        "exp": now + 10 * 365 * 24 * 3600,
        "nbf": now - 60,
        "iat": now - 60,
    }
    signing = (
        b64(json.dumps(header, separators=(",", ":")).encode())
        + "."
        + b64(json.dumps(claims, separators=(",", ":")).encode())
    )
    signature = b64(
        hmac.new(b"recording-only-secret", signing.encode(), hashlib.sha256).digest()
    )
    print(f"{signing}.{signature}")


if __name__ == "__main__":
    main()
