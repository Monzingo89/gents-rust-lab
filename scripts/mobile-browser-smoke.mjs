// Real HTTPS/cookies/proxy, but isolated SIMULATED jobs: no model runs or emails.
import assert from 'node:assert/strict';
import http from 'node:http';
import https from 'node:https';
import { once } from 'node:events';
import { mkdtempSync, readFileSync, rmSync } from 'node:fs';
import { join, resolve, extname } from 'node:path';
import { tmpdir } from 'node:os';
import { chromium } from '@playwright/test';
import { createLanHandler, localCertificate, selectLanAddress } from './mobile-access.mjs';

const root=resolve('web'),directory=mkdtempSync(join(tmpdir(),'gents-mobile-test-'));
const code='SIMULATED-fixture-code',address=selectLanAddress();let gateway,upstream,browser;
const organization=JSON.parse(readFileSync('config/organization.json','utf8'));
const localConfig=JSON.parse(readFileSync('config/company.local.json','utf8'));
const config={company_mode:true,company_tools:true,auth:'local',company_name:'Mobile fixture company',admin_judge:'Qwen3-8B Admin Judge',tools:{},organization,
 agents:localConfig.agents.map(a=>({id:a.id,specialty:a.assignment,planner_model:a.planner.model,worker_model:a.worker.model,planner_provider:'local',worker_provider:'local'}))};
let runs=[],submitted=null,needsAnswer=false,answered=false;const ops=[],requests=[];
const event=(kind='call_started')=>({seq:0,timestamp_ms:Date.now(),agent:'Engineer Gent',role:'worker',room:'workshop',kind,model:'local / Qwen3-Coder-30B-A3B-Instruct',message:'SIMULATED mobile integration fixture',call_id:'fixture-call'});
const snapshot=id=>({id,company_id:'we-need-gents-lc',status:answered?'completed':'running',events:[event(answered?'completed':'call_started')]});
try{
 upstream=http.createServer(async(req,res)=>{
  const url=new URL(req.url,'http://fixture');requests.push({method:req.method,path:url.pathname});
  let raw='';for await(const chunk of req)raw+=chunk;const payload=raw?JSON.parse(raw):null;
  const json=value=>{res.setHeader('Content-Type','application/json');res.end(JSON.stringify(value));};
  if(url.pathname==='/api/config')return json(config);
  if(url.pathname==='/api/runs'&&req.method==='GET')return json({runs});
  if(url.pathname==='/api/runs'&&req.method==='POST'){
   submitted=payload;runs=[{id:payload.run_id,company_id:'we-need-gents-lc',status:'running',task:payload.task,created_at_ms:Date.now()}];return json({id:payload.run_id});
  }
  if(url.pathname==='/api/companies/we-need-gents-lc'){
   ops.push(payload.op);
   if(payload.op==='intake'||payload.op==='approve')return json({name:'Mobile fixture company',jobs:[{run_id:payload.run_id,digest:'a'.repeat(64)}]});
   return json({name:'Mobile fixture company',tiers:[],jobs:runs.map(r=>({run_id:r.id,state:answered?'completed':'running'})),training:[],meetings:[],milestones:[]});
  }
  if(/^\/api\/runs\/[^/]+\/attention$/.test(url.pathname)){
   if(req.method==='POST'){assert.equal(payload.answer,'SIMULATED operator answer');answered=true;runs[0].status='completed';return json({answered:true});}
   return json({attention:needsAnswer&&!answered?{id:'fixture-question',question:'SIMULATED: Which source should I use?'}:null});
  }
  if(/^\/api\/runs\/[^/]+$/.test(url.pathname))return json(snapshot(url.pathname.split('/').at(-1)));
  if(req.method!=='GET'){res.writeHead(404);return res.end();}
  const file=resolve(root,url.pathname==='/'?'index.html':'.'+url.pathname);
  if(!file.startsWith(root+'/')){res.writeHead(403);return res.end();}
  try{res.setHeader('Content-Type',({'.html':'text/html','.js':'text/javascript','.css':'text/css','.png':'image/png','.jpg':'image/jpeg'})[extname(file)]||'application/octet-stream');res.end(readFileSync(file));}catch{res.writeHead(404);res.end();}
 });upstream.listen(0,'127.0.0.1');await once(upstream,'listening');
 let handler;gateway=https.createServer(localCertificate(directory,address),(req,res)=>handler(req,res));gateway.listen(0,address);await once(gateway,'listening');
 const origin=`https://${address}:${gateway.address().port}`;
 handler=createLanHandler({origin,upstream:`http://127.0.0.1:${upstream.address().port}`,code,computer:'SIMULATED laptop'});
 browser=await chromium.launch({channel:'chrome',headless:true});
 const context=await browser.newContext({viewport:{width:390,height:844},isMobile:true,hasTouch:true,ignoreHTTPSErrors:true});
 const page=await context.newPage(),errors=[];page.on('pageerror',e=>errors.push(e.message));
 await page.goto(origin);
 await page.getByRole('heading',{name:'Your laptop. Your Gents.'}).waitFor();
 assert.equal(requests.length,0,'Unauthenticated page must never reach the laptop harness');
 await page.getByLabel('Pairing code').fill(code);await page.getByRole('button',{name:'Connect to my laptop'}).click();
 await page.getByText(/Running on SIMULATED laptop/).waitFor();
 await page.waitForFunction(()=>document.querySelector('#status').textContent.startsWith('Connected'));
 assert.equal(await page.locator('#base').inputValue(),origin,'Phone API base must be its HTTPS origin, not localhost');
 const cookie=(await context.cookies()).find(c=>c.name==='__Host-gents_mobile');assert.ok(cookie.secure&&cookie.httpOnly);
 assert.equal(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth),true);
 await page.getByRole('slider',{name:'Office zoom'}).fill('200');
 assert.ok(await page.locator('.tower-stage').evaluate(stage=>stage.scrollWidth>stage.clientWidth));
 assert.equal(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth),true);
 await page.getByRole('slider',{name:'Office zoom'}).fill('100');
 // Floor close-ups and animation remain laptop-independent UI operations.
 await page.locator('[data-floor="4"]').tap();
 assert.equal(await page.getByRole('slider',{name:'Office zoom'}).inputValue(),'200');
 await page.locator('#preview-walking').tap();
 await page.waitForFunction(()=>document.querySelector('#office').gentLayout.length>0&&document.querySelector('#office').gentLayout.every(s=>s.floor===4&&s.preview));
 assert.equal(await page.locator('#walking-preview-warning').isVisible(),true);
 assert.equal(submitted,null,'Walking preview must not start a task');
 assert.equal(await page.locator('#company-signal').getAttribute('data-state'),'red');
 const beforeX=await page.locator('#office').evaluate(c=>c.gentLayout[0].hit.x);
 await page.waitForFunction(x=>Math.abs(document.querySelector('#office').gentLayout[0].hit.x-x)>4,beforeX);
 await page.locator('#office').evaluate(c=>c.scrollIntoView({behavior:'instant',block:'start'}));
 const sprite=await page.locator('#office').evaluate(c=>{const r=c.getBoundingClientRect(),s=c.gentLayout[0],h=s.hit;return{x:r.left+(h.x+h.w/2)*r.width/c.width,y:r.top+(h.y+h.h/2)*r.height/c.height,character:s.character};});
 await page.touchscreen.tap(sprite.x,sprite.y);
 await page.locator('#gent-drawer').waitFor({state:'visible'});
 assert.equal(await page.locator('.gent-portrait').getAttribute('data-character'),sprite.character);
 await page.locator('#close-gent').tap();
 await page.screenshot({path:'/tmp/gents-mobile-floor.png',fullPage:false});
 await page.locator('[data-floor="all"]').tap();
 await page.waitForFunction(()=>document.querySelector('#office').dataset.view==='building');
 assert.equal(await page.getByRole('slider',{name:'Office zoom'}).inputValue(),'100');
 assert.equal(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth),true);
 await page.getByRole('navigation',{name:'Mobile company navigation'}).getByRole('button',{name:'Ask',exact:true}).click();
 await page.locator('#task').fill('SIMULATED task requested from a phone');await page.locator('#consent').check();await page.locator('#run').click();
 await page.waitForFunction(()=>document.querySelector('#company-signal').dataset.state==='green');
 assert.equal(submitted.task,'SIMULATED task requested from a phone');assert.deepEqual(ops.filter(op=>['intake','approve'].includes(op)),['intake','approve']);
 // A fresh phone tab discovers laptop-owned work without any browser-local run ID.
 const observer=await context.newPage();await observer.goto(origin+'/?autoconnect=1');
 await observer.waitForFunction(()=>document.querySelector('#run-info').textContent.includes('running'));
 await observer.locator('#mobile-task-text').waitFor({state:'attached'});assert.match(await observer.locator('#mobile-task-text').textContent(),/SIMULATED task/);
 await page.close();assert.equal(runs[0].status,'running','Closing the initiating phone tab must not cancel its laptop task');
 needsAnswer=true;await observer.locator('#attention-panel').waitFor({state:'visible'});
 assert.equal(await observer.locator('#company-signal').getAttribute('data-state'),'yellow');
 await observer.locator('#attention-answer').fill('SIMULATED operator answer');await observer.getByRole('button',{name:'Answer & continue →'}).click();
 await observer.waitForFunction(()=>document.querySelector('#run-info').textContent.includes('completed'));
 await observer.screenshot({path:'/tmp/gents-mobile-control.png',fullPage:false});
 // An already-open idle phone also picks up a new job started by another client.
 answered=false;runs=[{id:'new-laptop-fixture',status:'running',task:'SIMULATED new laptop task',created_at_ms:Date.now(),company_id:'we-need-gents-lc'}];needsAnswer=false;
 await observer.waitForFunction(()=>document.querySelector('#run-info').textContent.includes('new-laptop-fixture'));
 await observer.getByRole('button',{name:'Disconnect this phone'}).click();
 await observer.getByRole('heading',{name:'Your laptop. Your Gents.'}).waitFor();
 assert.equal(runs[0].status,'running');assert.equal(requests.filter(r=>r.path.endsWith('/cancel')).length,0);
 assert.deepEqual(errors,[]);
 console.log('PASS: real LAN HTTPS gateway + pairing/cookie security, 390px mobile UI/zoom, task submission to isolated laptop fixture, cross-tab discovery, close-tab continuity, yellow question/answer, auto-follow and disconnect');
}finally{
 await browser?.close();for(const server of [gateway,upstream])if(server){server.closeAllConnections();await new Promise(resolve=>server.close(resolve));}
 rmSync(directory,{recursive:true,force:true});
}
