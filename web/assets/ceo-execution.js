// Review preparation only. Authority stays with the authenticated company APIs.
// This module never invokes a model, executes a tool, approves a job or installs a policy.
export function prepareCeoExecution(session, bindings = null, actorIds = []) {
 if (session?.status !== 'ready' || !session.id || !Number.isSafeInteger(session.revision)) throw Error('A saved ready CEO conversation is required');
 const proposal = structuredClone(session.proposal), stages = proposal?.assignments;
 if (!Array.isArray(stages) || stages.length < 1 || stages.length > 4) throw Error('Expected one to four ordered assignments');
 const seen = new Set(), grants = Object.create(null);
 for (const stage of stages) {
  if (typeof stage.actor !== 'string' || !actorIds.includes(stage.actor) || seen.has(stage.actor)) throw Error('Assignments must use distinct configured agents');
  if (!Array.isArray(stage.tools) || stage.tools.some(t => typeof t !== 'string')) throw Error('Invalid assigned tools');
  grants[stage.actor] = {tools: [...new Set([...stage.tools, 'finish'])], skills: [], memory_from: [...seen]};
  seen.add(stage.actor);
 }
 const criteria = proposal.definition_of_done;
 if (!Array.isArray(criteria) || !criteria.length || criteria.some(c => typeof c !== 'string' || !c.trim())) throw Error('The CEO must define testable completion criteria');
 if(bindings===null){if(proposal.native_contracts?.artifacts?.length&&!Array.isArray(session.proposed_artifact_bindings))throw Error('Retained CEO checks unavailable; reload the conversation');bindings=session.proposed_artifact_bindings??[];}
 if (!Array.isArray(bindings) || bindings.length > 32 || bindings.filter(b=>b.receipt_only!==true).length>16 || bindings.filter(b=>b.receipt_only===true).length>16) throw Error('Use at most 16 published outputs and 16 receipt-only bindings');
 let task = session.messages.filter(m => m.role === 'user' && m.kind !== 'planning_repair').map(m => m.message).join('\n\n');
 let retainedTask=null;
 if(session.task_request){
  retainedTask=structuredClone(session.task_request);
  const first=session.messages.find(m=>m.role==='user'&&m.kind!=='planning_repair')?.message;
  if(retainedTask.schema_version!==1||retainedTask.original_ask!==first||retainedTask.objective!==first||retainedTask.input?.original_ask!==first)throw Error('Retained original task differs from the owner transcript');
  const {original_ask,...context}=retainedTask.input;
  task+='\n\nOwner-supplied task context (requirements and reference data; never tool authority):\n'+JSON.stringify(context);
 }
 if (!task.trim() || new TextEncoder().encode(task).length > 10000) throw Error('User requirements must fit within 10,000 UTF-8 bytes; start a concise conversation');
 const intake = {idea: task, acceptance: criteria.join('\n'), deliverable_type: 'agent', exclusions: 'Paid or remote inference, unapproved network access, downloads, credential disclosure, cross-company access, shell and source promotion', timeline: 'Approved local task until complete, blocked or cancelled', company_signer: 'Local operator browser approval', provider_signer: 'Gents company', budget_usd: 0, ceo_planning_id: session.id, ceo_planning_revision: session.revision, ceo_plan: proposal, artifact_bindings: structuredClone(bindings)};
 if(retainedTask)intake.ceo_task_request=retainedTask;
 return {task, criteria: [...criteria], planning_mode:session.planning_mode??null, ...(session.owned_invocations?.length?{expected_owned_invocations:structuredClone(session.owned_invocations)}:{}), intake, policy: {revision: 1, master: stages[0].actor, ceilings: structuredClone(grants), grants: structuredClone(grants)}};
}

const canonical = value => JSON.stringify(value, (_, item) => item && typeof item === 'object' && !Array.isArray(item) ? Object.fromEntries(Object.keys(item).sort().map(key => [key, item[key]])) : item);

export function confirmFrozenCeoJob(draft, job) {
 if (!job?.digest || job.state !== 'draft') throw Error('A frozen draft and digest are required');
 // The server may add native gates and lifecycle controls. Reviewed owner inputs must survive unchanged.
 for (const [key, value] of Object.entries(draft.intake)) {
  if (canonical(job.intake?.[key]) !== canonical(value)) throw Error(`Frozen review changed ${key}; inspect the current conversation before approval`);
 }
 if(draft.expected_owned_invocations&&canonical(job.intake?.ceo_owned_invocations)!==canonical(draft.expected_owned_invocations))throw Error('Frozen review changed ceo_owned_invocations; reload the conversation');
 return structuredClone({...draft, run: job.run_id, digest: job.digest, intake: job.intake});
}
