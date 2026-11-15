// Pure operator review of a compiled proposal. No expected result values enter
// feedback or execution. Column/ref ordering is metadata, not row ordering.
import assert from 'node:assert/strict';
import {prepareCeoExecution} from '../web/assets/ceo-execution.js';
import {canonical} from './fixtures/mcp-reconciliation.mjs';
import {frozenOwnedCalls} from './mcp-owned-fixture.mjs';
import {hasOwnedSlots,ownedReferences} from './mcp-owned-edges.mjs';
export const planningInputs=fixture=>fixture.planningInputs??fixture.sourceInputs;
export function sourceArguments(tool,arguments_){const value=structuredClone(arguments_);if(tool==='table_join'&&Array.isArray(value.columns))value.columns.sort((a,b)=>String(a.as).localeCompare(String(b.as)));return value;}
export const references=items=>[...items].sort((a,b)=>a.field.localeCompare(b.field));
export function reviewWorkflowPlan(plan,fixture){
 assert.equal(plan.status,'ready','A ready proposal is required');
 const stages=plan.proposal.assignments;assert.equal(stages.length,2,'The user requested two independent roles: use one stage for all native sources, followed by a different actor for all artifact writers. Choose the actors yourself; do not combine analysis and output');const [analyst,writer]=stages;assert.notEqual(analyst.actor,writer.actor,'Independent roles require different actors');
 const requiredTools=new Set(fixture.sourceInputs.map(input=>input.tool));
 // Diagnose excess authority from the model's own graph, never the evaluator's
 // expected graph. Names already present in a rejected draft are not new facts.
 const declaredTools=new Set(plan.proposal.native_contracts.sources.map(source=>source.tool??plan.source_inputs.find(input=>input.id===source.input)?.tool).filter(Boolean));
 const unused=analyst.tools.filter(tool=>tool!=='finish'&&!declaredTools.has(tool));
 assert.equal(unused.length,0,`Analysis capabilities unused by the declared native sources: ${unused.join(', ')}. Remove unused capabilities from the draft`);
 assert([...requiredTools].every(tool=>analyst.tools.includes(tool))&&analyst.tools.every(tool=>requiredTools.has(tool)||tool==='finish'),'Analysis tools do not match the minimum local workflow for the stated outcome');
 assert(writer.tools.includes('files:write')&&writer.tools.includes('files:read')&&writer.tools.every(tool=>['files:write','files:read','finish'].includes(tool)),'The output role requires only approved file operations and verification');
 assert.equal(canonical(plan.source_inputs),canonical(planningInputs(fixture)),'The fixed owner inputs must remain unchanged');
 if(fixture.planningConstraints){assert.deepEqual(plan.planning_constraints,fixture.planningConstraints,'Owner duty constraints must remain fixed');assert.deepEqual(plan.proposal.planning_constraints,fixture.planningConstraints,'CEO must retain owner duty constraints');}
 if(fixture.outcomeOnly){
  const raw=plan.messages.filter(message=>message.role==='ceo'&&message.message?.status==='ready').at(-1)?.message;
  assert(raw,'The actual local CEO draft is required for outcome-only review');
  if(['native_graph','native_graph_v2','native_prepared_graph'].includes(raw.capability_mode)){
   assert(raw.assignments.every(stage=>stage.tools.length===0),'Graph drafts must not carry a second capability list');
   if(raw.capability_mode==='native_graph_v2'){
    assert.deepEqual(plan.proposal.input_owners,raw.input_owners,'Compilation must retain the CEO input-owner selections');
    assert.deepEqual(Object.keys(raw.input_owners).sort(),plan.source_inputs.map(input=>input.id).sort(),'Every attached input needs a choice');
    const roots=plan.proposal.native_contracts.sources.filter(source=>source.input);
    assert.equal(roots.length,Object.values(raw.input_owners).filter(actor=>actor!==null).length,'Only selected inputs may materialize');
    for(const root of roots)assert(root.id===root.input&&root.actor===raw.input_owners[root.input],'Materialized input owner differs from actual CEO choice');
    assert(raw.native_contracts.sources.every(source=>!source.input),'The v2 CEO draft must not recopy fixed input roots');
   }

   assert(raw.native_contracts.sources.every(source=>typeof source.actor==='string'&&source.actor),'Every graph source owner must be chosen by the CEO');
   assert(raw.native_contracts.artifacts.every(artifact=>typeof artifact.writer==='string'&&artifact.writer),'Every graph writer must be chosen by the CEO');
   assert.deepEqual(plan.proposal.assignments.map(({tools,...stage})=>stage),raw.assignments.map(({tools,...stage})=>stage),'Compilation must retain CEO staffing and stage criteria');
   assert.equal(canonical(plan.host_compiled_capabilities.map(({actor,tools})=>({actor,tools}))),canonical(plan.proposal.assignments.map(({actor,tools})=>({actor,tools}))),'Compiled capabilities must be explicitly reviewable');
  }

  assert.deepEqual(plan.proposal.native_contracts.sources.filter(source=>!source.input),raw.native_contracts.sources.filter(source=>!source.input),'Host compilation must not choose or alter the CEO transformations');
 }
 const draft=prepareCeoExecution(plan,null,stages.map(stage=>stage.actor)),bindings=draft.intake.artifact_bindings;
 assert.equal(plan.proposal.native_contracts.sources.length,fixture.sourceInputs.length,'Reuse the minimum distinct native sources for all requested outputs');assert.equal(bindings.filter(b=>b.receipt_only!==true).length,fixture.artifacts.length,'Every requested output requires one native check');
 if(hasOwnedSlots(fixture)){
  assert(bindings.every(b=>fixture.sourceInputs.some(i=>i.tool===b.source_tool&&i.id===b.arguments.binding_id)),'Unexpected prepared operation');
  for(const input of fixture.sourceInputs)assert.equal(bindings.filter(b=>b.arguments.binding_id===input.id).length,Math.max(1,fixture.artifacts.filter(a=>a.input===input.id).length),'Unexpected duplicate prepared receipt');
  for(const input of fixture.sourceInputs){const group=bindings.filter(b=>b.source_tool===input.tool&&b.arguments.binding_id===input.id);assert(group.length>0,'Every prepared operation needs a frozen receipt');for(const b of group){assert.equal(b.source_actor,analyst.actor);assert.equal(canonical(b.arguments),canonical(input.arguments));if(b.receipt_only){assert.equal(b.writer,analyst.actor);assert.equal(b.pointer,'/output');assert.equal(b.encoding,'json');assert(bindings.some(child=>(child.argument_sources??[]).some(ref=>ref.path===b.path)),'Unused intermediate receipt');}}ownedReferences(fixture,input,bindings);}
 }
 for(const spec of fixture.artifacts){
  const input=fixture.sourceInputs.find(input=>input.id===spec.input),binding=bindings.find(binding=>binding.path===spec.path);assert(binding,`Missing output ${spec.path}`);
  assert.equal(binding.source_actor,analyst.actor,`Wrong analysis owner for ${spec.path}`);assert.equal(binding.writer,writer.actor,`Wrong output owner for ${spec.path}`);
  assert.equal(binding.source_tool,input.tool,`The chosen operation for ${spec.path} differs from the stated outcome`);
  const actualArguments=sourceArguments(input.tool,binding.arguments),expectedArguments=sourceArguments(input.tool,input.arguments);
  const mismatches=[...new Set([...Object.keys(actualArguments),...Object.keys(expectedArguments)])].filter(key=>canonical(actualArguments[key])!==canonical(expectedArguments[key])).sort();
  assert.equal(mismatches.length,0,`The source policy for ${spec.path} differs from the stated outcome at argument fields: ${mismatches.join(', ')}`);
  assert.equal(binding.pointer,spec.pointer,`The selected native field for ${spec.path} differs from its requested content`);assert.equal(binding.encoding,spec.encoding,`Artifact ${spec.path} must use ${spec.encoding} encoding`);
  if(input.argument_slots?.length)ownedReferences(fixture,input,bindings);else assert.equal(canonical(references(binding.argument_sources??[])),canonical(references(fixture.dependencies[spec.input].map(reference=>({field:reference.field,path:reference.path})))),`The data lineage for ${spec.path} must originate in the pinned file contents`);
 }
 if(fixture.ownedComponents&&!fixture.ownerPreparation)draft.intake.required_owned_rust_calls=frozenOwnedCalls(fixture,analyst.actor);
 return {draft,stages,analyst,writer,requiredTools};
}
