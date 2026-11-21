import test from 'node:test';
import assert from 'node:assert/strict';
import {taskActivity,currentTaskActivity,currentTaskUsage,ceoPlanningActivity} from '../web/assets/task-activity.js';
import {screenDefinitions,screenValue,dashboardDetail} from '../web/assets/office-dashboards.js';
const task='190f3065-d78d-4575-a335-fc36822c39ad',job='b53b850b-640f-4fc4-9df9-b3cdf2e23eaf';
test('only current scoped persisted CEO requests animate planning, and terminal events stop it',()=>{
 const context={origin:'http://127.0.0.1:8788',company:'one',connected:true};
 const observation={...context,observed_at_ms:1000,events:[{run_id:task,seq:1,kind:'ceo_inference_requested'}]};
 assert.equal(ceoPlanningActivity(observation,task,context,1001).active,true);
 assert.equal(ceoPlanningActivity(observation,job,context,1001),null);
 assert.equal(ceoPlanningActivity(observation,task,{...context,company:'two'},1001),null);
 assert.equal(ceoPlanningActivity(observation,task,{...context,connected:false},1001),null);
 assert.equal(ceoPlanningActivity(observation,task,context,17000).active,false);
 observation.events.push({run_id:task,seq:2,kind:'ceo_planning_completed'});
 assert.equal(ceoPlanningActivity(observation,task,context,1001),null);
});
test('host requests animate task monitoring without fabricating actual inference or known usage',()=>{
 const activity=taskActivity({task_id:task,status:'running',progress:{job_id:job,phase:'model_request_pending',model_dispatches:2,model_returns:1,pending_model_requests:1},measurements:{generative_model_calls:null,input_tokens:null,provider_cost_usd:null}});
 assert.equal(activity.active,true);assert.equal(activity.label,'Model request pending');
 assert.equal(activity.model_dispatches,2);assert.equal(activity.generative_model_calls,null);assert.equal(activity.provider_cost_usd,null);
 for(const status of ['cancelled','interrupted','failed','evidence_invalidated','generated']){
  const stopped=taskActivity({task_id:task,status,progress:{phase:'model_request_pending',pending_model_requests:1}});
  assert.equal(stopped.active,false);assert.equal(stopped.pending_model_requests,0);
  assert.notEqual(stopped.label,'Model request pending');
 }
 assert.equal(taskActivity({task_id:'not-a-task',status:'running'}),null);
});
test('stale, disconnected, different-company or different-origin views cannot keep an actor busy',()=>{
 const value={...taskActivity({task_id:task,status:'running'}),observed_at_ms:1000,origin:'http://127.0.0.1:8788',company:'one'};
 const context={origin:value.origin,company:'one',connected:true};
 assert.equal(currentTaskActivity(value,context,1001).active,true);
 assert.equal(currentTaskActivity(value,context,17000).active,false);
 assert.equal(currentTaskActivity(value,context,999).stale,true);
 for(const change of [{company:'two'},{origin:'http://127.0.0.1:9999'},{connected:false}])assert.equal(currentTaskActivity(value,{...context,...change},1001),null);
});
test('CEO task screen and CFO detail keep selected-task usage distinct from journal totals',()=>{
 const taskScreen=screenDefinitions(10).find(s=>s.id==='task'),budget=screenDefinitions(5).find(s=>s.id==='budget');
 assert.equal(screenValue(taskScreen,{available:true}),'NO SELECTED TASK');
 const snapshot={available:true,task:{label:'Model request pending',task_id:task,generative_model_calls:null,model_dispatches:1,provider_cost_usd:null},totals:{provider_cost_usd:0}};
 assert.equal(screenValue(taskScreen,snapshot),'Model request pending');
 assert.equal(screenValue(budget,snapshot),'$0.0000');
 const detail=dashboardDetail(budget,snapshot);
 assert.ok(detail.facts.some(s=>s.includes('Selected task:')));
 assert.ok(detail.facts.some(s=>s.includes('Selected task provider cost: unknown')));
});
test('CFO company receipts remain distinct from legacy totals and disappear outside current scope',()=>{
 const budget=screenDefinitions(9)[0],context={company:'one',origin:'http://127.0.0.1:8788',connected:true};
 const value={...context,observed_at_ms:1000,sampled_at_ms:1000,provider_cost_usd:0,generative_model_calls:2,input_tokens:30,output_tokens:4,observed_native_replies:2,unresolved_workflow_requests:0,grounded_jobs_without_receipt:0,coverage_complete:true,scope:'Retained company receipts only'};
 const snapshot={available:true,totals:{provider_cost_usd:25},task_usage:currentTaskUsage(value,context,1001)};
 assert.equal(screenValue(budget,snapshot),'$0.0000');
 const detail=dashboardDetail(budget,snapshot);
 assert.ok(detail.facts.includes('Recorded provider cost: $25.0000'));
 assert.ok(detail.facts.includes('Company task provider cost: $0.0000'));
 assert.ok(detail.facts.includes('Company task tokens: 30 input / 4 output'));
 assert.equal(screenValue(budget,{...snapshot,task_usage:{...value,provider_cost_usd:null}}),'UNAVAILABLE');
 assert.equal(currentTaskUsage(value,context,17000),null);
 assert.equal(currentTaskUsage(value,context,999),null);
 for(const change of [{company:'two'},{origin:'other'},{connected:false}])assert.equal(currentTaskUsage(value,{...context,...change},1001),null);
 assert.equal(screenValue(budget,{...snapshot,task_usage:null}),'UNAVAILABLE');
});
