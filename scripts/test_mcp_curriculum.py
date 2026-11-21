import hashlib
import importlib.util
import json
import tempfile
import unittest
import uuid
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

import local_sqlite as sqlite3

spec = importlib.util.spec_from_file_location(
    "curriculum_journal", Path(__file__).with_name("mcp-curriculum-journal.py")
)
assert spec is not None and spec.loader is not None
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)


def catalog(first="a" * 64):
    return [
        {
            **row,
            "fixture_sha": first
            if row["level"] == 1
            else (
                chr(ord("a") + row["level"] - 1) * 64
                if row["level"] <= 6
                else hashlib.sha256(row["id"].encode()).hexdigest()
            ),
        }
        for row in module.POLICY["levels"]
    ]


class CurriculumJournalTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.path = Path(self.temp.name).resolve() / "private/curriculum.sqlite3"
        self.journal = module.Journal(self.path)
        self.journal.operate({"op": "sync", "cases": catalog()})

    def tearDown(self):
        self.journal.db.close()
        self.temp.cleanup()

    def begin(self, case="reconciliation", digest="a" * 64, attempt=None):
        attempt = attempt or str(uuid.uuid4())
        self.journal.operate(
            {
                "op": "begin",
                "attempt": attempt,
                "case_id": case,
                "fixture_sha": digest,
                "origin": "executed",
                "lease_sha": hashlib.sha256(b"test-lease").hexdigest(),
            }
        )
        return attempt

    def finish(self, attempt, **changes):
        proof = {
            "fixture_sha": "a" * 64,
            "verified": True,
            "run": str(uuid.uuid4()),
            "provider_cost_usd": 0,
            "report_sha256": "c" * 64,
            "evidence_sha256": "d" * 64,
        }
        proof.update(changes)
        return self.journal.operate(
            {
                "op": "finish",
                "attempt": attempt,
                "lease": "test-lease",
                "state": "pass",
                "proof": proof,
            }
        )

    def test_prerequisites_current_fixture_and_terminal_immutability(self):
        with self.assertRaises(ValueError):
            self.begin("reconciliation-edges", "b" * 64)
        attempt = self.begin()
        result = self.finish(attempt)
        self.assertTrue(result["cases"][0]["qualified_fixture"])
        with self.assertRaises(ValueError):
            self.finish(attempt)
        advanced = self.begin("reconciliation-edges", "b" * 64)
        self.finish(advanced, fixture_sha="b" * 64)
        changed = self.journal.operate({"op": "sync", "cases": catalog("e" * 64)})
        self.assertFalse(changed["cases"][0]["qualified_fixture"])
        self.assertFalse(changed["cases"][1]["eligible"])
        self.assertFalse(
            changed["cases"][2]["eligible"],
            "stale transitive prerequisite cannot be skipped",
        )
        self.assertEqual(changed["attempt_count"], 2)

    def test_wrong_lease_unverified_nonzero_cost_and_stale_proof_cannot_qualify(self):
        attempt = self.begin()
        with self.assertRaises(ValueError):
            self.journal.operate(
                {
                    "op": "finish",
                    "attempt": attempt,
                    "lease": "wrong",
                    "state": "pass",
                    "proof": {},
                }
            )
        for change in [
            {"verified": False},
            {"provider_cost_usd": 1},
            {"provider_cost_usd": None},
            {"provider_cost_usd": False},
            {"fixture_sha": "f" * 64},
            {"evidence_sha256": ""},
        ]:
            with self.assertRaises(ValueError):
                self.finish(attempt, **change)
        self.assertFalse(self.journal.status()["cases"][0]["qualified_fixture"])
        self.assertEqual(self.journal.status()["active"]["id"], attempt)
        failed = self.journal.operate(
            {
                "op": "finish",
                "attempt": attempt,
                "lease": "test-lease",
                "state": "fail",
                "proof": {
                    "fixture_sha": "a" * 64,
                    "reason": "SIMULATED terminal fixture failure",
                },
            }
        )
        self.assertIsNone(failed["active"])
        self.assertFalse(failed["cases"][0]["qualified_fixture"])
        self.assertEqual(failed["attempts"][0]["state"], "fail")

    def test_single_active_attempt_survives_reopen_and_concurrent_begin(self):
        self.journal.db.close()

        def compete(_):
            db = module.Journal(self.path)
            try:
                db.operate(
                    {
                        "op": "begin",
                        "attempt": str(uuid.uuid4()),
                        "case_id": "reconciliation",
                        "fixture_sha": "a" * 64,
                        "origin": "executed",
                        "lease_sha": "e" * 64,
                    }
                )
                return True
            except sqlite3.IntegrityError:
                return False
            finally:
                db.db.close()

        with ThreadPoolExecutor(max_workers=2) as pool:
            outcomes = list(pool.map(compete, range(2)))
        self.assertEqual(sum(outcomes), 1)
        self.journal = module.Journal(self.path)
        self.assertEqual(self.journal.status()["attempt_count"], 1)
        self.assertIsNotNone(self.journal.status()["active"])
        self.assertEqual(self.path.stat().st_mode & 0o777, 0o600)

    def test_native_authority_checks_owner_digest_and_draft_state(self):
        root = Path(self.temp.name).resolve()
        folder = root / "data/companies/fixture"
        folder.mkdir(parents=True)
        run = str(uuid.uuid4())
        db = sqlite3.connect(folder / "brain.sqlite3")
        db.executescript(
            "CREATE TABLE company(owner TEXT);INSERT INTO company VALUES('owner');CREATE TABLE jobs(run_id TEXT,digest TEXT,approval TEXT,intake TEXT,state TEXT);CREATE TABLE run_governance(run_id TEXT,revision INTEGER,manifest TEXT);CREATE TABLE audit(seq INTEGER,run_id TEXT,kind TEXT,body TEXT);"
        )
        db.execute(
            "INSERT INTO jobs VALUES(?,?,?,?,?)",
            (
                run,
                "hash",
                json.dumps({"owner": "owner", "sha256": "hash"}),
                "{}",
                "approved",
            ),
        )
        db.execute("INSERT INTO run_governance VALUES(?,1,'{}')", (run,))
        db.commit()
        self.assertEqual(
            module.native_authority(root, "fixture", run)["digest"], "hash"
        )
        db.execute("UPDATE jobs SET state='draft'")
        db.commit()
        with self.assertRaises(ValueError):
            module.native_authority(root, "fixture", run)
        db.execute(
            "UPDATE jobs SET state='approved',approval=?",
            (json.dumps({"owner": "other", "sha256": "hash"}),),
        )
        db.commit()
        with self.assertRaises(ValueError):
            module.native_authority(root, "fixture", run)
        db.close()


class PlanningContinuationAuthorityTests(unittest.TestCase):
    def test_only_terminal_unissued_canonical_checkpoints_are_read_without_mutation(
        self,
    ):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            path = root / "data/companies/fixture/brain.sqlite3"
            path.parent.mkdir(parents=True)
            session = str(uuid.uuid4())
            body = {
                "id": session,
                "revision": 3,
                "status": "failed",
                "error": "Interrupted turn; not a protocol error",
            }
            with sqlite3.connect(path) as db:
                db.executescript(
                    "CREATE TABLE ceo_planning(id TEXT,revision INTEGER,state TEXT,body TEXT);CREATE TABLE jobs(intake TEXT)"
                )
                db.execute(
                    "INSERT INTO ceo_planning VALUES(?,3,'failed',?)",
                    (session, json.dumps(body)),
                )
                db.commit()
                before = hashlib.sha256(path.read_bytes()).hexdigest()
                self.assertEqual(
                    module.planning_authority(root, "fixture", session),
                    {"planning": body},
                )
                self.assertEqual(hashlib.sha256(path.read_bytes()).hexdigest(), before)
                for field, value in [
                    ("status", "ready"),
                    ("revision", 4),
                    ("id", str(uuid.uuid4())),
                ]:
                    db.execute(
                        "UPDATE ceo_planning SET body=?",
                        (json.dumps({**body, field: value}),),
                    )
                    db.commit()
                    with self.assertRaises(ValueError):
                        module.planning_authority(root, "fixture", session)
                db.execute(
                    "UPDATE ceo_planning SET body=?,state='generating'",
                    (json.dumps(body),),
                )
                db.commit()
                with self.assertRaises(ValueError):
                    module.planning_authority(root, "fixture", session)
                db.execute("UPDATE ceo_planning SET state='failed'")
                db.execute(
                    "INSERT INTO jobs VALUES(?)",
                    (json.dumps({"ceo_planning_id": session}),),
                )
                db.commit()
                with self.assertRaises(ValueError):
                    module.planning_authority(root, "fixture", session)
            with self.assertRaises(ValueError):
                module.planning_authority(root, "../fixture", session)


if __name__ == "__main__":
    unittest.main()
