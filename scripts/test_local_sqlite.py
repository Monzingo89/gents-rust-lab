"""Actual patched SQLite IPC, transactions and injected acknowledgement loss."""

import os
import queue
import subprocess
import sys
import tempfile
import unittest
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path
from unittest.mock import patch

import local_sqlite as sql


class OwnedSQLiteTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.path = Path(self.temp.name) / "private ü.sqlite3"

    def tearDown(self):
        self.temp.cleanup()

    def test_patched_runtime_scalars_blobs_row_factory_and_cursor_iteration(self):
        with sql.connect(self.path) as db:
            self.assertGreaterEqual(
                tuple(map(int, db.runtime_info["sqlite_version"].split("."))),
                (3, 54, 0),
            )
            db.executescript(
                "CREATE TABLE values_(id INTEGER PRIMARY KEY,v TEXT,b BLOB,n INTEGER,r REAL);"
            )
            cursor = db.execute(
                "INSERT INTO values_(v,b,n,r) VALUES(?,?,?,?)",
                ("Unicode 尾\n", b"\x00\xff", -(2**63), 1.25),
            )
            self.assertEqual(cursor.lastrowid, 1)
            self.assertEqual(cursor.rowcount, 1)
            self.assertTrue(db.in_transaction)
            db.row_factory = sql.Row
            row = db.execute("SELECT * FROM values_").fetchone()
            self.assertEqual(row["V"], "Unicode 尾\n")
            self.assertEqual(row[2], b"\x00\xff")
            self.assertEqual(dict(row)["n"], -(2**63))
            self.assertEqual(row["r"], 1.25)
            self.assertEqual(next(iter(db.execute("SELECT id FROM values_")))[0], 1)
        db.close()
        self.assertEqual(self.path.stat().st_mode & 0o777, 0o600)

    def test_rollback_constraints_readonly_uri_and_single_statement(self):
        db = sql.connect(self.path)
        db.execute("CREATE TABLE t(v INTEGER UNIQUE)")
        with self.assertRaises(ValueError), db:
            db.execute("INSERT INTO t VALUES(?)", (1,))
            raise ValueError("synthetic rollback")
        self.assertEqual(db.execute("SELECT COUNT(*) FROM t").fetchone(), (0,))
        with db:
            db.execute("INSERT INTO t VALUES(1)")
        with self.assertRaises(sql.IntegrityError):
            db.execute("INSERT INTO t VALUES(1)")
        db.rollback()
        with self.assertRaises(sql.DatabaseError):
            db.execute("SELECT 1; SELECT 2")
        db.close()
        with sql.connect(self.path.as_uri() + "?mode=ro", uri=True) as readonly:
            self.assertEqual(readonly.execute("SELECT v FROM t").fetchall(), [(1,)])
            with self.assertRaises(sql.OperationalError):
                readonly.execute("INSERT INTO t VALUES(2)")
        readonly.close()

    def test_explicit_transactions_autocommit_and_cross_connection_locking(self):
        left = sql.connect(self.path, isolation_level=None)
        right = sql.connect(self.path, timeout=0.01, isolation_level=None)
        left.execute("CREATE TABLE t(v INTEGER)")
        left.execute("BEGIN IMMEDIATE")
        left.execute("INSERT INTO t VALUES(7)")
        with self.assertRaises(sql.OperationalError):
            right.execute("INSERT INTO t VALUES(8)")
        left.rollback()
        right.execute("INSERT INTO t VALUES(9)")
        self.assertEqual(left.execute("SELECT v FROM t").fetchall(), [(9,)])
        left.close()
        right.close()

    def test_bulk_dml_is_transactional_and_unsupported_bulk_sql_has_no_effect(self):
        db = sql.connect(self.path)
        db.execute("CREATE TABLE t(v INTEGER UNIQUE)")
        cursor = db.executemany("INSERT INTO t VALUES(?)", [(1,), (2,)])
        self.assertEqual(cursor.rowcount, 2)
        self.assertIsNone(cursor.lastrowid)
        db.rollback()
        self.assertEqual(db.execute("SELECT COUNT(*) FROM t").fetchone()[0], 0)
        with self.assertRaises(sql.DatabaseError):
            db.executemany("DROP TABLE t", [()])
        with self.assertRaises(sql.DatabaseError):
            db.executemany("SELECT v FROM t", [])
        self.assertEqual(db.execute("SELECT COUNT(*) FROM t").fetchone()[0], 0)
        cursor = db.execute("SELECT 1")
        cursor.close()
        with self.assertRaises(sql.ProgrammingError):
            cursor.fetchone()
        db.close()

    def test_lost_commit_ack_never_reconnects_or_replays_an_effect(self):
        db = sql.connect(self.path)
        db.execute("CREATE TABLE t(v INTEGER)")
        db.execute("INSERT INTO t VALUES(11)")
        original = db._responses.get

        def lost_ack(*args, **kwargs):
            original(
                *args, **kwargs
            )  # Native COMMIT completed; discard its actual acknowledgement.
            raise queue.Empty

        with (
            patch.object(db._responses, "get", side_effect=lost_ack),
            self.assertRaisesRegex(sql.OperationalError, "uncertain"),
        ):
            db.commit()
        with self.assertRaises(sql.ProgrammingError):
            db.execute("INSERT INTO t VALUES(12)")
        with sql.connect(self.path) as observed:
            self.assertEqual(observed.execute("SELECT v FROM t").fetchall(), [(11,)])
        observed.close()

    def test_process_death_rolls_back_and_parameters_cannot_be_sql(self):
        db = sql.connect(self.path)
        db.execute("CREATE TABLE t(v TEXT)")
        db.execute("INSERT INTO t VALUES(?)", ("'); DROP TABLE t; --",))
        db.commit()
        db.execute("INSERT INTO t VALUES(?)", ("uncommitted",))
        db._process.kill()
        db._process.wait()
        with self.assertRaises(sql.OperationalError):
            db.commit()
        with sql.connect(self.path) as check:
            self.assertEqual(check.execute("SELECT COUNT(*) FROM t").fetchone()[0], 1)
        check.close()

    def test_concurrent_unique_lease_and_no_inherited_credentials_or_fallback(self):
        with sql.connect(self.path) as db:
            db.execute("CREATE TABLE lease(v INTEGER UNIQUE)")
        db.close()

        def admit(_):
            with sql.connect(self.path) as db:
                try:
                    db.execute("INSERT INTO lease VALUES(1)")
                except sql.IntegrityError:
                    return False
                return True

        with ThreadPoolExecutor(max_workers=2) as pool:
            self.assertEqual(sum(pool.map(admit, range(2))), 1)
        with patch.dict(
            os.environ,
            {
                "HF_TOKEN": "private",
                "OPENAI_API_KEY": "private",
                "PYTHONPATH": "private",
                "HTTP_PROXY": "private",
            },
        ):
            env = sql._environment()
            self.assertFalse(
                set(env) & {"HF_TOKEN", "OPENAI_API_KEY", "PYTHONPATH", "HTTP_PROXY"}
            )
        with (
            patch.dict(
                os.environ,
                {"GENTS_SQLITE_BRIDGE": str(Path(self.temp.name) / "missing")},
            ),
            self.assertRaisesRegex(sql.OperationalError, "missing"),
        ):
            sql.connect(self.path)
        for path in ["file://remote/db?mode=ro", "//remote/db"]:
            with self.assertRaises(sql.NotSupportedError):
                sql.connect(path, uri=path.startswith("file:"))

    def test_runtime_does_not_import_system_sqlite_or_load_extensions(self):
        code = "import sys;sys.path.insert(0,'scripts');import local_sqlite as s;c=s.connect(':memory:');assert c.execute('select sqlite_version()').fetchone()[0]==c.runtime_info['sqlite_version'];assert 'sqlite3' not in sys.modules and '_sqlite3' not in sys.modules;c.close()"
        process = subprocess.run(
            [sys.executable, "-c", code],
            cwd=Path(__file__).resolve().parents[1],
            capture_output=True,
            check=False,
        )
        self.assertEqual(process.returncode, 0)
        db = sql.connect(":memory:")
        with self.assertRaises(sql.OperationalError) as failure:
            db.execute("SELECT load_extension(?)", ("private-sentinel-not-loaded",))
        self.assertNotIn("private-sentinel", str(failure.exception))
        self.assertEqual(db.execute("SELECT 1").fetchone(), (1,))
        db.close()

    def test_leaf_symlink_and_sensitive_parse_error_are_not_admitted_or_echoed(self):
        with sql.connect(self.path) as db:
            db.execute("CREATE TABLE t(v INTEGER)")
        db.close()
        link = Path(self.temp.name) / "alias.db"
        link.symlink_to(self.path)
        with self.assertRaises(sql.OperationalError):
            sql.connect(link)
        db = sql.connect(self.path)
        with self.assertRaises(sql.DatabaseError) as failure:
            db.execute("SELECT 'private-sentinel-invalid")
        self.assertNotIn("private-sentinel", str(failure.exception))
        db.close()


if __name__ == "__main__":
    unittest.main()
