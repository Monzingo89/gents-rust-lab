import assert from 'node:assert/strict';import{chromium}from'@playwright/test';import{writeFile}from'node:fs/promises';
const browser=await chromium.launch({executablePath:chromium.executablePath(),headless:true});
try{
 const page=await browser.newPage();const errors=[];page.on('pageerror',e=>errors.push(e.message));
 await page.goto('http://127.0.0.1:8788/?autoconnect=1',{waitUntil:'networkidle'});
 assert.equal(await page.locator('#ceo-reasoning-only').count(),1);
 await page.evaluate(async()=>{
  const {architectureReviewPanel}=await import('/assets/architecture-review.js');
  window.__reviewCalls=[];
  const result={status:'ready_for_owner_review',request:{speed:'patient',maximum_elapsed_seconds:1209600},recommendation:{architecture:'ordered',agent_count:1,rationale:'SIMULATED <img src=x onerror=alert(1)>'},review:{verdict:'accept',architecture:'ordered',agent_count:1,rationale:'SIMULATED review'},execution_authorized:false};
  window.__reviewPanel=architectureReviewPanel({context:()=>({company:'SIMULATED',session:'SIMULATED',revision:2,visible:true}),invalidate:()=>{},request:async(path,options)=>{window.__reviewCalls.push(options?.method||'GET');if(path.endsWith('/hardware'))return {accelerators:{devices:[{name:'SIMULATED GPU'}]},memory:{host_total_bytes:1024**3}};if(options)throw Error('SIMULATED lost response');return result;}});
  const host=document.createElement('div');host.id='review-browser-fixture';host.append(window.__reviewPanel.element);document.body.append(host);window.__reviewPanel.update({status:'ready',revision:2});
 });
 const host=page.locator('#review-browser-fixture');await host.getByRole('button',{name:'Ask CEO and local judge'}).click();
 await host.getByText('SIMULATED lost response',{exact:false}).waitFor();assert.equal(await host.locator('[data-start]').isDisabled(),true);
 await host.locator('[data-inspect]').click();await host.getByText('SIMULATED review',{exact:false}).waitFor();
 assert.equal(await host.locator('img').count(),0);assert.ok((await host.innerText()).includes('<img src=x'));
 await host.locator('[data-speed]').selectOption('fast');await host.locator('[data-inspect]').click();
 await host.getByText('Earlier review retained. Preferences changed; request a fresh review.',{exact:true}).waitFor();
 assert.equal(await page.evaluate(()=>window.__reviewPanel.latest()),null);
 assert.equal(await page.evaluate(()=>window.__reviewCalls.filter(m=>m==='POST').length),1);
 await page.evaluate(()=>window.__reviewPanel.destroy());assert.deepEqual(errors,[]);
 const proof={status:'PASS',model_calls:0,real_architecture_posts:0,scope:'Actual office DOM/module with explicitly simulated review transport: lost response read recovery, no POST replay, stale preference guard and safe text rendering. This is not native inference or task qualification.'};
 await writeFile('data/mcp-basement-verification/architecture-review-browser-proof.json',JSON.stringify(proof,null,2)+'\n');console.log(JSON.stringify(proof));
}finally{await browser.close();}
