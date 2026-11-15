// Owner review preparation only. No API calls, model choices or permissions installed.
export function supportsApprovedWorkflow(draft) {
 return Array.isArray(draft?.intake?.artifact_bindings)&&draft.intake.artifact_bindings.length>0
  &&!draft.intake.required_rust_checks?.length&&draft.intake.ceo_planning_mode!=='implementation'&&draft.planning_mode!=='implementation';
}
export function ownerAuthorization(draft,mode='model_review') {
 if(!['model_review','approved_workflow'].includes(mode))throw Error('Unknown owner authorization mode');
 if(draft?.run||draft?.digest||draft?.approved||draft?.installed)throw Error('Prepare a new draft to change authorization mode');
 if(mode==='approved_workflow'&&!supportsApprovedWorkflow(draft))throw Error('Approved workflow needs frozen native artifact bindings and excludes source authoring');
 const owned=structuredClone(draft);
 for(const group of ['ceilings','grants']) {
  if(!owned.policy?.[group]||!Object.keys(owned.policy[group]).length)throw Error('Owner authorization requires explicit agent grants and ceilings');
  for(const access of Object.values(owned.policy[group])) {
   if(mode==='approved_workflow')access.contract_execution_phases=['worker'];
   else delete access.contract_execution_phases;
  }
 }
 owned.owner_authorization_mode=mode;return owned;
}
export function authorizationDescription(mode) {
 if(mode==='model_review')return 'Local model review before each proposed operation; native permissions and independent final QA still apply.';
 if(mode==='approved_workflow')return 'Execute only exact steps from the approved native workflow. Native guards recheck each operation; independent local QA still decides completion.';
 throw Error('Unknown owner authorization mode');
}
export function sameApprovalContext(expected,current) {
 if(!expected||!current||!['base','companyId','session'].every(key=>typeof expected[key]==='string'&&expected[key].length>0)||!Number.isSafeInteger(expected.revision)||expected.revision<1||typeof expected.bindingsText!=='string'||!['model_review','approved_workflow'].includes(expected.authorizationMode))return false;
 return ['base','companyId','session','revision','bindingsText','authorizationMode','dataClassification','architectureAssessment'].every(key=>expected[key]===current[key]);
}
