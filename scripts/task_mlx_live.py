#!/usr/bin/env python3
"""One isolated, bounded MLX RAG protocol trial against existing local weights."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import sqlite3
import subprocess
import time
import urllib.request
import uuid
from task_engine_live import ROOT, port


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--model', default='models/gemma-4-E4B-it')
    parser.add_argument('--context-tokens', type=int, default=8192)
    parser.add_argument('--generation-tokens', type=int, default=768)
    parser.add_argument('--native', action='store_true')
    args = parser.parse_args()
    binary = ROOT / 'target/debug/gents-harness'
    # This command inventories and hashes existing inputs; it never loads weights.
    manifest = json.loads(subprocess.check_output([str(binary), '--model-package-pin', args.model], cwd=ROOT))
    run = str(uuid.uuid4())
    company = 'mlx-trial-' + run[:8]
    output = ROOT / 'data/task-mlx-live' / run
    output.mkdir(parents=True)
    (output / 'package-pin.json').write_text(json.dumps(manifest, indent=2) + '\n')
    expected = 'violet-' + run[:8]
    source_text = f'The synthetic observatory beacon has the recorded color code {expected}.'
    criteria = {'expected_exact_answer': expected, 'generative_calls': 1, 'source_quote_required': True,
                'tokenizer_matches_server_usage': True, 'production_admission': False}
    if args.native:criteria.update(transport='native MCP stdio',parent_call_allowance=1)
    (output / 'criteria.json').write_text(json.dumps(criteria, indent=2) + '\n')
    model = {'provider': 'mlx', 'model': 'Existing MLX bounded protocol trial', 'path': args.model, 'context_tokens': args.context_tokens}
    memory = {'weights_bytes': manifest['weight_bytes'], 'adapters_bytes': 0, 'kv_bytes': 2 * 1024**3,
              'context_bytes': 128 * 1024**2, 'runtime_bytes': 2 * 1024**3, 'headroom_bytes': 4 * 1024**3,
              'provenance': 'Conservative trial estimate; disk weights plus separate KV/context/runtime/system reservations. Not measured allocation.'}
    reservation = sum(v for k, v in memory.items() if k.endswith('_bytes'))
    # The scheduler accounts in MiB: capacity rounds down, requests round up.
    reservation = ((reservation + 1024**2 - 1) // 1024**2) * 1024**2
    worker = {'model': model, 'sha256': manifest['sha256'], 'memory_estimate': memory,
              'evaluated_profiles': ['general'], 'evaluation_reference': 'Isolated protocol trial only; no production qualification', 'max_input_bytes': 16000}
    config = {'company_mode': True, 'company_tools': True, 'admin_judge': model,
              'agents': [{'id': 'MLX trial', 'assignment': 'Bounded read-only trial', 'planner': model, 'worker': model}],
              'max_steps': 1, 'max_tokens': args.generation_tokens, 'max_run_seconds': 180,
              'task_workflows': {'models': {'trial': worker}, 'default_model': 'trial', 'max_calls': 1,
                                 'total_generation_tokens': args.generation_tokens, 'per_call_tokens': args.generation_tokens, 'deadline_seconds': 150,
                                 'max_retries': 0, 'max_pending_jobs': 1, 'concurrency': 1,
                                 'resident_budget_bytes': reservation, 'allow_parametric_answers': False}}
    configpath = output / 'config.json'
    if args.native:config['task_workflows'].update(native_mcp=True,native_companies=[company])
    configpath.write_text(json.dumps(config, indent=2) + '\n')
    origin = f'http://127.0.0.1:{port()}'
    modelport = port()

    def req(path, body=None):
        request = urllib.request.Request(origin + path, data=None if body is None else json.dumps(body).encode(),
                                         headers={'Content-Type': 'application/json'})
        with urllib.request.urlopen(request, timeout=20) as response:
            return json.load(response)

    env = {k: v for k, v in os.environ.items() if k in {'PATH', 'HOME', 'USER', 'TMPDIR', 'LANG', 'SHELL'}}
    env.update(AUTH_MODE='local', BIND=origin.removeprefix('http://'), LOCAL_MODEL_PORT=str(modelport),
               CONFIG=str(configpath), RUN_STORE=str(output / 'runs.sqlite3'), GENTS_DISABLE_SCHEDULER='1',
               GENTS_OFFICE_ACTIVITY='0', GENTS_LOCAL_ONLY='1')
    began = time.monotonic()
    with binary.open('rb') as stream:binary_sha=hashlib.file_digest(stream,'sha256').hexdigest()
    native_client=None
    with (output / 'harness.log').open('wb') as log:
        child = subprocess.Popen([str(binary)], cwd=ROOT, env=env, stdout=log, stderr=subprocess.STDOUT)
        try:
            deadline = time.monotonic() + 20
            while True:
                try:
                    req('/api/config')
                    break
                except OSError:
                    if child.poll() is not None or time.monotonic() > deadline:
                        raise
                    time.sleep(.1)
            req('/api/companies/' + company, {'op': 'register', 'name': 'Isolated MLX RAG protocol trial'})
            hardware = req('/api/companies/' + company + '/hardware')
            route = '/api/companies/' + company + '/tasks'
            now = int(time.time() * 1000)
            source={'source_id': 'synthetic-beacon', 'revision': run,
                'title': 'Synthetic beacon fixture', 'domain': 'general', 'language': 'en', 'text': source_text,
                'observed_at_ms': now, 'valid_until_ms': now + 600000, 'lookups': []}
            operation={'input':{'original_ask':'What is the recorded color code of the synthetic observatory beacon? Return only the exact code.'},'hints':{'template':'rag/v1','domain':'general','evidence_required':True}}
            if args.native:
                from task_native_smoke import Mcp
                parent=str(uuid.uuid4());company_route='/api/companies/'+company
                intake={'idea':'Isolated native MLX protocol trial','acceptance':'One source-cited exact synthetic literal','exclusions':'Paid services','timeline':'Bounded local trial','company_signer':'Synthetic fixture','provider_signer':'Synthetic fixture','budget_usd':0,'max_model_calls':1}
                state=req(company_route,{'op':'intake','run_id':parent,'intake':intake});job=next(j for j in state['jobs'] if j['run_id']==parent)
                req(company_route,{'op':'approve','run_id':parent,'digest':job['digest']})
                access={'tools':['execute_task','get_task_status','cancel_task','ingest_sources','list_capabilities','search_knowledge'],'skills':[],'memory_from':[]}
                req(company_route,{'op':'governance_install','run_id':parent,'expected_revision':0,'policy':{'revision':1,'master':'Worker','ceilings':{'Worker':access},'grants':{'Worker':access}}})
                native_client=Mcp(ROOT,parent,company=company)
                lease_deadline=time.monotonic()+10
                while not native_client.call('list_capabilities')['native_async']['enabled']:
                    if time.monotonic()>lease_deadline:raise RuntimeError('Native worker lease unavailable')
                    time.sleep(.05)
                native_client.call('ingest_sources',{'source':source})
                launched=native_client.call('execute_task',operation);task=launched['intake']['request']['task_id']
            else:
                req(route,{'op':'ingest','source':source})
                launched=req(route,{'op':'execute_workflow',**operation});task=launched['request']['task_id']
            deadline = time.monotonic() + 170
            while True:
                result = native_client.call('get_task_status',{'task_id':task}) if args.native else req(route, {'op': 'status', 'task_id': task})
                if result['status'] != 'running':
                    break
                if time.monotonic() > deadline:
                    if args.native:native_client.call('cancel_task',{'task_id':task})
                    else:req(route, {'op': 'cancel_task', 'task_id': task})
                    raise RuntimeError('Bounded MLX trial deadline')
                time.sleep(.2)
            with sqlite3.connect(ROOT / 'data/companies' / company / 'brain.sqlite3') as db:
                receipts = [{'kind': kind, 'body': json.loads(body)} for kind, body in db.execute(
                    "SELECT kind,body FROM audit WHERE kind IN ('task_model_dispatch','task_model_return') ORDER BY rowid")]
                native_proof=None
                if args.native:
                    native_proof={'parent_run':parent,'parent_reserved_calls':db.execute('SELECT COUNT(*) FROM inference_attempt_reservations WHERE run_id=?',(parent,)).fetchone()[0],
                                  'native_admissions':db.execute("SELECT COUNT(*) FROM audit WHERE run_id=? AND kind='task_native_admitted'",(parent,)).fetchone()[0],
                                  'scope_subject':db.execute('SELECT subject FROM task_native_queue_v1 WHERE task=?',(task,)).fetchone()[0]}
                    expected_subject=f'local-developer:{parent}:Worker'
            returns = [r['body'] for r in receipts if r['kind'] == 'task_model_return']
            preflight = returns[0].get('local_execution', {}).get('prompt_preflight', {}) if returns else {}
            checks = {'generated': result['status'] == 'generated', 'exact_answer': (result.get('answer') or '').strip() == expected,
                      'one_generative_call': result['measurements']['generative_model_calls'] == 1,
                      'source_cited': bool(result['evidence']),
                      'offline_preflight': bool(preflight) and preflight.get('weights_loaded') is False,
                      'tokenizer_matches_server_usage': bool(preflight) and preflight.get('prompt_tokens') == returns[0].get('input_tokens')}
            with binary.open('rb') as stream:
                unchanged_binary = hashlib.file_digest(stream, 'sha256').hexdigest()==binary_sha
            checks['unchanged_harness_binary']=unchanged_binary
            if args.native:
                checks['native_parent_allowance_charged']=native_proof['parent_reserved_calls']==1
                checks['native_admission_recorded']=native_proof['native_admissions']==1
                checks['actor_private_scope']=native_proof['scope_subject']==expected_subject
            report = {'schema_version': 1, 'package_sha256': manifest['sha256'], 'model_path': args.model,
                      'harness_binary_sha256': binary_sha, 'transport':'native MCP stdio' if args.native else 'owner HTTP', 'criteria': criteria, 'checks': checks, 'passed': all(checks.values()),
                      'result': result, 'receipts': receipts, 'hardware': hardware, 'native_proof':native_proof, 'elapsed_seconds': time.monotonic() - began,
                      'provider_cost_usd': 0, 'production_admission': False,
                      'quality_scope': 'One synthetic source-backed literal/protocol trial; no specialist or general-quality qualification.'}
            (output / 'report.json').write_text(json.dumps(report, indent=2) + '\n')
            print(json.dumps({'report': str(output / 'report.json'), 'passed': report['passed'], 'checks': checks,
                              'answer': result.get('answer'), 'measurements': result['measurements'], 'provider_cost_usd': 0}), flush=True)
        finally:
            if native_client is not None:native_client.close()
            child.terminate()
            try:
                child.wait(timeout=20)
            except subprocess.TimeoutExpired:
                child.kill()
                child.wait(timeout=5)
            checkpoint = ROOT / 'data/companies' / company
            if checkpoint.is_dir() and not checkpoint.is_symlink():
                checkpoint.rename(output / 'company-checkpoint')


if __name__ == '__main__':
    main()
