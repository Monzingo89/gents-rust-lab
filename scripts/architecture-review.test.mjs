import test from 'node:test';
import assert from 'node:assert/strict';
import {reviewController} from '../web/assets/architecture-review.js';
test('lost native review response is inspected by GET and never reissued',async()=>{
 const calls=[];const controller=reviewController({uuid:()=> 'fixed',request:async(path,options)=>{calls.push(options?.method||'GET');if(options)throw Error('lost reply');return {status:'ready_for_owner_review'};}});
 const context={company:'fixture',planning:'planning',revision:2,speed:'fast',days:14};
 await assert.rejects(controller.start(context));await assert.rejects(controller.start(context));
 assert.equal((await controller.inspect()).status,'ready_for_owner_review');assert.deepEqual(calls,['POST','GET']);
});
test('output routing and unknown review states cannot enable another POST',async()=>{
 for(const state of ['pending_output_judge','pending_judge','future_unknown_state']){
  const calls=[];const controller=reviewController({uuid:()=> 'fixed',request:async(_path,options)=>{calls.push(options?.method||'GET');return {status:state};}});
  const input={company:'fixture',planning:'planning',revision:2,speed:'patient',days:1};
  await controller.start(input);await assert.rejects(controller.start(input));await controller.inspect();await assert.rejects(controller.start(input));assert.deepEqual(calls,['POST','GET']);
 }
});
