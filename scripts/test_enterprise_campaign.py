import unittest
import json
import local_sqlite as sqlite3
import tempfile
from pathlib import Path
from unittest.mock import patch
from enterprise_campaign import task_cases, reference, native_children
import enterprise_campaign as campaign


class BankingDatasetTests(unittest.TestCase):
    def test_triage_packet_binds_original_epoch_and_input_without_a_queue_answer(self):
        import hashlib
        canonical=lambda v:hashlib.sha256(json.dumps(v,sort_keys=True,separators=(",",":"),ensure_ascii=False).encode()).hexdigest()
        original={"case_id":"fixture-case","status":"mismatch","difference_cents":-5}
        binding={"run_id":"fixture-run","recipient":"triage","sender":"reconciliation","correlation_key":"case_id","correlation_value":"fixture-case","source_sha256":"s"*64,"accepted_record_sha256":canonical(original),"policy_revision":2}
        packet={"case_id":"fixture-case","retrieved_source":{"record":original},"evidence_policy_revision":2,"routes":{"invalid_currency":"data_quality","duplicate":"data_quality","missing":"data_quality","mismatch":"reconciliation","matched":"none"},"source_binding":{"source_task_sha256":"t"*64,"retry_after_validation_failure":False}}
        decision={"case_id":"fixture-case","input":packet,"task_input_sha256":canonical(packet)}
        self.assertEqual(campaign.verify_triage_packet(binding,decision,original,"fixture-run",2,"t"*64),canonical(packet))
        for mutate in (lambda d:d["input"].update(queue="reconciliation"),lambda d:d["input"].update(status="mismatch"),lambda d:d["input"].update(evidence_policy_revision=3),lambda d:d["input"]["retrieved_source"]["record"].update(status="matched"),lambda d:d.update(task_input_sha256="wrong")):
            attacked=json.loads(json.dumps(decision));mutate(attacked)
            with self.assertRaises(AssertionError):campaign.verify_triage_packet(binding,attacked,original,"fixture-run",2,"t"*64)
    def test_release_interpreter_does_not_repair_a_wrong_model_policy(self):
        rules=[{"when":"outside_scope","release":"deny"},{"when":"any_failed","release":"deny"},{"when":"always","release":"allow_synthetic_evaluation"}]
        program={"case_id":"risk-policy","rules":rules}
        self.assertEqual(campaign.verify_release_policy(program,["qa","memory","tools"]),24)
        wrong=json.loads(json.dumps(program));wrong["rules"][0]["release"]="allow_synthetic_evaluation"
        self.assertEqual(campaign.execute_release_policy(wrong,"production",{"qa":True})["release"],"allow_synthetic_evaluation")
        with self.assertRaises(AssertionError):campaign.verify_release_policy(wrong,["qa","memory","tools"])
        wrong["rules"][0]["when"]="execute_shell"
        with self.assertRaises(AssertionError):campaign.execute_release_policy(wrong,"production",{"qa":True})
    def test_live_revocation_restore_is_scoped_and_fails_closed_on_stale_outputs_or_live_cache(self):
        for failure in (None,"persisted-output","live-cache","concurrent-policy"):
            with self.subTest(failure=failure),tempfile.TemporaryDirectory() as directory:
                root=Path(directory);folder=root/"data/evaluation";folder.mkdir(parents=True)
                brain=root/"data/companies/one/brain.sqlite3";brain.parent.mkdir(parents=True)
                grants={"master":{"memory_from":[]},"intake":{"memory_from":["master"]},"schema":{"memory_from":["master","intake"]}}
                policy={"revision":2,"grants":grants,"ceilings":grants,"master":"master"}
                with sqlite3.connect(brain) as db:
                    db.executescript("CREATE TABLE run_governance(run_id TEXT,manifest TEXT);CREATE TABLE workflow_nodes(run_id TEXT,id TEXT,state TEXT,attempt INTEGER,output TEXT,output_sha256 TEXT);CREATE TABLE context(run_id TEXT,agent TEXT,source TEXT,body TEXT);CREATE TABLE context_archive(run_id TEXT,agent TEXT,source TEXT,body TEXT);")
                    db.execute("INSERT INTO run_governance VALUES('run',?)",(json.dumps(policy),))
                    db.executemany("INSERT INTO workflow_nodes VALUES('run',?,'completed',1,'fixture',?)",[("master","hash-master"),("intake","hash-intake")])
                    db.execute("INSERT INTO workflow_nodes VALUES('run','schema','running',1,NULL,NULL)")
                    if failure=="persisted-output":db.execute("INSERT INTO context VALUES('run','schema','model_output','fixture')")
                operations=[];requests=[]
                def op(kind,**fields):
                    operations.append((kind,fields))
                    with sqlite3.connect(brain) as db:
                        current=json.loads(db.execute("SELECT manifest FROM run_governance").fetchone()[0])
                        self.assertEqual(fields["expected_revision"],current["revision"])
                        db.execute("UPDATE run_governance SET manifest=?",(json.dumps(fields["policy"]),))
                def request(path,body=None):
                    requests.append(path)
                    if failure=="concurrent-policy" and path.endswith("/run"):
                        with sqlite3.connect(brain) as db:
                            changed={**policy,"revision":99};db.execute("UPDATE run_governance SET manifest=?",(json.dumps(changed),))
                    return {"status":"failed","events":[{"kind":"policy_epoch_cancelled","call_id":"active"}]}
                before={"events":[{"agent":"schema","kind":"call_started","call_id":"active"}]}
                with patch.object(campaign,"ROOT",root),patch.object(campaign,"native_children",return_value={7} if failure=="live-cache" else set()):
                    if failure:
                        with self.assertRaises(AssertionError):campaign.exercise_revocation(folder,"one","run","schema",before,{7},op,request)
                        self.assertEqual(len(operations),1)
                        self.assertFalse(any(path.endswith("/resume") for path in requests))
                    else:
                        result=campaign.exercise_revocation(folder,"one","run","schema",before,{7},op,request)
                        self.assertTrue(result["verified"])
                        self.assertEqual(result["restored_revision"],4)
                        self.assertEqual(operations[0][1]["policy"]["grants"]["schema"]["memory_from"],[])
                        self.assertEqual(operations[1][1]["policy"]["grants"],policy["grants"])
                        self.assertTrue(requests[-1].endswith("/resume"))

    def test_fault_injection_targets_only_direct_owned_native_children(self):
        processes="41 10 /opt/llama-server\n42 11 /opt/llama-server\n43 10 /opt/other\n44 10 /opt/llama-server\n"
        with patch("enterprise_campaign.subprocess.check_output",return_value=processes) as inspected:
            self.assertEqual(native_children(10),{41,44})
            self.assertNotIn("args",str(inspected.call_args))

    def test_challenge_layout_breaks_case_id_to_status_correlation_without_changing_case_coverage(self):
        original=task_cases(12419)
        challenge=task_cases(12419,True)
        self.assertEqual(len({c["case_id"] for c in challenge}),10)
        self.assertEqual(sorted(reference(c)["status"] for c in original),sorted(reference(c)["status"] for c in challenge))
        self.assertNotEqual([(c["case_id"],reference(c)["status"]) for c in original],[(c["case_id"],reference(c)["status"]) for c in challenge])
        self.assertEqual(challenge,task_cases(12419,True))

    def test_empty_zero_is_missing_and_duplicate_credits_are_not_discarded(self):
        cases=task_cases(123)
        self.assertEqual(reference(cases[9])["status"],"missing")
        self.assertEqual(reference(cases[9])["difference_cents"],0)
        self.assertEqual(reference(cases[2])["observed_cents"],sum(p["cents"] for p in cases[2]["payments"]))
        self.assertEqual(reference(cases[3])["status"],"invalid_currency")


if __name__ == "__main__":
    unittest.main()
