import assert from 'node:assert/strict';
import {mkdir,writeFile,readFile} from 'node:fs/promises';
import {chromium} from '@playwright/test';
const base=process.env.GENTS_SITE_URL||'http://127.0.0.1:4173';
if(!['http://127.0.0.1:4173','https://gents-office.vercel.app','https://metivolve.com'].includes(base))throw Error('Unreviewed browser target');
if(process.argv.includes('--send-owner-support'))throw Error('Routine verification does not send email. Use the separately retained owner delivery proof.');
const native=process.argv.includes('--with-local-companion'),folder=new URL('../data/mcp-basement-verification/',import.meta.url);await mkdir(folder,{recursive:true});
const browser=await chromium.launch({executablePath:chromium.executablePath(),headless:true});
try{
 const context=await browser.newContext();await context.addInitScript(()=>{window.__cspViolations=[];document.addEventListener('securitypolicyviolation',e=>window.__cspViolations.push(e.violatedDirective));});
 let nativePage,reportFile,priorCalls=null;const nativePosts=[];
 if(native){
  nativePage=await context.newPage();nativePage.on('request',r=>{if(r.method()==='POST')nativePosts.push(r.url());});
  const before=await context.request.get('http://127.0.0.1:8788/api/office/activity');priorCalls=(await before.json()).totals.calls;
  await nativePage.goto('http://127.0.0.1:8788/?autoconnect=1&onboarding=1',{waitUntil:'networkidle'});
  await nativePage.locator('#local-basic-answers[data-basic-answers-ready="true"]').waitFor();
  await nativePage.locator('#local-basic-answers select').selectOption('0');
  assert.match(await nativePage.locator('#local-basic-answers [data-basic-answer]').innerText(),/MIT/);
  const save=nativePage.getByRole('button',{name:'Save computer report',exact:true});await save.waitFor();await nativePage.waitForFunction(()=>!document.querySelector('[data-report]').disabled,{timeout:20000});
  const download=await Promise.all([nativePage.waitForEvent('download'),save.click()]);reportFile=new URL('metivolve-native-computer-check.json',folder).pathname;await download[0].saveAs(reportFile);
  const report=JSON.parse(await readFile(reportFile,'utf8'));assert.equal(report.type,'metivolve-computer-check');assert.ok(report.hardware.memory.host_total_bytes>0);assert.ok(!JSON.stringify(report).includes('PRIVATE'));
 }
 const page=await context.newPage(),errors=[],requests=[];page.on('pageerror',e=>errors.push(e.message));page.on('request',r=>requests.push({url:r.url(),method:r.method()}));
 await page.setViewportSize({width:1440,height:1000});await page.goto(base,{waitUntil:'networkidle'});await page.getByRole('heading',{level:1}).waitFor();
 await page.locator('[data-basic-answers-ready="true"]').waitFor();const beforeAnswer=requests.length;
 const basicAnswerMs=await page.evaluate(()=>{const node=document.querySelector('[data-basic-answers] select'),start=performance.now();node.value='0';node.dispatchEvent(new Event('change'));return performance.now()-start;});
 assert.match(await page.locator('[data-basic-answer]').innerText(),/MIT/);assert.equal(requests.length,beforeAnswer);assert.ok(basicAnswerMs<1000);
 await page.keyboard.press('Tab');assert.equal(await page.locator('.skip').evaluate(e=>e===document.activeElement),true);
 await page.getByRole('link',{name:'Plan my first task',exact:true}).click();
 await page.locator('#first-task[data-ready="true"]').waitFor();
 await page.getByRole('button',{name:'Continue to the result'}).click();assert.equal(await page.locator('[data-step]').nth(0).isVisible(),true);
 await page.locator('#new-goal').fill('Create a clear plan from my synthetic checklist.');await page.getByRole('button',{name:'Continue to the result'}).click();
 await page.locator('#new-output').fill('A weekly plan and a list of open questions.');await page.locator('#new-done').fill('Include every supplied item. Invent no facts. I review the result.');await page.getByRole('button',{name:'Check my computer',exact:true}).click();
 await page.waitForFunction(()=>!document.querySelector('#check-browser').disabled);assert.ok((await page.locator('#computer-result').innerText()).includes('No speed or finish time has been assumed.'));
 await page.getByText('Get a full local computer check',{exact:true}).click();
 if(native){const [popup]=await Promise.all([context.waitForEvent('page'),page.getByRole('link',{name:'Open the local computer check',exact:true}).click()]);await popup.waitForLoadState('networkidle');assert.ok(popup.url().startsWith('http://127.0.0.1:8788/'));await popup.waitForFunction(()=>document.querySelector('[data-report]')&&!document.querySelector('[data-report]').disabled,{timeout:20000});await popup.close();}

 if(reportFile)await page.locator('#computer-report').setInputFiles(reportFile);
 else {const gib=1024**3;await page.locator('#computer-report').setInputFiles({name:'synthetic-check.json',mimeType:'application/json',buffer:Buffer.from(JSON.stringify({type:'metivolve-computer-check',version:1,hardware:{memory:{host_total_bytes:36*gib,effective_limit_bytes:36*gib,effective_available_bytes:20*gib,observed_at_ms:Date.now()},accelerators:{devices:[{name:'SIMULATED GPU',memory_kind:'unified_shared_with_host'}]}},policy:{candidates:[{minimum_memory_mib:24576}]}}))});}
 await page.getByText('Imported file information is advisory',{exact:false}).waitFor();await page.getByRole('button',{name:'Review my next step'}).click();
 assert.ok((await page.locator('#brief-summary').innerText()).includes('Invent no facts.'));assert.ok((await page.locator('#readiness-summary').innerText()).includes('basic memory check passes'));
 const downloads=await Promise.all([page.waitForEvent('download'),page.getByRole('button',{name:'Save my task brief'}).click()]);const briefFile=new URL('metivolve-browser-task-brief.json',folder).pathname;await downloads[0].saveAs(briefFile);const brief=JSON.parse(await readFile(briefFile,'utf8'));assert.equal(brief.type,'metivolve-task-brief');assert.ok(!('tools' in brief));
 await page.screenshot({path:new URL('metivolve-onboarding-desktop.png',folder).pathname});
 if(native){await nativePage.getByRole('button',{name:'New conversation',exact:true}).click();await nativePage.locator('[data-brief]').setInputFiles(briefFile);await nativePage.getByText('Brief loaded. Review it',{exact:false}).waitFor();assert.ok((await nativePage.locator('#ceo-plan-message').inputValue()).includes('Invent no facts.'));const after=await context.request.get('http://127.0.0.1:8788/api/office/activity');assert.equal((await after.json()).totals.calls,priorCalls);assert.ok(!nativePosts.some(u=>u.includes('/planning/')||u.endsWith('/api/runs')));}
 await page.goto(base,{waitUntil:'networkidle'});await page.locator('#advanced-time summary').click();assert.equal(await page.locator('#prefill').inputValue(),'');await page.locator('#computer').selectOption({label:'Apple M5 family'});assert.equal(await page.locator('#generation').inputValue(),'');await page.locator('#prefill').fill('1000');await page.locator('#generation').fill('20');await page.getByRole('button',{name:'Calculate a scenario'}).click();assert.equal(await page.locator('#estimate strong').innerText(),'2.2 minutes');
 for(const[id,value]of Object.entries({'hardware-cost':'1000','api-spend':'50','power-watts':'100','compute-hours':'20','electricity-price':'0.2'}))await page.locator('#'+id).fill(value);await page.getByRole('button',{name:'Compare my scenario'}).click();assert.equal(await page.locator('#ownership-result strong').innerText(),'Illustrative payback: 20.2 months');await page.locator('#api-spend').fill('0');await page.getByRole('button',{name:'Compare my scenario'}).click();assert.equal(await page.locator('#ownership-result strong').innerText(),'No positive monthly saving in this scenario');
 assert.equal(await page.evaluate(()=>Object.keys(localStorage).length),0);assert.deepEqual(await context.cookies(),[]);assert.deepEqual(await page.evaluate(()=>window.__cspViolations),[]);await page.evaluate(()=>scrollTo(0,0));await page.screenshot({path:new URL('metivolve-desktop.png',folder).pathname});
 await page.goto(base+'/index.html#achievements',{waitUntil:'networkidle'});
 await page.locator('#achievements-title').waitFor();
 assert.equal(await page.locator('#achievements tbody tr').count(),16);
 assert.equal(await page.locator('#achievements img').evaluate(image=>image.complete&&image.naturalWidth===512&&image.naturalHeight===512),true);
 assert.match(await page.locator('.achievement-metrics').innerText(),/120\.29 seconds/);
 assert.equal(await page.getByRole('heading',{name:'A tested MCP workflow repair',exact:true}).count(),1);
 assert.match(await page.locator('#achievements').innerText(),/43,297 → 17,732 bytes/);
 assert.match(await page.locator('#achievements').innerText(),/37\.0 minutes/);
 assert.equal(await page.getByRole('heading',{name:'Research with official sources',exact:true}).count(),1);
 assert.match(await page.locator('#achievements').innerText(),/23\.3 minutes/);
 await page.screenshot({path:new URL('metivolve-achievements-desktop.png',folder).pathname});
 await page.setViewportSize({width:390,height:844});
 for(const path of ['index.html','start.html','guide.html','capabilities.html','faq.html','privacy.html','terms.html','readiness.html','support.html','cookbook.html']){await page.goto(`${base}/${path}`,{waitUntil:'networkidle'});assert.equal(await page.getByRole('heading',{level:1}).count(),1);assert.equal(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth),true,path);assert.deepEqual(await page.evaluate(()=>window.__cspViolations),[],path);if(path==='start.html')await page.screenshot({path:new URL('metivolve-onboarding-mobile.png',folder).pathname});}
 await page.goto(base+'/index.html#achievements',{waitUntil:'networkidle'});assert.equal(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth),true);await page.screenshot({path:new URL('metivolve-achievements-mobile.png',folder).pathname});
 assert.deepEqual(errors,[]);assert.ok(requests.every(r=>r.url.startsWith(base+'/')&&r.method==='GET'));
 const proof={status:'PASS',base,pages:10,desktop:[1440,1000],mobile:[390,844],page_errors:errors,external_origin_requests:0,website_posts:0,basic_answer:{render_ms:basicAnswerMs,extra_requests:0,model_calls:0,scope:'Known setup answers only'},task_brief:'browser-only download; no grants or dispatch',computer_check:native?'actual local GPU/memory report exported, imported into website, brief loaded into local CEO; no model calls':'explicit synthetic report; browser-only UI verification',time:'unknown until measured; entered scenario math verified',inference_calls:0,scope:'Website/companion onboarding verification only; no task execution or completion claim.'};await writeFile(new URL(base.startsWith('https:')?'metivolve-live-browser-proof.json':'metivolve-local-browser-proof.json',folder),JSON.stringify(proof,null,2)+'\n');console.log(JSON.stringify(proof));
}finally{await browser.close();}
