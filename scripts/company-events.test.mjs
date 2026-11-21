import test from 'node:test';
import assert from 'node:assert/strict';
import {createServer} from 'node:http';
import {followCompany,projectEvent} from './company-events.mjs';

test('terminal projection retains provenance without private prompts, tool data or error bodies',()=>{
 const secret='fixture-private-credential';
 const event={kind:'tool_result',agent:'Engineer Gent',run_id:'run',seq:2,message:JSON.stringify({action:'files:read',ok:true,text:secret}),reply:{text:secret,local_execution:{backend_pid:123,dispatched_at_ms:10,received_at_ms:20,finish_reason:'stop',private:secret}}};
 const projected=projectEvent(event);
 assert.equal(projected.tool,'files:read');assert.equal(projected.ok,true);
 assert.equal(projected.inference.backend_pid,123);assert.equal(projected.result_sha256.length,64);
 assert.ok(!JSON.stringify(projected).includes(secret));
 assert.ok(!JSON.stringify(projectEvent({kind:'ceo_planning_completed',detail:{response:secret,error:secret,model:'local model',status:'failed'}} ,true)).includes(secret));
 const review=projectEvent({kind:'architecture_assessment_checkpoint',detail:{calls:[{agent:'judge',model:'pinned-reviewer',inference:{backend_pid:4,private:secret}}]}},true);
 assert.equal(review.call_snapshots[0].inference.backend_pid,4);assert.ok(!JSON.stringify(review).includes(secret));
});

test('follower tails new CEO and run events once, then stops without stopping the service',async t=>{
 let fresh=false,initialHistory;
 const seeded=new Promise(resolve=>{initialHistory=resolve;});
 const server=createServer((req,res)=>{
  res.setHeader('content-type','application/json');
  if(req.url==='/api/companies/company'&&req.method==='POST')res.end(JSON.stringify({events:fresh?[{seq:1,run_id:'planning',kind:'ceo_inference_requested',detail:{model:'local'}}]:[]}));
  else if(req.url==='/api/runs'){res.end(JSON.stringify({runs:fresh?[{id:'run',company_id:'company',updated_at_ms:2}]:[]}));initialHistory();}
  else res.end(JSON.stringify({events:[{seq:0,run_id:'run',kind:'call_started',model:'local',agent:'A'},{seq:1,run_id:'run',kind:'completed'}]}));
 });
 await new Promise(resolve=>server.listen(0,'127.0.0.1',resolve));t.after(()=>server.close());
 const controller=new AbortController(),logs=[];
 const running=followCompany({origin:`http://127.0.0.1:${server.address().port}`,company:'company',signal:controller.signal,pollMs:10,write:line=>logs.push(JSON.parse(line))});
 await seeded;fresh=true;
 const deadline=Date.now()+2000;
 while(logs.length<3&&Date.now()<deadline)await new Promise(resolve=>setTimeout(resolve,10));
 controller.abort();await running;
 assert.deepEqual(logs.map(e=>e.event).sort(),['call_started','ceo_inference_requested','completed']);
 assert.ok(server.listening);
});
