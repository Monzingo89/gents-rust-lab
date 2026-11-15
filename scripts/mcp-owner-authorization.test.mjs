import test from 'node:test';import assert from 'node:assert/strict';
import{ownerAuthorization}from'./mcp-owner-authorization.mjs';
import{supportsApprovedWorkflow,sameApprovalContext,authorizationDescription}from'../web/assets/owner-authorization.js';
test('owner mode changes no task facts, tool grants, memory edges or model-selected staffing',()=>{
 const access={tools:['files:read','files:write','finish'],skills:[],memory_from:['Analyst']};
 const draft={task:'frozen task',criteria:['actual results'],intake:{artifact_bindings:[{path:'result.json'}]},policy:{revision:1,master:'Writer',ceilings:{Writer:structuredClone(access)},grants:{Writer:structuredClone(access)}}};
 const baseline=structuredClone(draft),ordinary=ownerAuthorization(draft),closed=ownerAuthorization(draft,'approved_workflow');
 assert.deepEqual(draft,baseline);assert.deepEqual(ordinary.policy,draft.policy);assert.deepEqual(closed.intake,draft.intake);assert.deepEqual(closed.criteria,draft.criteria);
 for(const key of ['ceilings','grants'])assert.deepEqual(closed.policy[key].Writer,{...access,contract_execution_phases:['worker']});
 assert.throws(()=>ownerAuthorization(draft,'model_selected'),/Unknown/);assert.throws(()=>ownerAuthorization({...draft,intake:{}},'approved_workflow'),/bindings/);
});
test('mode changes require a fresh eligible draft and preserve explicit owner review',()=>{
 const grant={tools:['files:read','finish'],skills:[],memory_from:[]};
 const draft={intake:{artifact_bindings:[{path:'result.txt'}]},policy:{grants:{Writer:grant},ceilings:{Writer:grant}}};
 assert(supportsApprovedWorkflow(draft));
 assert(!supportsApprovedWorkflow({...draft,planning_mode:'implementation'}));
 const closed=ownerAuthorization(draft,'approved_workflow'),normal=ownerAuthorization(closed,'model_review');
 assert(!normal.policy.grants.Writer.contract_execution_phases);assert.deepEqual(normal.intake,draft.intake);
 for(const patch of [{run:'issued'},{digest:'frozen'},{approved:true},{installed:true}])assert.throws(()=>ownerAuthorization({...draft,...patch},'approved_workflow'),/new draft/);
 for(const intake of [{artifact_bindings:[]},{artifact_bindings:[{}],required_rust_checks:[{}]},{artifact_bindings:[{}],ceo_planning_mode:'implementation'}])assert.throws(()=>ownerAuthorization({...draft,intake},'approved_workflow'),/bindings/);
 assert.match(authorizationDescription('approved_workflow'),/independent local QA/);assert.throws(()=>authorizationDescription('model_choice'));
});
test('scope, mode and binding edits invalidate an asynchronous review context',()=>{
 const context={base:'http://localhost',companyId:'company',session:'saved',revision:2,bindingsText:'[]',authorizationMode:'model_review',dataClassification:'synthetic',architectureAssessment:'review-id'};
 assert(sameApprovalContext(context,{...context}));assert(!sameApprovalContext(null,context));
 assert(!sameApprovalContext({},{}));
 for(const key of Object.keys(context))assert(!sameApprovalContext(context,{...context,[key]:'changed'}),key);
});
