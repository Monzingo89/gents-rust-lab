// Read-only identity checks for continuation before any lease/model mutation.
import assert from 'node:assert/strict';
import {canonical,fixtureDigest} from './fixtures/mcp-workflow-catalog.mjs';
import {planningInputs} from './mcp-workflow-review.mjs';
import {hasSelection,selectedExecutionFixture,reviewSelectionDraft} from './mcp-owned-selection.mjs';
export function validatePlanningContinuation(prior,fixture,profile,checkpoint){
 assert.equal(prior.run,null,'Issued runs must use execution continuation');
 assert(['FAIL','OBSERVATION_ERROR_LIVE_WORK_RETAINED'].includes(prior.status),'Planning origin must retain interrupted/failed evidence');
 assert.equal(prior.company,'mcp-workflow-check');
 assert.equal(prior.fixture_id,fixture.id);assert.equal(prior.fixture_sha,fixtureDigest(fixture),'Planning fixture changed');
 assert.equal(profile.sha256,prior.runtime_profile?.sha256,'Planning continuation must retain the original exact model profile');
 assert.equal(profile.path,prior.runtime_profile?.path,'Planning continuation must retain the original profile path');
 assert.equal(checkpoint.id,prior.planning_session,'Planning session changed');
 assert(['question','selection_review','ready','failed'].includes(checkpoint.status),'Live or uncertain planning cannot be restarted');
 const view=hasSelection(fixture)&&checkpoint.accepted_owned_selection?selectedExecutionFixture(fixture,checkpoint):fixture;
 if(hasSelection(fixture)){assert.equal(canonical(checkpoint.owned_argument_data),canonical(fixture.selection.data),'Owner selection data changed');if(checkpoint.pending_owned_selection)reviewSelectionDraft(fixture,checkpoint);}
 assert.equal(canonical(checkpoint.source_inputs),canonical(planningInputs(view)),'Pinned planning inputs changed');
 assert.equal(canonical(checkpoint.planning_constraints??{}),canonical(fixture.planningConstraints??{}),'Owner planning constraints changed');
 const requirements=checkpoint.messages.filter(m=>m.role==='user'&&m.kind!=='planning_repair').map(m=>m.message);
 assert(requirements.includes(fixture.initial),'Original user goal missing');
 assert(Number.isSafeInteger(prior.started_at_ms)&&prior.started_at_ms>0,'Original timing missing');
 if(prior.planning_origin_started_at_ms!==undefined&&prior.planning_origin_started_at_ms!==null)assert(Number.isSafeInteger(prior.planning_origin_started_at_ms)&&prior.planning_origin_started_at_ms>0&&prior.planning_origin_started_at_ms<=prior.started_at_ms,'Invalid planning origin timing');
 assert(['model_review','approved_workflow'].includes(prior.owner_authorization_mode??'model_review'),'Unknown original owner mode');
}
