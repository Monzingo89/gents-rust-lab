import test from 'node:test';
import assert from 'node:assert/strict';
import {fixtureFromId} from './fixtures/mcp-workflow-catalog.mjs';
import {planningInputs,reviewWorkflowPlan} from './mcp-workflow-review.mjs';
const fixture=fixtureFromId('outcome-only-reconciliation');
function proposal(task=fixture){
 const fixture=task;
 const source=input=>({id:input.id,actor:'Analyst',...(planningInputs(fixture).some(f=>f.id===input.id)?{input:input.id}:{tool:input.tool,arguments:structuredClone(input.arguments)}),argument_sources:fixture.dependencies[input.id].map(ref=>({field:ref.field,source:ref.input,pointer:ref.pointer}))});
 return {planning_constraints:structuredClone(fixture.planningConstraints),id:'00000000-0000-4000-8000-000000000001',revision:2,status:'ready',source_inputs:structuredClone(planningInputs(fixture)),messages:[{role:'user',message:fixture.initial},{role:'user',message:fixture.requirements}],proposal:{planning_constraints:structuredClone(fixture.planningConstraints),definition_of_done:['Every requested file matches actual native results and readback'],assignments:[{actor:'Analyst',task:'Analyze pinned inputs',tools:[...new Set(fixture.sourceInputs.map(input=>input.tool))],acceptance:['Actual native data flow']},{actor:'Writer',task:'Write and verify results',tools:['files:write','files:read'],acceptance:['Actual readbacks']}],native_contracts:{sources:fixture.sourceInputs.map(source),artifacts:fixture.artifacts.map(a=>({path:a.path,source:a.input,pointer:a.pointer,encoding:a.encoding,writer:'Writer'}))}},proposed_artifact_bindings:fixture.artifacts.map(spec=>({path:spec.path,source_actor:'Analyst',writer:'Writer',source_tool:fixture.sourceInputs.find(input=>input.id===spec.input).tool,arguments:structuredClone(fixture.sourceInputs.find(input=>input.id===spec.input).arguments),pointer:spec.pointer,encoding:spec.encoding,...(fixture.dependencies[spec.input].length?{argument_sources:fixture.dependencies[spec.input].map(ref=>({field:ref.field,path:ref.path}))}:{})}))};
}
test('outcome task gives file inputs and desired results without prescribing the transformation graph',()=>{
 assert.equal(planningInputs(fixture).length,2);assert(planningInputs(fixture).every(input=>input.tool==='table_read_json'&&!input.derived_fields));
 for(const hint of ['table_join','table_aggregate','argument_sources','join-input','summary-input'])assert(!fixture.requirements.includes(hint)&&!fixture.initial.includes(hint));
 assert.deepEqual(fixture.planningConstraints,{separate_source_writers:true});
 assert.throws(()=>fixtureFromId('../../unreviewed'));
});
test('review accepts equivalent metadata order while preserving data and policy constraints',()=>{
 const plan=proposal();plan.messages.push({role:'ceo',message:structuredClone({...plan.proposal,status:'ready'})});reviewWorkflowPlan(plan,fixture);
 for(const binding of plan.proposed_artifact_bindings)if(binding.source_tool==='table_join'){binding.arguments.columns.reverse();binding.argument_sources.reverse();}
 plan.proposal.native_contracts.sources.find(source=>source.tool==='table_join').arguments.columns.reverse();plan.messages.at(-1).message.native_contracts.sources.find(source=>source.tool==='table_join').arguments.columns.reverse();
 reviewWorkflowPlan(plan,fixture);
 const wrongFlag=structuredClone(plan);for(const binding of wrongFlag.proposed_artifact_bindings)if(binding.source_tool==='table_aggregate')binding.arguments.filter.equals='true';assert.throws(()=>reviewWorkflowPlan(wrongFlag,fixture));
 const invented=structuredClone(plan);invented.proposed_artifact_bindings.find(binding=>binding.source_tool==='table_join').arguments.left_rows=[{sku:'invented',units:99}];assert.throws(()=>reviewWorkflowPlan(invented,fixture));
 const expanded=structuredClone(plan);expanded.proposal.assignments[0].tools.push('memory_append');assert.throws(()=>reviewWorkflowPlan(expanded,fixture),/unused by the declared native sources: memory_append/);
});
test('graph mode retains model ownership while exposing the compiled draft capabilities',()=>{
 const plan=proposal();const raw=structuredClone(plan.proposal);raw.status='ready';raw.capability_mode='native_graph';for(const stage of raw.assignments)stage.tools=[];
 plan.messages.push({role:'ceo',message:raw});plan.proposal.capability_mode='native_graph';plan.host_compiled_capabilities=plan.proposal.assignments.map(({actor,tools})=>({actor,tools}));
 reviewWorkflowPlan(plan,fixture);
 const owner=structuredClone(plan);delete owner.messages.at(-1).message.native_contracts.sources[0].actor;assert.throws(()=>reviewWorkflowPlan(owner,fixture),/owner must be chosen/);
 const hidden=structuredClone(plan);hidden.host_compiled_capabilities=[];assert.throws(()=>reviewWorkflowPlan(hidden,fixture),/explicitly reviewable/);
 const changed=structuredClone(plan);changed.proposal.assignments[0].task='Host authored replacement';assert.throws(()=>reviewWorkflowPlan(changed,fixture),/retain CEO staffing/);
});

test('v2 root ownership is checked against the actual CEO map',()=>{
 const plan=proposal();const raw=structuredClone(plan.proposal);raw.status='ready';raw.capability_mode='native_graph_v2';raw.input_owners=Object.fromEntries(raw.native_contracts.sources.filter(source=>source.input).map(source=>[source.input,source.actor]));raw.native_contracts.sources=raw.native_contracts.sources.filter(source=>!source.input);for(const stage of raw.assignments)stage.tools=[];
 plan.messages.push({role:'ceo',message:raw});plan.proposal.capability_mode=raw.capability_mode;plan.proposal.input_owners=structuredClone(raw.input_owners);plan.host_compiled_capabilities=plan.proposal.assignments.map(({actor,tools})=>({actor,tools}));reviewWorkflowPlan(plan,fixture);
 const changed=structuredClone(plan);changed.proposal.input_owners['sales-input']='Writer';assert.throws(()=>reviewWorkflowPlan(changed,fixture),/retain the CEO input-owner/);
 const root=structuredClone(plan);root.proposal.native_contracts.sources.find(source=>source.input).actor='Writer';assert.throws(()=>reviewWorkflowPlan(root,fixture),/Materialized input owner/);
});

test('owner duty constraints and policy mismatch diagnostics do not disclose expected values',()=>{
 const plan=proposal();plan.messages.push({role:'ceo',message:structuredClone({...plan.proposal,status:'ready'})});
 const changed=structuredClone(plan);changed.planning_constraints.separate_source_writers=false;assert.throws(()=>reviewWorkflowPlan(changed,fixture),/Owner duty constraints/);
 const wrong=structuredClone(plan);wrong.proposed_artifact_bindings.find(binding=>binding.source_tool==='table_join').arguments.kind='inner';assert.throws(()=>reviewWorkflowPlan(wrong,fixture),/argument fields: kind/);
});

test('harder depot case has three pinned inputs and independent literal edge-case expectations',()=>{
 const task=fixtureFromId('depot-reconciliation');assert.equal(planningInputs(task).length,3);assert.equal(task.sourceInputs.length,6);assert.equal(task.artifacts.length,9);
 for(const name of ['table_join','table_aggregate','product-match','depot-match','depot-summary','argument_sources'])assert(!task.initial.includes(name)&&!task.requirements.includes(name));
 const first=task.expectedResults.get('product-match'),last=task.expectedResults.get('depot-match'),summary=task.expectedResults.get('depot-summary');assert.equal(first.rows.length,10);assert.equal(first.unmatched_left_rows,1);assert.equal(last.rows.length,15);assert.equal(last.unmatched_left_rows,1);
 assert.equal(summary.total,7);assert.equal(summary.matched_rows,8);assert.equal(summary.excluded_rows,7);assert.deepEqual(summary.groups.find(group=>group.key==='parts'),{key:'parts',rows:4,total:0});
 assert.equal(last.rows.filter(row=>row.enabled===true).reduce((sum,row)=>sum+row.delta,0),summary.total);
});

test('audio company plan retains hash-bound source and independent least-authority writer',()=>{
 const task=fixtureFromId('audio-inspection'),plan=proposal(task);
 plan.messages.push({role:'ceo',message:structuredClone({...plan.proposal,status:'ready'})});
 const reviewed=reviewWorkflowPlan(plan,task);assert.deepEqual([...reviewed.requiredTools],['audio_inspect']);
 assert.equal(reviewed.draft.intake.artifact_bindings.length,4);
 const expanded=structuredClone(plan);expanded.proposal.assignments[0].tools.push('files:write');assert.throws(()=>reviewWorkflowPlan(expanded,task),/unused/);
 const altered=structuredClone(plan);altered.source_inputs[0].arguments.expected_sha256='0'.repeat(64);assert.throws(()=>reviewWorkflowPlan(altered,task),/fixed owner inputs/);
 assert(!task.initial.includes(task.files[0].bytes_base64));assert(!task.requirements.includes('32768'));
});

test('batch audio case binds all nine outputs to three distinct pinned sources',()=>{
 const task=fixtureFromId('audio-batch'),plan=proposal(task);plan.messages.push({role:'ceo',message:structuredClone({...plan.proposal,status:'ready'})});
 reviewWorkflowPlan(plan,task);assert.equal(task.files.length,3);assert.equal(task.artifacts.length,9);assert.equal(task.expectedResults.get('empty-input').channel_statistics[0].minimum,null);
 const substituted=structuredClone(plan);substituted.proposed_artifact_bindings.find(b=>b.path==='session_frames.txt').arguments=structuredClone(task.sourceInputs[1].arguments);assert.throws(()=>reviewWorkflowPlan(substituted,task),/source policy/);
 assert.equal(task.complexity,'medium');
});
test('reuse review binds source/API/input to the actual CEO analysis actor without source authorship',()=>{
 const task=fixtureFromId('owned-weighted-reuse'),plan=proposal(task);plan.messages.push({role:'ceo',message:structuredClone({...plan.proposal,status:'ready'})});
 const reviewed=reviewWorkflowPlan(plan,task),calls=reviewed.draft.intake.required_owned_rust_calls;
 assert.equal(calls.length,2);assert(calls.every(call=>call.actor===plan.proposal.assignments[0].actor));
 assert(calls.every(call=>call.source_sha256===task.files[0].sha256));
 assert.equal(calls[0].function,'select_slots');assert.notEqual(calls[0].input_sha256,calls[1].input_sha256);
 assert.equal(reviewed.draft.intake.required_rust_checks,undefined);
 const changed=structuredClone(plan);changed.source_inputs[0].arguments.arguments[0][0][2]='100';assert.throws(()=>reviewWorkflowPlan(changed,task),/fixed owner inputs/);
});
test('prepared graph review distinguishes receipt-only computation from final publication',()=>{
 const task=fixtureFromId('owned-allocation-chain'),plan=proposal(task),first=task.sourceInputs[0];
 const parent={path:'synthetic-receipt.json',source_actor:'Analyst',writer:'Analyst',source_tool:first.tool,arguments:structuredClone(first.arguments),pointer:'/output',encoding:'json',receipt_only:true};
 plan.proposed_artifact_bindings.unshift(parent);
 plan.proposed_artifact_bindings[1].argument_sources=[{field:'arguments',path:parent.path,argument_index:1,pointer:'/some'}];
 const raw=structuredClone(plan.proposal);raw.status='ready';raw.capability_mode='native_prepared_graph';for(const stage of raw.assignments)stage.tools=[];
 plan.messages.push({role:'ceo',message:raw});plan.proposal.capability_mode=raw.capability_mode;plan.host_compiled_capabilities=plan.proposal.assignments.map(({actor,tools})=>({actor,tools}));
 const reviewed=reviewWorkflowPlan(plan,task);assert.equal(reviewed.draft.intake.artifact_bindings.length,2);assert.equal(reviewed.draft.intake.required_owned_rust_calls,undefined);
 for(const mutate of [p=>p.proposed_artifact_bindings.push({...parent,path:'extra.json'}),p=>p.proposed_artifact_bindings[0].arguments.binding_id='unknown',p=>p.proposed_artifact_bindings[0].writer='Writer',p=>p.proposed_artifact_bindings[1].argument_sources[0].pointer='',p=>p.proposed_artifact_bindings[0].receipt_only=false]){
  const p=structuredClone(plan);mutate(p);assert.throws(()=>reviewWorkflowPlan(p,task));
 }
});
