import test from 'node:test';
import assert from 'node:assert/strict';
import {validatePlanningContinuation} from './mcp-planning-continuation.mjs';
import {fixtureFromId,fixtureDigest} from './fixtures/mcp-workflow-catalog.mjs';
test('planning continuation retains the exact fixture, profile, owner constraints and conversation identity',()=>{
 const fixture=fixtureFromId('audio-routing'),profile={path:'config/profile.json',sha256:'a'.repeat(64)};
 const prior={run:null,status:'OBSERVATION_ERROR_LIVE_WORK_RETAINED',company:'mcp-workflow-check',fixture_id:fixture.id,fixture_sha:fixtureDigest(fixture),runtime_profile:profile,planning_session:'session',started_at_ms:1234,owner_authorization_mode:'approved_workflow'};
 const checkpoint={id:'session',status:'failed',source_inputs:fixture.planningInputs,planning_constraints:fixture.planningConstraints,messages:[{role:'user',kind:'requirements',message:fixture.initial}]};
 const before=JSON.stringify({prior,checkpoint});validatePlanningContinuation(prior,fixture,profile,checkpoint);assert.equal(JSON.stringify({prior,checkpoint}),before);
 for(const patch of [{run:'issued-run'},{status:'PASS'},{fixture_sha:'b'.repeat(64)},{planning_session:'changed'},{runtime_profile:{...profile,sha256:'changed'}},{started_at_ms:0},{owner_authorization_mode:'invented'}])assert.throws(()=>validatePlanningContinuation({...prior,...patch},fixture,profile,checkpoint));
 for(const patch of [{status:'generating'},{source_inputs:[]},{planning_constraints:{}},{id:'other'},{messages:[]}])assert.throws(()=>validatePlanningContinuation(prior,fixture,profile,{...checkpoint,...patch}));
 assert.throws(()=>validatePlanningContinuation(prior,fixture,{...profile,path:'config/other.json'},checkpoint));
});
