#!/usr/bin/env python3
"""Coverage report: every Go route needs a success and a failure case.

Reads `rust/app-cases/http/*.toml`, maps each request to the route pattern in
`rust/app/src/go_routes.rs`, and classifies the recorded response as success
(code 0) or failure. Exceptions listed in `rust/record/coverage-exceptions.md`
are reported but not counted as gaps.

Usage: rust/check/coverage-report.py [--json]
Exit status: 0 when every route has at least one success and one failure, or an
exception; 1 otherwise.
"""

from __future__ import annotations

import json
import re
import sys
import tomllib
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
CASES = REPO_ROOT / "rust/app-cases/http"
GO_ROUTES = REPO_ROOT / "rust/app/src/go_routes.rs"
EXCEPTIONS = REPO_ROOT / "rust/record/coverage-exceptions.md"
ENTRY = re.compile(r'\(\s*"([A-Z]+)"\s*,\s*"([^"]+)"\s*\)')


def inventory() -> list[tuple[str, str]]:
    return ENTRY.findall(GO_ROUTES.read_text(encoding="utf-8"))


def segments(path: str) -> list[str]:
    return [part for part in path.split("?")[0].split("/") if part]


def matches(pattern: str, path: str) -> bool:
    expected = segments(pattern)
    actual = segments(path)
    for index, part in enumerate(expected):
        if part.startswith("*"):
            return True
        if part.startswith(":"):
            if index >= len(actual):
                return False
            continue
        if index >= len(actual) or part != actual[index]:
            return False
    return len(expected) == len(actual)


def exceptions() -> set[tuple[str, str]]:
    if not EXCEPTIONS.is_file():
        return set()
    found: set[tuple[str, str]] = set()
    row = re.compile(r"\|\s*(GET|POST|PUT|DELETE|PATCH)\s*\|\s*`([^`]+)`")
    for line in EXCEPTIONS.read_text(encoding="utf-8").splitlines():
        match = row.search(line)
        if match:
            found.add((match.group(1), match.group(2)))
    return found


def case_outcome(data: dict, case: Path) -> str:
    response = data.get("response", {})
    body = response.get("body", "")
    if not body:
        sibling = case.with_suffix(".body")
        if sibling.is_file():
            body = sibling.read_text(encoding="utf-8", errors="replace")
    status = response.get("status", 0)
    if isinstance(body, str) and body.strip().startswith("{"):
        try:
            code = json.loads(body).get("code")
        except json.JSONDecodeError:
            code = None
        if code == 0:
            return "success"
        if code is not None:
            return "failure"
    return "failure" if status >= 400 else "success"


def main() -> int:
    routes = inventory()
    covered: dict[tuple[str, str], dict[str, int]] = {
        route: {"success": 0, "failure": 0} for route in routes
    }
    unmapped: list[str] = []
    for case in sorted(CASES.glob("*.toml")):
        try:
            data = tomllib.loads(case.read_text(encoding="utf-8"))
        except tomllib.TOMLDecodeError:
            continue
        request = data.get("request", {})
        method = request.get("method", "")
        path = request.get("path", "")
        hit = None
        best = -1
        for route in routes:
            if route[0] != method or not matches(route[1], path):
                continue
            # Gin prefers a static segment over `:param`, so the route with the
            # most literal segments wins.
            score = sum(
                1 for part in segments(route[1]) if not part.startswith((":", "*"))
            )
            if score > best:
                best = score
                hit = route
        if hit is None:
            unmapped.append(case.name)
            continue
        covered[hit][case_outcome(data, case)] += 1

    exceptions_set = exceptions()
    gaps = []
    for route in routes:
        counts = covered[route]
        if counts["success"] and counts["failure"]:
            continue
        if route in exceptions_set:
            continue
        gaps.append((route, counts))

    total_success = sum(counts["success"] for counts in covered.values())
    total_failure = sum(counts["failure"] for counts in covered.values())
    print(f"routes inventoried:   {len(routes)}")
    print(f"cases mapped:         {len(routes) and total_success + total_failure}")
    print(f"success cases:        {total_success}")
    print(f"failure cases:        {total_failure}")
    print(f"documented exceptions: {len(exceptions_set)}")
    print(f"unmapped case files:  {len(unmapped)}")
    print(f"routes missing a pair: {len(gaps)}")
    for (method, pattern), counts in gaps:
        print(f"  {method:6} {pattern:55} success={counts['success']} failure={counts['failure']}")
    if unmapped:
        for name in unmapped[:10]:
            print(f"  unmapped: {name}")
    return 0 if not gaps else 1


if __name__ == "__main__":
    sys.exit(main())
