"""Independent read-only verifier for the one-writer synthetic-note trial.
Checks actual canonical rows, native packets, model bytes, lifecycle and a
private exact-text oracle. It does not judge arbitrary tasks or certify models.
"""
import argparse,hashlib,json,sqlite3
from pathlib import Path
p=argparse.ArgumentParser(description=__doc__);p.add_argument('trial',type=Path);a=p.parse_args()
ROOT=Path(__file__).resolve().parents[1];folder=a.trial.resolve();freeze=json.loads((folder/'freeze.json').read_text())
sha=lambda b:hashlib.sha256(b).hexdigest()
encode=lambda v:json.dumps(v,sort_keys=True,separators=(',',':'),ensure_ascii=False).encode()
run=freeze['run'];company=freeze['company'];assert company.startswith('architecture-native-')
with sqlite3.connect(f'file:{ROOT}/data/companies/{company}/brain.sqlite3?mode=ro',uri=True) as db:
 db.row_factory=sqlite3.Row
 job=db.execute('SELECT * FROM jobs WHERE run_id=?',(run,)).fetchone();assert job['state']=='completed'
 intake=json.loads(job['intake']);approval=json.loads(job['approval']);assert sha(job['intake'].encode())==job['digest']==approval['sha256']
 programrow=db.execute('SELECT * FROM coordination_programs WHERE run_id=?',(run,)).fetchone();program=json.loads(programrow['contract'])
 assert sha(programrow['contract'].encode())==programrow['sha256'] and program['coordination']=='ordered' and len(program['roles'])==1
 assert program==intake['coordination_contract'] and program['objective']==intake['idea']
 policyrow=db.execute('SELECT * FROM run_governance WHERE run_id=?',(run,)).fetchone();policy=json.loads(policyrow['manifest']);assert policyrow['revision']==programrow['policy_revision']==policy['revision']
 turns=list(db.execute('SELECT * FROM coordination_turns WHERE run_id=? ORDER BY ordinal',(run,)));assert len(turns)==1
 turn=turns[0];message=json.loads(turn['message']);receipt=json.loads(turn['receipt']);role=program['roles'][0]
 assert turn['state']=='completed' and turn['ordinal']==0 and message['turn']['actor']==role['actor'] and message['turn']['assignment']==role['assignment'] and message['turn']['purpose']=='contribute' and message['turn']['sources']==[] and message['lineage']==[]
 assert json.loads(receipt['text'])==message['content']
 assert sha(encode({'message':turn['message'],'receipt':turn['receipt']}))==turn['sha256']
 packet={'objective':program['objective'],'definition_of_done':program['definition_of_done'],'input':program['input'],'assignment':role['assignment'],'purpose':'contribute','roles':program['roles'],'sources':[],'delegated_turns':0,'max_delegations':program['max_delegations'],'task_accepted':False,'tools_available':[],'contract_version':2,'evidence_references':['input:'+sha(encode(program['input']))]}
 assert sha(encode(packet))==turn['packet_sha256']==receipt['local_execution']['task_input_sha256']
 reviewrow=db.execute('SELECT * FROM coordination_reviews WHERE run_id=?',(run,)).fetchone();assert reviewrow['state']=='received'
 snapshot=json.loads(reviewrow['snapshot']);reviewreply=json.loads(reviewrow['receipt']);review=json.loads(reviewreply['text'])
 assert sha(reviewrow['snapshot'].encode())==reviewrow['snapshot_sha256'] and sha(reviewrow['receipt'].encode())==reviewrow['receipt_sha256']
 assert snapshot['intake_sha256']==job['digest'] and snapshot['program_sha256']==programrow['sha256'] and snapshot['policy_revision']==policyrow['revision'] and snapshot['policy_sha256']==sha(policyrow['manifest'].encode())
 assert snapshot['history_sha256']==sha(encode({'messages':[message],'native_checkpoints':[turn['sha256']]}))
 assert snapshot['packet']['candidate']==message['content'] and snapshot['packet']['input']==program['input'] and snapshot['packet']['criteria']=={str(i):s for i,s in enumerate(program['definition_of_done'])}
 assert snapshot['packet']['candidate_sha256']==sha(turn['message'].encode())
 assert sha(encode(snapshot['packet']))==reviewreply['local_execution']['task_input_sha256']
 assert review['verdict']=='accepted' and set(review['checks'])=={str(i) for i in range(len(program['definition_of_done']))} and all(c['passed'] is True and c['evidence'].strip() for c in review['checks'].values())
 for r in [receipt,reviewreply]:
  native=r['local_execution'];assert native['finish_reason']=='stop' and native['backend_pid']>0 and native['port']>0 and native['received_at_ms']>=native['dispatched_at_ms']
  assert r['usage']['cost_usd']==0 and isinstance(r['usage']['input'],int) and isinstance(r['usage']['output'],int)
 assert intake['reasoning_acceptance']['reviewer']['sha256']!=role['model_sha256']
 prepared=json.loads((folder/'prepared.json').read_text());catalog=prepared['architecture_assessment']['models']
 # The runtime verifies worker bytes. Independently hash the installed owner
 # configuration/pinned reviewer used by this frozen one-writer trial as well.
 config=json.loads((ROOT/'config/company.mcp-development-reasoning-repair.json').read_text())
 worker=next(x for x in config['agents'] if x['id']==role['actor'])['worker'];judge=intake['reasoning_acceptance']['reviewer']
 for model,pin in [(worker,role['model_sha256']),(judge['model'],judge['sha256'])]:
  path=(ROOT/model['path']).resolve();assert path.is_relative_to((ROOT/'models').resolve())
  with path.open('rb') as f:assert hashlib.file_digest(f,'sha256').hexdigest()==pin
 actor=role['actor'];assert policy['grants'][actor]['tools']==[] and policy['grants'][actor]['skills']==[] and policy['grants'][actor]['memory_from']==[]
 checkpoints=list(db.execute("SELECT * FROM memory_archive_checkpoints WHERE run_id=? AND lifecycle='completed'",(run,)))
 assert {x['agent'] for x in checkpoints}=={actor} and all(x['contract_sha256']==job['digest'] and x['policy_revision']==policyrow['revision'] and x['record_count']>0 for x in checkpoints)
 assert db.execute("SELECT COUNT(*) FROM context_archive WHERE run_id=? AND agent=? AND tier='ALTC'",(run,actor)).fetchone()[0]>0
oracle=json.loads((folder/'private-oracle.json').read_text());text=message['content']['deliverable'];assert ' '.join(text.split()).lower()==oracle['expected_exact_normalized_text']
report={'status':'PASS','run':run,'company':company,'canonical_approval_program_worker_and_review_packets_verified':True,'actual_pinned_weight_bytes_verified':True,'private_exact_text_oracle_passed':True,'completed_agent_memory_checkpoints':len(checkpoints),'provider_fees_usd':0,'scope':'Independent read-only verification of one actual local one-writer synthetic-note task continued from prior real CEO planning. Not arbitrary task, tool execution, three-pattern or compliance qualification.'}
(folder/'independent-verification.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(report))
