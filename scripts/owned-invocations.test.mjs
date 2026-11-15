import test from 'node:test';
import assert from 'node:assert/strict';
import {appendInvocation} from '../web/assets/owned-invocations.js';
test('owner data templates select only inspected functions, unique ids and no actor/source authority',()=>{
 const component={component_id:'a'.repeat(64),functions:[{name:'first'},{name:'second'}]};
 const once=appendInvocation('[]',component,component.functions[1]);assert.equal(once[0].function,'second');assert.deepEqual(once[0].arguments,[]);
 assert(!('actor' in once[0]));assert(!('source' in once[0]));
 const twice=appendInvocation(JSON.stringify(once),component,component.functions[0]);assert.notEqual(once[0].id,twice[1].id);assert.deepEqual(twice[0],once[0]);
 assert.throws(()=>appendInvocation('[]',component,{name:'uninspected'}));assert.throws(()=>appendInvocation('{}',component,component.functions[0]));
 assert.throws(()=>appendInvocation(JSON.stringify(Array.from({length:8},(_,i)=>({id:String(i)}))),component,component.functions[0]));
});
