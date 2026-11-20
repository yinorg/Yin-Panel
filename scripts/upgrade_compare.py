#!/usr/bin/env python3
"""Content-level snapshot and comparison for the Go -> Rust upgrade check.

Row counts cannot prove an upgrade kept the data: a migration can rewrite a
value in place, or delete one row and insert another, and the count stays the
same. This tool snapshots every table row by *column name* and compares rows as
multisets, so a changed value is a mismatch and a duplicate is visible. A schema
upgrade may drop or add columns; rows are compared on the columns both
snapshots have, and the retired columns are reported separately instead of
turning every surviving row into a false positive.

  upgrade_compare.py snapshot DB OUT.json [--uploads DIR]
  upgrade_compare.py compare BEFORE.json AFTER.json
  upgrade_compare.py selftest

Exit status: 0 when the "after" state contains every row and file the "before"
state had, 1 when anything was lost or changed, 2 when the tool itself failed.
"""

from __future__ import annotations

import argparse
import base64
import hashlib
import json
import os
import sqlite3
import sys
import tempfile
import shutil


def encode(value: object) -> str:
    """A stable, typed encoding so 1, 1.0 and '1' never collide."""
    if value is None:
        return "null"
    if isinstance(value, bytes):
        return "blob:" + base64.b64encode(value).decode("ascii")
    if isinstance(value, bool):  # sqlite3 never returns bool, but be explicit
        return "int:1" if value else "int:0"
    if isinstance(value, int):
        return "int:" + str(value)
    if isinstance(value, float):
        return "float:" + repr(value)
    return "text:" + str(value)


def checkpoint(db: str) -> None:
    """Fold the write-ahead log into the database so both phases read the same file.

    The server is stopped before this runs; a leftover WAL would otherwise carry
    rows that a plain file copy (or a second reader) can miss.
    """
    con = sqlite3.connect(db)
    try:
        con.execute("PRAGMA wal_checkpoint(TRUNCATE)")
        con.commit()
    finally:
        con.close()


def table_columns(con: sqlite3.Connection, table: str) -> list[str]:
    return [row[1] for row in con.execute(f'PRAGMA table_info("{table}")')]


def snapshot(db: str, uploads: str | None) -> dict:
    con = sqlite3.connect(f"file:{db}?mode=ro", uri=True)
    try:
        tables = [
            row[0]
            for row in con.execute(
                "SELECT name FROM sqlite_master WHERE type='table' "
                "AND name NOT LIKE 'sqlite_%' ORDER BY name"
            )
        ]
        result: dict = {"database": os.path.basename(db), "tables": {}, "uploads": {}}
        for table in tables:
            columns = table_columns(con, table)
            primary_key = [
                row[1]
                for row in con.execute(f'PRAGMA table_info("{table}")')
                if row[5]
            ]
            rows = []
            for row in con.execute(f'SELECT * FROM "{table}"'):
                rows.append({column: encode(value) for column, value in zip(columns, row)})
            result["tables"][table] = {
                "columns": columns,
                "primary_key": primary_key,
                "rows": rows,
            }
    finally:
        con.close()

    if uploads:
        for name in sorted(os.listdir(uploads)):
            path = os.path.join(uploads, name)
            if not os.path.isfile(path):
                continue
            with open(path, "rb") as handle:
                digest = hashlib.sha256(handle.read()).hexdigest()
            result["uploads"][name] = digest
    return result


def row_digest(row: dict, columns: list[str]) -> str:
    payload = json.dumps([row.get(column, "<absent>") for column in columns], ensure_ascii=False)
    return hashlib.sha256(payload.encode("utf-8")).hexdigest()


def compare(before: dict, after: dict, ignored: dict[str, list[str]] | None = None) -> tuple[list[str], list[str]]:
    """Returns (problems, notes). Problems make the upgrade fail.

    `ignored` names columns the two builds are *known* to refresh themselves, in
    the form `{table: [column, ...]}`. It can never name a primary-key column,
    so a deleted row is always a failure: only a value the service rewrites at
    startup (for example Go's `EnsureBuiltinV2` touching a package's
    `updated_at`) may differ.
    """
    problems: list[str] = []
    notes: list[str] = []
    ignored = ignored or {}

    before_tables = before["tables"]
    after_tables = after["tables"]

    for table, snapshot in before_tables.items():
        if table not in after_tables:
            problems.append(f"table {table} is missing after the upgrade")
            continue
        before_columns = snapshot["columns"]
        after_snapshot = after_tables[table]
        after_columns = after_snapshot["columns"]
        ignored_here = ignored.get(table, [])
        unknown = [column for column in ignored_here if column not in before_columns or column not in after_columns]
        if unknown:
            problems.append(f"table {table}: ignore policy names columns the table does not have: {', '.join(unknown)}")
            continue
        primary_key = snapshot.get("primary_key", [])
        if set(ignored_here) & set(primary_key):
            problems.append(f"table {table}: ignore policy must not exclude a primary-key column")
            continue
        common = [
            column
            for column in before_columns
            if column in after_columns and column not in ignored_here
        ]
        if not common:
            problems.append(f"table {table} has no column left in common")
            continue
        dropped = [column for column in before_columns if column not in after_columns]
        added = [column for column in after_columns if column not in before_columns]
        if dropped:
            notes.append(f"table {table}: columns retired by the upgrade: {', '.join(dropped)}")
        if added:
            notes.append(f"table {table}: columns added by the upgrade: {', '.join(added)}")
        for column in ignored_here:
            notes.append(f"table {table}: {column} is refreshed by the service and is compared by row identity only")

        before_rows = [row_digest(row, common) for row in snapshot["rows"]]
        after_rows = [row_digest(row, common) for row in after_snapshot["rows"]]
        # Multiset containment: every (row, multiplicity) of the old table must
        # still be present. A rewritten value changes the digest; a delete plus
        # insert of the same count lowers the multiplicity of one digest.
        remaining = {}
        for digest in after_rows:
            remaining[digest] = remaining.get(digest, 0) + 1
        lost = []
        for index, digest in enumerate(before_rows):
            if remaining.get(digest, 0) == 0:
                lost.append(index)
            else:
                remaining[digest] -= 1
        if lost:
            sample = snapshot["rows"][lost[0]]
            shown = {column: sample.get(column) for column in common}
            problems.append(
                f"table {table}: {len(lost)} of {len(before_rows)} old rows are gone or changed; "
                f"first sample: {json.dumps(shown, ensure_ascii=False)[:400]}"
            )
        gained = len(after_rows) - len(before_rows) + len(lost)
        if gained > 0:
            notes.append(f"table {table}: {gained} row(s) were added")

    for table in after_tables:
        if table not in before_tables:
            notes.append(f"table {table} was created by the upgrade")

    for name, digest in before["uploads"].items():
        if name not in after["uploads"]:
            problems.append(f"upload {name} is missing after the upgrade")
        elif after["uploads"][name] != digest:
            problems.append(f"upload {name} changed content (sha256 differs)")

    extra_uploads = sorted(set(after["uploads"]) - set(before["uploads"]))
    if extra_uploads:
        notes.append(f"uploads added: {', '.join(extra_uploads)}")

    return problems, notes


def command_snapshot(args: argparse.Namespace) -> int:
    if not os.path.isfile(args.db):
        print(f"database not found: {args.db}", file=sys.stderr)
        return 2
    checkpoint(args.db)
    snap = snapshot(args.db, args.uploads)
    with open(args.out, "w", encoding="utf-8") as handle:
        json.dump(snap, handle, indent=1, sort_keys=True)
        handle.write("\n")
    tables = snap["tables"]
    rows = sum(len(table["rows"]) for table in tables.values())
    print(f"snapshot: {len(tables)} tables, {rows} rows, {len(snap['uploads'])} uploads")
    return 0


def command_compare(args: argparse.Namespace) -> int:
    with open(args.before, encoding="utf-8") as handle:
        before = json.load(handle)
    with open(args.after, encoding="utf-8") as handle:
        after = json.load(handle)
    ignored: dict[str, list[str]] = {}
    for spec in args.ignore_column:
        table, _, column = spec.rpartition(".")
        if not table or not column:
            print(f"--ignore-column expects TABLE.COLUMN, got: {spec}", file=sys.stderr)
            return 2
        ignored.setdefault(table, []).append(column)
    problems, notes = compare(before, after, ignored)
    for note in notes:
        print(f"  note: {note}")
    if problems:
        for problem in problems:
            print(f"FAIL: {problem}", file=sys.stderr)
        return 1
    before_rows = sum(len(table["rows"]) for table in before["tables"].values())
    print(f"  every row and upload present: {before_rows} rows across {len(before['tables'])} tables, "
          f"{len(before['uploads'])} uploads")
    return 0


def command_selftest(args: argparse.Namespace) -> int:
    """Prove the comparison detects the damage a row count would hide.

    Every scenario starts from its own copy of a pristine fixture, so one
    scenario's mutation cannot make the next one fail for the wrong reason.
    """
    work = tempfile.mkdtemp(prefix="upgrade-compare-")
    checks: list[tuple[str, bool]] = []
    try:
        def make_pristine(directory: str) -> tuple[str, str]:
            os.makedirs(directory, exist_ok=True)
            db = os.path.join(directory, "fixture.db")
            uploads = os.path.join(directory, "uploads")
            os.makedirs(uploads, exist_ok=True)
            con = sqlite3.connect(db)
            con.executescript(
                """
                CREATE TABLE account (id INTEGER PRIMARY KEY, mail TEXT, nickname TEXT);
                CREATE TABLE item (id INTEGER PRIMARY KEY, title TEXT NOT NULL, url TEXT);
                INSERT INTO account VALUES (1, 'admin@example.com', 'admin');
                INSERT INTO account VALUES (2, 'other@example.com', 'other');
                INSERT INTO item VALUES (1, 'Home', 'https://example.com');
                INSERT INTO item VALUES (2, 'Docs', 'https://docs.example.com');
                """
            )
            con.commit()
            con.close()
            with open(os.path.join(uploads, "icon.png"), "wb") as handle:
                handle.write(b"\x89PNG\r\n\x1a\nfixture")
            return db, uploads

        pristine_db, pristine_uploads = make_pristine(os.path.join(work, "pristine"))
        base_before = snapshot(pristine_db, pristine_uploads)

        def scenario(name: str) -> tuple[str, str, dict]:
            directory = os.path.join(work, name)
            db, uploads = make_pristine(directory)
            return db, uploads, json.loads(json.dumps(base_before))

        # 1. Identical state must pass.
        db, uploads, before = scenario("same")
        checks.append(("identical database passes", not compare(before, snapshot(db, uploads))[0]))

        # 2. A rewritten value with the same row count must fail.
        db, uploads, before = scenario("rewritten")
        con = sqlite3.connect(db)
        con.execute("UPDATE item SET title = 'Home!' WHERE id = 1")
        con.commit()
        con.close()
        checks.append(("rewritten value is caught", bool(compare(before, snapshot(db, uploads))[0])))

        # 3. Delete plus insert, same count, must fail.
        db, uploads, before = scenario("swapped")
        con = sqlite3.connect(db)
        con.execute("DELETE FROM item WHERE id = 1")
        con.execute("INSERT INTO item VALUES (3, 'Replacement', 'https://replacement.example')")
        con.commit()
        con.close()
        checks.append(("delete plus insert is caught", bool(compare(before, snapshot(db, uploads))[0])))

        # 4. An added column must not look like data loss: this is the shape a
        #    real schema upgrade has, and every surviving row is unchanged.
        db, uploads, before = scenario("migrated")
        con = sqlite3.connect(db)
        con.execute("ALTER TABLE item ADD COLUMN pinned INTEGER DEFAULT 0")
        con.commit()
        con.close()
        checks.append(("added column is tolerated", not compare(before, snapshot(db, uploads))[0]))

        # 5. A truncated upload must fail.
        db, uploads, before = scenario("truncated")
        with open(os.path.join(uploads, "icon.png"), "wb") as handle:
            handle.write(b"\x89PNG\r\n\x1a\n")
        checks.append(("changed upload is caught", bool(compare(before, snapshot(db, uploads))[0])))

        # 6. A missing upload must fail.
        db, uploads, before = scenario("missing")
        os.unlink(os.path.join(uploads, "icon.png"))
        checks.append(("deleted upload is caught", bool(compare(before, snapshot(db, uploads))[0])))

        # 7. The ignore policy must never hide a primary key, and must still
        #    detect a deleted row when another column is ignored.
        db, uploads, before = scenario("ignore-pk")
        problems, _ = compare(before, snapshot(db, uploads), {"item": ["id"]})
        checks.append((
            "ignoring a primary key is refused",
            any("primary-key" in problem for problem in problems),
        ))
        db, uploads, before = scenario("ignore-deleted")
        con = sqlite3.connect(db)
        con.execute("DELETE FROM item WHERE id = 2")
        con.commit()
        con.close()
        problems, _ = compare(before, snapshot(db, uploads), {"item": ["title"]})
        checks.append(("ignored column still detects a deleted row", bool(problems)))

        failed = [name for name, ok in checks if not ok]
        for name, ok in checks:
            print(f"  {'ok  ' if ok else 'FAIL'} {name}")
        if failed:
            print(f"selftest failed: {', '.join(failed)}", file=sys.stderr)
            return 1
        print(f"  {len(checks)}/{len(checks)} comparison self-checks pass")
        return 0
    finally:
        shutil.rmtree(work, ignore_errors=True)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = parser.add_subparsers(dest="command", required=True)

    snap = sub.add_parser("snapshot", help="write a content snapshot of a database")
    snap.add_argument("db")
    snap.add_argument("out")
    snap.add_argument("--uploads", default=None)
    snap.set_defaults(handler=command_snapshot)

    cmp = sub.add_parser("compare", help="compare two snapshots")
    cmp.add_argument("before")
    cmp.add_argument("after")
    cmp.add_argument(
        "--ignore-column",
        action="append",
        default=[],
        metavar="TABLE.COLUMN",
        help="a column the service refreshes itself; never a primary key",
    )
    cmp.set_defaults(handler=command_compare)

    test = sub.add_parser("selftest", help="prove the comparison detects damage")
    test.set_defaults(handler=command_selftest)

    args = parser.parse_args()
    return args.handler(args)


if __name__ == "__main__":
    sys.exit(main())
