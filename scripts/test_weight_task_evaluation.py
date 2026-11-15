import unittest
from weight_task_evaluation import score, evaluate_tasks, compare


class TaskEvaluationTests(unittest.TestCase):
    def test_strict_outputs(self):
        self.assertTrue(score('{"b":2,"a":1}', '{"a":1,"b":2}'))
        for output in ['{"a":true}', '{"a":1,"a":1}', '```json\n{"a":1}\n```', '{"a":NaN}']:
            self.assertFalse(score(output, '{"a":1}'))

    def test_expected_answer_never_sent_to_inference(self):
        rows = [{"messages": [{"role":"user","content":"task"}, {"role":"assistant","content":"secret oracle"}]}]
        seen = []
        def infer(messages):
            seen.extend(messages)
            return "wrong"
        result = evaluate_tasks(rows, infer)
        self.assertEqual(seen, rows[0]["messages"][:-1])
        self.assertEqual(result["passed"], 0)

    def test_gain_cannot_hide_regression_or_changed_cases(self):
        before = {"cases":[{"prompt_sha256":str(i),"passed":i==0} for i in range(3)], "passed":1,"total":3}
        after = {"cases":[{"prompt_sha256":str(i),"passed":i!=0} for i in range(3)], "passed":2,"total":3}
        result = compare(before, after)
        self.assertEqual(result["regressions"], 1)
        self.assertFalse(result["measured_improvement"])
        after["cases"][0]["prompt_sha256"] = "changed"
        with self.assertRaises(ValueError):
            compare(before, after)


if __name__ == "__main__":
    unittest.main()
