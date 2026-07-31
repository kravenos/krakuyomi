import importlib.util
import sqlite3
import tempfile
import unittest
from pathlib import Path


SCRIPT_PATH = Path(__file__).parents[1] / "migrate-mangafire-ids.py"
SPEC = importlib.util.spec_from_file_location("migrate_mangafire_ids", SCRIPT_PATH)
MIGRATION = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MIGRATION)


class PosterCacheMigrationTests(unittest.TestCase):
    def test_cache_filename_matches_rust_chapter_storage(self):
        self.assertEqual(
            MIGRATION.poster_cache_filename(
                "multi.mangafire", "/manga/20th-century-boyss.kwj4"
            ),
            "LoOLUGy7yUTYYDooVPXQ_-k4EGtrU_YF1URtx3aF-RQ.jpg",
        )
        self.assertEqual(
            MIGRATION.poster_cache_filename(
                "multi.mangafire", "/title/kwj4-20th-century-boys"
            ),
            "RRXls4YMUkWzfP2aRBwciMfd_bvsPv6kE5GP0SDj0tM.jpg",
        )

    def test_migration_copies_old_poster_to_new_canonical_id(self):
        mapping = {
            "/manga/20th-century-boyss.kwj4": "/title/kwj4-20th-century-boys"
        }
        with tempfile.TemporaryDirectory() as directory:
            poster_dir = Path(directory)
            old_path = poster_dir / MIGRATION.poster_cache_filename(
                "multi.mangafire", next(iter(mapping))
            )
            old_path.write_bytes(b"valid cached poster")

            result = MIGRATION.migrate_poster_cache(poster_dir, mapping, apply=True)

            new_path = poster_dir / MIGRATION.poster_cache_filename(
                "multi.mangafire", next(iter(mapping.values()))
            )
            self.assertEqual(new_path.read_bytes(), b"valid cached poster")
            self.assertTrue(old_path.exists(), "rollback cache must be retained")
            self.assertEqual(result.copied, 1)
            self.assertEqual(result.missing, 0)

    def test_migration_refuses_to_overwrite_different_new_poster(self):
        mapping = {
            "/manga/20th-century-boyss.kwj4": "/title/kwj4-20th-century-boys"
        }
        with tempfile.TemporaryDirectory() as directory:
            poster_dir = Path(directory)
            old_id, new_id = next(iter(mapping.items()))
            (poster_dir / MIGRATION.poster_cache_filename("multi.mangafire", old_id)).write_bytes(
                b"old poster"
            )
            (poster_dir / MIGRATION.poster_cache_filename("multi.mangafire", new_id)).write_bytes(
                b"different new poster"
            )

            with self.assertRaises(MIGRATION.PosterConflictError):
                MIGRATION.migrate_poster_cache(poster_dir, mapping, apply=True)


class DatabaseMigrationTests(unittest.TestCase):
    def test_migration_includes_wal_state_and_emits_one_self_contained_database(self):
        mapping = {"/manga/old.hash": "/title/hash-new"}
        with tempfile.TemporaryDirectory() as directory:
            directory = Path(directory)
            input_db = directory / "input.db"
            output_db = directory / "output.db"

            connection = sqlite3.connect(input_db)
            connection.execute("PRAGMA journal_mode=WAL")
            connection.execute(
                "CREATE TABLE manga_library ("
                "source_id TEXT NOT NULL, manga_id TEXT NOT NULL, "
                "PRIMARY KEY (source_id, manga_id))"
            )
            connection.execute(
                "INSERT INTO manga_library VALUES (?, ?)",
                ("multi.mangafire", "/manga/old.hash"),
            )
            connection.commit()

            plan, total = MIGRATION.migrate_database(
                input_db, output_db, mapping, apply=True
            )
            connection.close()

            migrated = sqlite3.connect(f"file:{output_db.as_posix()}?mode=ro", uri=True)
            self.assertEqual(
                migrated.execute("SELECT manga_id FROM manga_library").fetchone()[0],
                "/title/hash-new",
            )
            self.assertEqual(migrated.execute("PRAGMA integrity_check").fetchone()[0], "ok")
            self.assertEqual(migrated.execute("PRAGMA journal_mode").fetchone()[0], "delete")
            migrated.close()
            self.assertEqual(plan, [("manga_library", 1)])
            self.assertEqual(total, 1)
            self.assertFalse(Path(f"{output_db}-wal").exists())
            self.assertFalse(Path(f"{output_db}-shm").exists())


if __name__ == "__main__":
    unittest.main()
