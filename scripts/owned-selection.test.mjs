import test from 'node:test';
import assert from 'node:assert/strict';
import {selectionInvocations} from '../web/assets/owned-selection.js';
function session(){return {status:'selection_review',owned_argument_data:{values:['0','2']},owned_component_catalog:{catalog_sha256:'catalog',components:[{component_id:'a'.repeat(64),functions:[{name:'selected',arguments:{prefixItems:[{}]}}]}]},pending_owned_selection:{proposal_sha256:'b'.repeat(64),catalog_sha256:'catalog',execution_authorized:false,promotion_authorized:false,invocations:[{id:'chosen',component_id:'a'.repeat(64),function:'selected',arguments:[null],argument_slots:[0]}]}};}
test('review copies only prepared call data without mutating the session or installing authority',()=>{
 const s=session(),copy=selectionInvocations(s,'{"values":["0","2"]}');copy[0].id='changed';assert.equal(s.pending_owned_selection.invocations[0].id,'chosen');assert.equal(s.status,'selection_review');assert(!('policy' in s));
});
test('stale data, uninspected functions, unknown authority fields and invalid result slots cannot be copied',()=>{
 for(const mutate of [s=>s.status='ready',s=>s.pending_owned_selection.execution_authorized=true,s=>s.pending_owned_selection.catalog_sha256='stale',s=>s.pending_owned_selection.invocations[0].function='invented',s=>s.pending_owned_selection.invocations[0].actor='Owner',s=>s.pending_owned_selection.invocations[0].source='code',s=>s.pending_owned_selection.invocations[0].argument_slots=[0,0],s=>s.pending_owned_selection.invocations[0].arguments[0]=['invented']]){const s=session();mutate(s);assert.throws(()=>selectionInvocations(s,'{"values":["0","2"]}'));}
 assert.throws(()=>selectionInvocations(session(),'{"values":["other"]}'));
});
