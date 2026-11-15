"""Explicit prompt/data adapters for chat and base completion models."""


def validate_format(name):
    if name not in ('chat-v1','completion-v1'):
        raise ValueError('unsupported weight experiment prompt format')
    return name


def inference_prompt(tokenizer,messages,name):
    validate_format(name)
    if name=='chat-v1':
        return tokenizer.apply_chat_template(messages,tokenize=False,add_generation_prompt=True,enable_thinking=False)
    if len(messages)!=1 or messages[0]['role']!='user' or not isinstance(messages[0]['content'],str) or not messages[0]['content']:
        raise ValueError('completion-v1 requires one nonempty raw user prompt')
    return messages[0]['content']


class CompletionDataset:
    def __init__(self,rows,tokenizer):
        self.rows=rows
        self.tokenizer=tokenizer

    def __len__(self):return len(self.rows)

    def __getitem__(self,index):return self.rows[index]

    def process(self,row):
        messages=row['messages']
        if len(messages)!=2 or messages[-1]['role']!='assistant' or not messages[-1]['content']:
            raise ValueError('completion-v1 requires one prompt and one completion')
        prompt=inference_prompt(self.tokenizer,messages[:-1],'completion-v1')
        # Encode separately so inference has exactly the same prompt prefix.
        prefix=self.tokenizer.encode(prompt,add_special_tokens=False)
        suffix=self.tokenizer.encode(messages[-1]['content'],add_special_tokens=False)
        eos=self.tokenizer.eos_token_id
        if not prefix or not suffix or eos is None:
            raise ValueError('completion tokens and EOS are required')
        if suffix[-1]!=eos:suffix.append(eos)
        return prefix+suffix,len(prefix)


def datasets(rows,tokenizer,name):
    validate_format(name)
    if name=='completion-v1':
        return {split:CompletionDataset(items,tokenizer) for split,items in rows.items()}
    from mlx_lm.tuner.datasets import ChatDataset
    return {split:ChatDataset(items,tokenizer,mask_prompt=True) for split,items in rows.items()}
