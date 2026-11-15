import json
import unittest

from completed_memory_proof import completed_archive
import local_sqlite

RUN = "00000000-0000-0000-0000-000000000001"


def fixture():
    db = local_sqlite.connect(":memory:")
    db.executescript("""
      CREATE TABLE jobs(run_id,digest,state);
      CREATE TABLE run_governance(run_id,revision);
      CREATE TABLE context(run_id);
      CREATE TABLE context_archive(run_id,agent,id,tier,source,body,created);
      CREATE TABLE memory_archive_checkpoints(run_id,agent,lifecycle,policy_revision,contract_sha256,record_count,records_sha256);
    """)
    db.execute("INSERT INTO jobs VALUES(?,?,'completed')", (RUN, "c" * 64))
    db.execute("INSERT INTO run_governance VALUES(?,1)", (RUN,))
    db.execute("INSERT INTO context_archive VALUES(?,'A','r','ALTC','fixture','private fixture body',17)", (RUN,))
    db.execute("INSERT INTO memory_archive_checkpoints VALUES(?,'A','completed',1,?,1,?)", (RUN, "c" * 64, "6beb3d355afe251c9cf9d733bbed7c66f9c50dc6105a879ef9ad8b9812c244ce"))
    db.commit()
    db.execute("BEGIN")
    return db


class CompletedMemoryProofTests(unittest.TestCase):
    def test_complete_archive_proof_contains_no_payload_or_source(self):
        with fixture() as db:
            proof = completed_archive(db, RUN)
            self.assertEqual((proof["status"], proof["actors"], proof["cold_records"], proof["hot_records"]), ("PASS", 1, 1, 0))
            self.assertNotIn("private fixture body", json.dumps(proof))
            self.assertNotIn("fixture", json.dumps(proof))

    def test_tampered_archive_or_wrong_authority_never_passes(self):
        for mutation in [
            "UPDATE jobs SET state='approved'",
            "UPDATE jobs SET digest=NULL",
            "DELETE FROM run_governance",
            "INSERT INTO context VALUES('" + RUN + "')",
            "DELETE FROM context_archive",
            "UPDATE context_archive SET body='changed'",
            "UPDATE context_archive SET tier='PLTC'",
            "UPDATE context_archive SET body=hex(zeroblob(600000))",
            "UPDATE memory_archive_checkpoints SET policy_revision=2",
            "UPDATE memory_archive_checkpoints SET contract_sha256='wrong'",
            "UPDATE memory_archive_checkpoints SET record_count=2",
            "UPDATE memory_archive_checkpoints SET agent='B'",
            "DELETE FROM memory_archive_checkpoints",
        ]:
            with self.subTest(mutation=mutation), fixture() as db:
                db.execute(mutation)
                with self.assertRaises(ValueError):
                    completed_archive(db, RUN)

    def test_requires_consistent_transaction_and_canonical_run_identity(self):
        with fixture() as db:
            for run in ["../escape", "not-a-uuid", "00000000000000000000000000000001"]:
                with self.assertRaises(ValueError):
                    completed_archive(db, run)
            db.commit()
            with self.assertRaises(ValueError):
                completed_archive(db, RUN)


if __name__ == "__main__":
    unittest.main()
