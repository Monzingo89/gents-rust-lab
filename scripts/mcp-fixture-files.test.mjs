import test from 'node:test';
import assert from 'node:assert/strict';
import {createHash} from 'node:crypto';
import {fixtureBytes,fixtureFileSpec} from './mcp-fixture-files.mjs';
import {fixtureFromId,fixtureDigest} from './fixtures/mcp-workflow-catalog.mjs';
const digest=bytes=>createHash('sha256').update(bytes).digest('hex');
test('binary sources preserve exact bytes and reject ambiguous, unsafe or corrupt admission',()=>{
 const bytes=Buffer.from([0,255,128,13,10]),file={path:'input.wav',bytes_base64:bytes.toString('base64'),sha256:digest(bytes),input:'input'};
 assert.deepEqual(fixtureBytes(file),bytes);assert.deepEqual(fixtureFileSpec(file),{path:file.path,bytes_base64:file.bytes_base64,input:'input'});
 for(const patch of [{text:'also text'},{path:'../escape'},{path:'a/b'},{path:'x'.repeat(121)},{sha256:'0'.repeat(64)},{bytes_base64:file.bytes_base64+'\n'},{bytes_base64:'!!!!'},{bytes_base64:undefined}])assert.throws(()=>fixtureBytes({...file,...patch}));
 assert.throws(()=>fixtureBytes({...file,bytes_base64:'A'.repeat(Math.ceil(16*1024*1024/3)*4+4)}),/bound/);
});
test('legacy text fixture hash representation remains unchanged',()=>{
 const text='signed −1\n',file={path:'input.json',text,input:'input',sha256:digest(text)};
 assert.deepEqual(fixtureBytes(file),Buffer.from(text));assert.deepEqual(fixtureFileSpec(file),{path:file.path,text,input:'input'});
 const prior={'reconciliation':'e7cbcebb1a25948bb6e5d8ebff4c9e933e6e5fca4aa1d6f439460d3f2ec529bf','reconciliation-edges':'d5e40542c66922c15c9868aa9e218d5b07483a70226897cec92d0b04a70b139b','outcome-only-reconciliation':'03b21cbed588b7c699c4e77ce3fdd9e3dd12533bb6f332d9518d34493cdcbd8e','depot-reconciliation':'5cc5316b064a63daffa16c171a65efe30e978609864ae52366801c46eab498cb'};
 for(const [id,sha] of Object.entries(prior))assert.equal(fixtureDigest(fixtureFromId(id)),sha);
});
test('audio fixture is explicitly a bounded easy audio and text family, with no model answer payload',()=>{
 const f=fixtureFromId('audio-inspection');assert.equal(f.complexity,'easy');assert.deepEqual(f.modalities,['audio','text']);assert.equal(fixtureBytes(f.files[0]).length,76);
 assert.equal(f.sourceInputs.length,1);assert.equal(f.sourceInputs[0].tool,'audio_inspect');assert.equal(f.artifacts.length,4);
 assert(!f.initial.includes(f.files[0].bytes_base64)&&!f.requirements.includes(f.files[0].bytes_base64));
 assert.notEqual(fixtureDigest(f),fixtureDigest({...f,files:[{...f.files[0],input:'changed'}]}));
});
test('hard audio routing keeps frozen graphs private and preserves the qualified audio versions',()=>{
 const f=fixtureFromId('audio-routing');assert.equal(f.complexity,'hard');assert.equal(f.planningInputs.length,2);assert(f.planningInputs.every(input=>!input.derived_fields));
 assert.deepEqual(f.sourceInputs.map(input=>input.tool),['audio_inspect','table_read_json','table_join','table_aggregate']);
 assert.equal(Object.values(f.dependencies).flat().length,3);assert.equal(f.artifacts.length,6);
 for(const file of f.files)fixtureBytes(file);
 assert.equal(fixtureDigest(fixtureFromId('audio-inspection')),'3c37919567c0d62d9199651ac9f25cdf043a58ef09fdd5f0dbca3a2359715c7e');
 assert.equal(fixtureDigest(fixtureFromId('audio-batch')),'ffcc13ff2b30222ecf9adeb0460683ab9b1ecd0e086a7579df9fcda33a036489');
 assert.notEqual(fixtureDigest(f),fixtureDigest({...f,dependencies:{...f.dependencies,'summary-input':[]}}));
});
