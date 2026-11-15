// Dedicated regression fixture for this app, not a live-company task.
import assert from 'node:assert/strict';
import {mkdir,writeFile} from 'node:fs/promises';
import {chromium} from '@playwright/test';
const origin=process.env.GENTS_UI_URL||'http://127.0.0.1:8788';
const output=new URL('../data/office-activity/verification/',import.meta.url);await mkdir(output,{recursive:true});
const startedAt=Date.now();await writeFile(new URL('browser-result.json',output),JSON.stringify({status:'RUNNING',started_at_ms:startedAt}));
const browser=await chromium.launch({channel:'chrome',headless:true});
const errors=[],requests=[];let page,lastHover;
try{
 page=await browser.newPage({viewport:{width:1600,height:1100}});
 page.on('pageerror',e=>errors.push(e.message));page.on('request',r=>requests.push(r.url()));
 await page.goto(origin+'/?autoconnect=1&tour=1');
 await page.waitForFunction(()=>document.querySelector('#office').gentLayout?.length===17&&Number(document.querySelector('#office').dataset.telemetryAt)>0);
 const before=await (await page.request.get(origin+'/api/office/activity')).json();
 const exterior=await page.locator('#office').evaluate(c=>({height:c.height,crew:c.gentLayout,elevator:c.elevator,screenCount:c.dashboardHits.length}));
 assert.ok(exterior.height>5000);assert.equal(exterior.crew.filter(c=>c.resting).length,2);assert.equal(new Set(exterior.crew.map(c=>c.character)).size,17);assert.ok(exterior.screenCount>=20);
 await page.screenshot({path:new URL('building.png',output).pathname});
 // Observe the actual rendered CFO journey in real time, not injected poses.
 await page.locator('[data-floor="9"]').click();
 await page.waitForFunction(()=>document.querySelector('#office').gentLayout?.[0]?.state==='walking');
 await page.mouse.move(0,0);
 const first=await page.locator('#office').evaluate(c=>({...c.gentLayout[0].hit}));
 await page.waitForFunction(start=>{const s=document.querySelector('#office').gentLayout[0];return s?.state==='walking'&&Math.hypot(s.hit.x-start.x,s.hit.y-start.y)>35;},first,{timeout:10000});
 await page.waitForFunction(()=>document.querySelector('#office').gentLayout[0]?.state==='interacting',null,{timeout:12000});
 const interaction=await page.locator('#office').evaluate(c=>c.gentLayout[0]);
 assert.equal(interaction.object,'finance-books');
 await page.locator('#office').screenshot({path:new URL('cfo-interacting.png',output).pathname});
 await page.waitForFunction(()=>document.querySelector('#office').gentLayout[0]?.state==='seated',null,{timeout:30000});
 const seated=await page.locator('#office').evaluate(c=>c.gentLayout[0]);
 assert.equal(seated.object,'chair');assert.ok(Math.abs(seated.hit.x+seated.hit.w/2-833)<1&&Math.abs(seated.hit.y+seated.hit.h-569)<1,'return to the assigned photographic chair anchor');
 await page.locator('#office').screenshot({path:new URL('cfo-seated.png',output).pathname});
 let all=[];const rooms={};let hoverCount=0;
 for(const floor of [11,10,9,8,7,6,5,4,3,2,1]){
  await page.locator(`[data-floor="${floor}"]`).click();
  await page.waitForFunction(f=>document.querySelector('#office').interiorDesign?.floor===f&&document.querySelector('#office').interiorDesign?.rendering==='photoreal-architectural',floor);
  await page.locator('#office').scrollIntoViewIfNeeded();
  const data=await page.locator('#office').evaluate(c=>({crew:c.gentLayout,screens:c.dashboardHits,elevator:c.elevator}));rooms[floor]={occupants:data.crew.length,screens:data.screens.map(s=>s.id)};all.push(...data.crew);
  for(const gent of data.crew)assert.equal(gent.floor,floor);
  for(const gent of data.crew){
   const point=await page.locator('#office').evaluate((c,key)=>{const s=c.gentLayout.find(s=>s.key===key),r=c.getBoundingClientRect();return{x:r.left+(s.hit.x+s.hit.w/2)*r.width/c.width,y:r.top+(s.hit.y+s.hit.h*.3)*r.height/c.height};},gent.key);
   lastHover={floor,key:gent.key,point};await page.mouse.move(point.x,point.y);await page.locator('#gent-hover-preview').waitFor({state:'visible',timeout:5000});
   assert.equal(await page.locator('#gent-hover-preview').getAttribute('data-character'),gent.character,`hover ${gent.key}`);hoverCount++;
  }
  await page.mouse.move(0,0);await page.locator('#gent-hover-preview').waitFor({state:'hidden'});
  if([10,9,5,4].includes(floor))await page.locator('#office').screenshot({path:new URL(`floor-${floor}.png`,output).pathname});
 }
 assert.equal(all.length,17);assert.equal(all.filter(c=>c.resting).length,2);
 assert.deepEqual(rooms[9].screens,['budget']);assert.equal(rooms[9].occupants,1);assert.equal(rooms[10].occupants,1);
 await page.locator('[data-floor="5"]').click();
 await page.waitForFunction(()=>document.querySelector('#office').interiorDesign?.floor===5);
 const point=await page.locator('#office').evaluate(c=>{const s=c.dashboardHits.find(s=>s.id==='gpu'),r=c.getBoundingClientRect();return{x:r.left+(s.x+s.w/2)*r.width/c.width,y:r.top+(s.y+s.h/2)*r.height/c.height};});
 await page.mouse.move(point.x,point.y);await page.locator('.office-dashboard[data-dashboard="gpu"]').waitFor({state:'visible'});
 assert.match(await page.locator('.office-dashboard').innerText(),/macOS IOAccelerator|GPU sensor unavailable/);
 await page.mouse.click(point.x,point.y);await page.getByText('Pinned',{exact:false}).waitFor();
 const sampled=await page.locator('#office').getAttribute('data-telemetry-at');
 await page.waitForFunction(previous=>document.querySelector('#office').dataset.telemetryAt!==previous,sampled,{timeout:15000});
 await page.screenshot({path:new URL('operations-dashboard.png',output).pathname});
 await page.getByRole('button',{name:'Close dashboard'}).click();
 const gent=await page.locator('#office').evaluate(c=>{const s=c.gentLayout[0],r=c.getBoundingClientRect();return{x:r.left+(s.hit.x+s.hit.w/2)*r.width/c.width,y:r.top+(s.hit.y+s.hit.h*.3)*r.height/c.height,character:s.character};});
 await page.mouse.move(gent.x,gent.y);await page.locator('#gent-hover-preview').waitFor({state:'visible'});
 assert.equal(await page.locator('#gent-hover-preview').getAttribute('data-character'),gent.character);
 assert.match(await page.locator('#gent-hover-preview').innerText(),/staged room activity/);
 await page.screenshot({path:new URL('gent-hover.png',output).pathname});
 const after=await (await page.request.get(origin+'/api/office/activity')).json();
 assert.ok(after.sampled_at_ms>before.sampled_at_ms);assert.equal(after.research.cycles,before.research.cycles,'screen polling must not trigger new inference');
 assert.equal(after.totals.provider_cost_usd,0);assert.ok(after.research.enabled);
 assert.ok(requests.every(url=>url.startsWith(origin)||url.startsWith('data:')),'no external calls from presentation');
 assert.deepEqual(errors,[]);
 const report={status:'PASS',started_at_ms:startedAt,rooms,occupants:all.length,sleepers:2,screens:exterior.screenCount,room_journey:{real_time:true,walking:true,object_interaction:interaction.object,returned_to_chair:seated.object},live_updates:true,gent_hover:true,gent_hover_count:hoverCount,screen_hover:true,provider_cost_usd:0,research_cycles:after.research.cycles,page_errors:errors,local_requests_only:true};
 await writeFile(new URL('browser-result.json',output),JSON.stringify(report,null,2));console.log(JSON.stringify(report));
}catch(error){await page?.screenshot({path:new URL('failure.png',output).pathname});const report={status:'FAIL',started_at_ms:startedAt,lastHover,error:String(error),page_errors:errors};await writeFile(new URL('browser-result.json',output),JSON.stringify(report,null,2));console.error(JSON.stringify(report));throw error;}finally{await browser.close();}
