import unittest
from weight_prompt_formats import CompletionDataset,inference_prompt,validate_format


class Tokenizer:
    eos_token_id=0
    def encode(self,text,add_special_tokens=False):return [ord(c) for c in text]


class PromptTests(unittest.TestCase):
    def test_training_prefix_matches_inference_and_answer_is_masked(self):
        row={'messages':[{'role':'user','content':'def add(a, b):\n'},{'role':'assistant','content':'    return a + b\n'}]}
        tokenizer=Tokenizer();dataset=CompletionDataset([row],tokenizer)
        tokens,offset=dataset.process(row)
        prompt=inference_prompt(tokenizer,row['messages'][:-1],'completion-v1')
        self.assertEqual(tokens[:offset],tokenizer.encode(prompt))
        self.assertEqual(tokens[offset:-1],tokenizer.encode(row['messages'][-1]['content']))
        self.assertEqual(tokens[-1],0)

    def test_invalid_modes_and_conversations_rejected(self):
        with self.assertRaises(ValueError):validate_format('automatic')
        with self.assertRaises(ValueError):inference_prompt(Tokenizer(),[{'role':'system','content':'x'}],'completion-v1')
        with self.assertRaises(ValueError):CompletionDataset([],Tokenizer()).process({'messages':[]})


if __name__=='__main__':unittest.main()
