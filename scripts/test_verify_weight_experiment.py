import json
from pathlib import Path
import tempfile
import unittest
from train_role import digest
from weight_task_evaluation import evaluate_tasks,compare
from verify_weight_experiment import verify


class EvidenceTests(unittest.TestCase):
    def setUp(self):
        self.temp=tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root=Path(self.temp.name)
        self.work=self.root/"run"; self.work.mkdir()
        self.dataset=self.root/"data/companies/test/training/datasets/trial"
        self.dataset.mkdir(parents=True)
        (self.root/"models/base").mkdir(parents=True)
        (self.root/"models/base/model.safetensors").write_bytes(b"fixture base, not a real model")
        (self.work/"adapter").mkdir()
        (self.work/"adapter/adapters.safetensors").write_bytes(b"fixture adapter")
        self.rows=[{"messages":[{"role":"user","content":"Return ok"},{"role":"assistant","content":"ok"}]}]
        for split in ["train","valid","test"]:
            (self.dataset/f"{split}.jsonl").write_text(json.dumps(self.rows[0]))
        result=evaluate_tasks(self.rows,lambda _:"ok")
        for name in ["task-baseline","task-candidate","task-reloaded"]:
            self.write(name+".json",result)
        comparison=compare(result,result)
        self.write("task-comparison.json",comparison)
        self.write("manifest.json",{"repository_root":str(self.root),"base_model":"models/base","company_id":"test","training_id":"trial",
                                    "dataset_sha256":{s:digest(self.dataset/f"{s}.jsonl") for s in ["train","valid","test"]}})
        self.write("experiment.json",{"source_sha256":{}})
        self.write("metrics.json",{"base_files":{"model.safetensors":digest(self.root/"models/base/model.safetensors")},
                                   "adapter_sha256":digest(self.work/"adapter/adapters.safetensors"),
                                   "persisted_adapter_reload_matches":True,"task_comparison":comparison,
                                   "iterations":1,"elapsed_seconds":1,"peak_memory_gb":1})

    def write(self,name,value):
        (self.work/name).write_text(json.dumps(value))

    def test_complete_fixture_verifies(self):
        self.assertTrue(verify(self.work)["persisted_adapter_reload_matches"])

    def test_changed_base_rejected(self):
        (self.root/"models/base/model.safetensors").write_bytes(b"changed")
        with self.assertRaisesRegex(ValueError,"base changed"):
            verify(self.work)

    def test_changed_oracle_rejected_even_if_score_is_recomputed(self):
        data=json.loads((self.work/"task-candidate.json").read_text())
        data["cases"][0].update(output="wrong",expected="wrong")
        self.write("task-candidate.json",data)
        with self.assertRaisesRegex(ValueError,"oracle changed"):
            verify(self.work)

    def test_reload_output_difference_rejected_even_if_both_pass(self):
        data=json.loads((self.work/"task-reloaded.json").read_text())
        data["cases"][0]["output"]="ok "
        self.write("task-reloaded.json",data)
        with self.assertRaisesRegex(ValueError,"reload output mismatch"):
            verify(self.work)
