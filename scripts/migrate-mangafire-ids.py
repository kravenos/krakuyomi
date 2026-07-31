#!/usr/bin/env python3
"""Safely migrate MangaFire ids and their cached library posters.

MangaFire changed canonical ids from ``/manga/...`` to ``/title/...``. RakuYomi
keys both database rows and cached poster filenames by manga id, so migrating
only SQLite leaves every migrated title without its cached cover.

Examples:

    # Preview, then migrate a database and an attached downloads/.posters dir.
    python scripts/migrate-mangafire-ids.py migrate input.db id-map.json output.db \
        --posters-dir /path/to/downloads/.posters
    python scripts/migrate-mangafire-ids.py migrate input.db id-map.json output.db \
        --posters-dir /path/to/downloads/.posters --apply

    # Repair posters after the database migration was already installed.
    python scripts/migrate-mangafire-ids.py posters id-map.json \
        /path/to/downloads/.posters --apply

The input database and old poster files are never modified. New poster files
are written atomically, and an existing different destination is a hard error.
"""

import argparse
import base64
import filecmp
import hashlib
import json
import os
import shutil
import sqlite3
import sys
import tempfile
from pathlib import Path
from typing import NamedTuple


SOURCE_ID = "multi.mangafire"


class PosterConflictError(RuntimeError):
    """A new-id cache entry exists but differs from the old-id poster."""


class PosterMigrationResult(NamedTuple):
    copied: int
    already_present: int
    missing: int


def poster_cache_filename(source_id: str, manga_id: str) -> str:
    """Return the filename used by Rust ``ChapterStorage::path_for_poster``."""

    digest = hashlib.sha256((source_id + manga_id).encode("utf-8")).digest()
    encoded = base64.urlsafe_b64encode(digest).rstrip(b"=").decode("ascii")
    return f"{encoded}.jpg"


def load_mapping(path: Path) -> dict[str, str]:
    """Load and validate an old-to-new canonical MangaFire id mapping."""

    with path.open(encoding="utf-8") as handle:
        mapping = json.load(handle)
    if not isinstance(mapping, dict) or not mapping:
        raise ValueError("id map must be a non-empty JSON object")
    if not all(isinstance(old, str) and isinstance(new, str) for old, new in mapping.items()):
        raise ValueError("every id map key and value must be a string")
    if len(set(mapping.values())) != len(mapping):
        raise ValueError("id map contains duplicate destination ids")
    return mapping


def migrate_poster_cache(
    poster_dir: Path, mapping: dict[str, str], *, apply: bool
) -> PosterMigrationResult:
    """Copy old-id posters to their new-id cache keys without overwriting data."""

    poster_dir = Path(poster_dir)
    if not poster_dir.is_dir():
        raise FileNotFoundError(f"poster directory does not exist: {poster_dir}")

    copied = 0
    already_present = 0
    missing = 0
    planned: list[tuple[Path, Path]] = []

    for old_id, new_id in mapping.items():
        old_path = poster_dir / poster_cache_filename(SOURCE_ID, old_id)
        new_path = poster_dir / poster_cache_filename(SOURCE_ID, new_id)

        if new_path.exists():
            if old_path.exists() and not filecmp.cmp(old_path, new_path, shallow=False):
                raise PosterConflictError(
                    f"refusing to overwrite different cached poster: {new_path}"
                )
            already_present += 1
        elif old_path.exists():
            planned.append((old_path, new_path))
        else:
            missing += 1

    if apply:
        for old_path, new_path in planned:
            temporary_path = None
            try:
                with tempfile.NamedTemporaryFile(
                    prefix=f".{new_path.name}.", suffix=".tmp", dir=poster_dir, delete=False
                ) as temporary:
                    temporary_path = Path(temporary.name)
                shutil.copy2(old_path, temporary_path)
                os.replace(temporary_path, new_path)
            finally:
                if temporary_path is not None and temporary_path.exists():
                    temporary_path.unlink()

    copied = len(planned)
    return PosterMigrationResult(copied, already_present, missing)


def quote_identifier(identifier: str) -> str:
    return '"' + identifier.replace('"', '""') + '"'


def tables_with_manga_id(connection: sqlite3.Connection) -> list[str]:
    tables = []
    for (table,) in connection.execute(
        "SELECT name FROM sqlite_master WHERE type='table' ORDER BY name"
    ):
        quoted = quote_identifier(table)
        columns = [row[1] for row in connection.execute(f"PRAGMA table_info({quoted})")]
        if "source_id" in columns and "manga_id" in columns:
            tables.append(table)
    return tables


def database_plan(
    connection: sqlite3.Connection, mapping: dict[str, str]
) -> tuple[list[tuple[str, int]], int]:
    tables = tables_with_manga_id(connection)
    plan = []
    total = 0

    for table in tables:
        quoted = quote_identifier(table)
        table_total = sum(
            connection.execute(
                f"SELECT COUNT(*) FROM {quoted} WHERE source_id=? AND manga_id=?",
                (SOURCE_ID, old_id),
            ).fetchone()[0]
            for old_id in mapping
        )
        if table_total:
            plan.append((table, table_total))
            total += table_total

        collisions = sum(
            connection.execute(
                f"SELECT COUNT(*) FROM {quoted} WHERE source_id=? AND manga_id=?",
                (SOURCE_ID, new_id),
            ).fetchone()[0]
            for new_id in mapping.values()
        )
        if collisions:
            raise ValueError(
                f"{table} already contains {collisions} row(s) at destination ids"
            )

    return plan, total


def migrate_database(
    input_db: Path, output_db: Path, mapping: dict[str, str], *, apply: bool
) -> tuple[list[tuple[str, int]], int]:
    """Preview or create a self-contained migrated SQLite database."""

    input_db = Path(input_db).resolve()
    output_db = Path(output_db).resolve()
    if input_db == output_db:
        raise ValueError("input and output database paths must differ")
    if not input_db.is_file():
        raise FileNotFoundError(f"input database does not exist: {input_db}")
    if output_db.exists():
        raise FileExistsError(f"refusing to overwrite existing output: {output_db}")

    source = sqlite3.connect(f"file:{input_db.as_posix()}?mode=ro", uri=True)
    try:
        plan, total = database_plan(source, mapping)
        if not apply:
            return plan, total

        output_db.parent.mkdir(parents=True, exist_ok=True)
        target = sqlite3.connect(output_db)
        try:
            # SQLite's backup API includes committed WAL state; copying only the
            # main file is the failure mode that corrupted the incident restore.
            source.backup(target)
            target.execute("PRAGMA journal_mode=DELETE")
            target.execute("PRAGMA foreign_keys=OFF")
            with target:
                for table, _ in plan:
                    quoted = quote_identifier(table)
                    for old_id, new_id in mapping.items():
                        target.execute(
                            f"UPDATE {quoted} SET manga_id=? WHERE source_id=? AND manga_id=?",
                            (new_id, SOURCE_ID, old_id),
                        )

            integrity = target.execute("PRAGMA integrity_check").fetchone()[0]
            if integrity != "ok":
                raise RuntimeError(f"SQLite integrity_check failed: {integrity}")
            foreign_key_errors = target.execute("PRAGMA foreign_key_check").fetchall()
            if foreign_key_errors:
                raise RuntimeError(
                    f"SQLite foreign_key_check found {len(foreign_key_errors)} violation(s)"
                )
        except Exception:
            target.close()
            output_db.unlink(missing_ok=True)
            raise
        finally:
            if output_db.exists():
                target.close()
    finally:
        source.close()

    return plan, total


def print_database_plan(plan: list[tuple[str, int]], total: int) -> None:
    print(f"{'table':24s} rows to rewrite")
    print(f"{'-' * 24} ---------------")
    for table, count in plan:
        print(f"{table:24s} {count}")
    print(f"{'TOTAL':24s} {total}")


def print_poster_result(result: PosterMigrationResult, apply: bool) -> None:
    verb = "copied" if apply else "to copy"
    print(f"posters {verb}: {result.copied}")
    print(f"posters already present: {result.already_present}")
    print(f"posters unavailable in old cache: {result.missing}")


def parse_args(argv: list[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    subparsers = parser.add_subparsers(dest="command", required=True)

    migrate = subparsers.add_parser("migrate", help="migrate a database and optional posters")
    migrate.add_argument("input_db", type=Path)
    migrate.add_argument("id_map", type=Path)
    migrate.add_argument("output_db", type=Path)
    migrate.add_argument("--posters-dir", type=Path)
    migrate.add_argument("--apply", action="store_true")

    posters = subparsers.add_parser(
        "posters", help="repair poster keys after a database was already migrated"
    )
    posters.add_argument("id_map", type=Path)
    posters.add_argument("posters_dir", type=Path)
    posters.add_argument("--apply", action="store_true")
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = parse_args(sys.argv[1:] if argv is None else argv)
    try:
        mapping = load_mapping(args.id_map)
        print(f"id map entries: {len(mapping)}")

        if args.command == "posters":
            result = migrate_poster_cache(args.posters_dir, mapping, apply=args.apply)
            print_poster_result(result, args.apply)
        else:
            if args.posters_dir is not None:
                # Validate all poster destinations before producing a database.
                migrate_poster_cache(args.posters_dir, mapping, apply=False)
            plan, total = migrate_database(
                args.input_db, args.output_db, mapping, apply=args.apply
            )
            print_database_plan(plan, total)
            if args.posters_dir is not None:
                result = migrate_poster_cache(
                    args.posters_dir, mapping, apply=args.apply
                )
                print_poster_result(result, args.apply)

        if not args.apply:
            print("DRY RUN - nothing written. Re-run with --apply to make changes.")
        return 0
    except (OSError, ValueError, RuntimeError, sqlite3.Error) as error:
        print(f"ERROR: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
