import test from 'node:test';
import assert from 'node:assert/strict';
import {planningTurn,PlanningObservationUncertain} from './mcp-planning-observer.mjs';
const body={expected_revision:1,message:'frozen requirement'},path='/planning/session',session='session';
test('selection review is a terminal non-dispatching turn and a lost reply never repeats its POST',async()=>{
 let posts=0,gets=0;
 const result=await planningTurn({path,session,body,pause:async()=>{},request:async(_,data)=>{if(data){posts++;throw Error('lost reply');}gets++;return {id:session,revision:2,status:'selection_review'};}});
 assert.equal(result.status,'selection_review');assert.equal(posts,1);assert.equal(gets,1);
});
test('a timed-out POST is issued once and recovered by reads across transient observation failures',async()=>{
 let posts=0,gets=0;const observations=[];
 const result=await planningTurn({path,session,body,pause:async()=>{},onObservation:async item=>observations.push(item),request:async(_,data)=>{
  if(data){posts++;throw new DOMException('timeout','TimeoutError');}
  gets++;if(gets===1)throw Error('temporary read failure');
  return {id:session,revision:2,status:gets===2?'generating':'ready'};
 }});
 assert.equal(posts,1);assert.equal(gets,3);assert.equal(result.status,'ready');assert.equal(observations.at(-1).replayed,false);
});
test('uncertain dispatch, concurrent revisions and missing sessions never reissue a POST',async()=>{
 for(const observed of [{id:session,revision:1,status:'question'},{id:session,revision:3,status:'ready'},{id:'another-session',revision:2,status:'ready'},{id:session,revision:2,status:'unknown'},Object.assign(Error('not found'),{status:404})]){
  let posts=0;
  await assert.rejects(planningTurn({path,session,body,pause:async()=>{},request:async(_,data)=>{if(data){posts++;throw Error('lost reply');}if(observed instanceof Error)throw observed;return observed;}}),PlanningObservationUncertain);
  assert.equal(posts,1);
 }
});
test('successful POST and explicit HTTP rejection do not cause recovery or more model work',async()=>{
 let count=0;const result=await planningTurn({path,session,body,request:async()=>{count++;return {status:'question'};}});assert.equal(result.status,'question');assert.equal(count,1);
 const error=Object.assign(Error('stale revision'),{status:409});
 await assert.rejects(planningTurn({path,session,body,request:async()=>{throw error;}}),e=>e===error);
});
