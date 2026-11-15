// Read-only replay of an independently verified real local task. Every frame
// is labeled REPLAY; no new tasks, inference, fabricated results or grants.
import assert from 'node:assert/strict';
import {chromium} from '@playwright/test';
import {readFile,writeFile,mkdir} from 'node:fs/promises';
import {resolve,join} from 'node:path';
import {createHash} from 'node:crypto';
const folder=resolve(process.argv[2]),freeze=JSON.parse(await readFile(join(folder,'freeze.json'),'utf8'));
const proof=JSON.parse(await readFile(join(folder,'independent-verification.json'),'utf8'));
assert.equal(proof.status,'PASS');assert.equal(proof.run,freeze.run);assert.equal(freeze.base,'http://127.0.0.1:8788');
const bytes=await readFile(join(folder,'observed-run.json')),recorded=JSON.parse(bytes);
assert.equal(recorded.status,'completed');assert.equal(recorded.run_id,freeze.run);
const frames=join(folder,'replay-frames');await mkdir(frames,{recursive:true});
const browser=await chromium.launch({executablePath:chromium.executablePath(),headless:true});
let cutoff=recorded.events.findIndex(e=>e.kind==='call_started');assert(cutoff>=0);
const calls=recorded.events.filter(e=>e.kind==='call_started');assert.equal(calls.length,2);
let phase='Engineer writing';const errors=[],captured=[];
const prefix=()=>recorded.events.slice(0,cutoff+1),done=()=>cutoff===recorded.events.length-1;
try {
 const page=await browser.newPage({viewport:{width:1440,height:1050}});
 await page.addInitScript(stamp=>{const real=Date.now;window.__replayNow=stamp;Date.now=()=>window.__replayNow??real();},recorded.events[cutoff].timestamp_ms);
 page.on('pageerror',e=>errors.push(e.message));
 await page.route('**/api/office/activity',route=>route.fulfill({json:{available:false,history:[],scope:'Historical host telemetry unavailable in recorded replay'}}));
 await page.route('**/api/companies/**',route=>{
  if(route.request().method()==='POST' && route.request().postDataJSON()?.op!=='status')return route.abort();
  return route.continue();
 });
 await page.route('**/api/runs',route=>route.request().method()==='GET'?route.continue():route.abort());
 await page.route('**/api/runs/'+freeze.run,route=>route.request().method()==='GET'?route.fulfill({json:{...recorded,events:prefix(),status:done()?'completed':'running'}}):route.abort());
 await page.route('**/api/runs/'+freeze.run+'/observability',route=>route.fulfill({json:{version:1,run_id:freeze.run,status:'recorded_replay',calls:[],context_handoffs:[],issues:[],feedback_loops:[],assurance:{compliance_status:'recorded_task_not_certification'}}}));
 await page.goto(freeze.base+'/?autoconnect=1',{waitUntil:'networkidle'});
 await page.locator('#company-id').fill(freeze.company);await page.locator('#watch-company').click();
 await page.evaluate(()=>{const label=document.createElement('div');label.id='verified-replay-label';label.style.cssText='position:fixed;top:0;left:0;right:0;z-index:9999;padding:12px 20px;background:#08232a;color:#d4fff3;font:600 17px system-ui;box-shadow:0 2px 8px #0008';document.body.append(label);});
 const stages=[
  {cutoff,phase:'Engineer writing from the frozen brief',floor:'6',active:true},
  {cutoff:recorded.events.findIndex(e=>e.kind==='coordination_candidate_submitted'),phase:'Candidate submitted · final acceptance still required',floor:'6'},
  {cutoff:recorded.events.findIndex(e=>e.kind==='call_started'&&e.role==='reasoning_acceptance'),phase:'Independent local reviewer checks every criterion',floor:'2',active:true},
  {cutoff:recorded.events.length-1,phase:'Completed · exact two-sentence fixture independently verified',floor:'2'},
 ];
 for(const stage of stages){
  assert(stage.cutoff>=0);cutoff=stage.cutoff;phase=stage.phase;
  await page.evaluate(stamp=>{window.__replayNow=stamp;},recorded.events[cutoff].timestamp_ms);
  await page.locator('#verified-replay-label').evaluate((el,text)=>el.textContent='REPLAY OF A REAL LOCAL TASK · '+text+' · $0 provider fees',phase);
  await page.locator(`[data-floor="${stage.floor}"]`).click();await page.locator('#office').scrollIntoViewIfNeeded();
  await page.waitForTimeout(1900);
  await page.waitForFunction(f=>document.querySelector('#office').interiorDesign?.floor===Number(f),stage.floor);
  const layout=await page.locator('#office').evaluate(c=>c.gentLayout.map(x=>({key:x.key,resting:x.resting,floor:x.floor})));
  if(stage.active&&!layout.some(x=>x.resting===false&&x.key===(stage.floor==='6'?'Engineer Gent:worker':'Independent Reviewer:judge'))){console.log(JSON.stringify({phase,layout,status:await page.locator('#status').innerText(),info:await page.locator('#run-info').innerText(),error:await page.locator('#error').innerText()}));await page.screenshot({path:join(folder,'replay-error.png')});throw Error('Actual recorded native phase must activate a visible Gent');}
  for(let i=0;i<10;i++){
   const name=`frame-${String(captured.length).padStart(4,'0')}.png`;
   await page.screenshot({path:join(frames,name)});captured.push({frame:name,event_prefix_through:cutoff,phase});
   await page.waitForTimeout(150);
  }
 }
 assert.deepEqual(errors,[]);
 const report={status:'PASS',run:freeze.run,source_run_sha256:createHash('sha256').update(bytes).digest('hex'),independent_verification:proof,frames:captured,model_calls_during_replay:0,task_mutations_during_replay:0,scope:'Explicitly labeled visual replay of a verified actual local run. Native events are exact retained prefixes. Character motion illustrates recorded activity; it is not a new live execution or a performance benchmark. Room/assurance telemetry suppressed as unavailable, and the display clock follows actual event timestamps rather than replay wall time. No future or invented measurements.'};
 await writeFile(join(folder,'replay.json'),JSON.stringify(report,null,2)+'\n');console.log(JSON.stringify({status:report.status,run:freeze.run,frames:captured.length}));
} finally {await browser.close();}
