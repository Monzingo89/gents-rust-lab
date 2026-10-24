import assert from 'node:assert/strict';
import { chromium } from '@playwright/test';

const browser = await chromium.launch({ channel: 'chrome', headless: true });
try {
  const page = await browser.newPage({ viewport: { width: 1600, height: 1100 } });
  await page.addInitScript(()=>{
    window.__gentArt=new Set();window.__gentLabels=new Set();
    const draw=CanvasRenderingContext2D.prototype.drawImage,text=CanvasRenderingContext2D.prototype.fillText;
    CanvasRenderingContext2D.prototype.drawImage=function(img,...args){if(this.canvas.id==='office'&&img?.src?.includes('/assets/gents/')){window.__gentArt.add(img.src);const [sx,sy,sw,sh,x,y,w,h]=args;window.__lastGent={src:img.src,cell:Math.floor((sx+sw/2)/(img.naturalWidth/2))+2*Math.floor((sy+sh/2)/(img.naturalHeight/2)),x,y,w,h};}return draw.call(this,img,...args);};
    CanvasRenderingContext2D.prototype.fillText=function(value,...args){if(['ZZZZ','Working on the','mission'].includes(value))window.__gentLabels.add(value);return text.call(this,value,...args);};
  });
  const errors = [];
  page.on('pageerror', error => errors.push(error.message));
  // Optional read-only idle fixture when the actual company has a live task.
  // Later test routes handle fake submissions; never mutate/cancel a real job.
  if(process.env.GENTS_UI_IDLE_FIXTURE==='1'){
    await page.route('**/api/runs',route=>route.request().method()==='GET'?route.fulfill({contentType:'application/json',body:JSON.stringify({runs:[]})}):route.continue());
    await page.route('**/api/companies/we-need-gents-lc',async route=>{
      if(route.request().postDataJSON()?.op!=='status')return route.continue();
      const response=await route.fetch();const state=await response.json();state.jobs=[];
      return route.fulfill({response,json:state});
    });
  }
  await page.goto((process.env.GENTS_UI_URL || 'http://127.0.0.1:8788')+'/?autoconnect=1');
  await page.getByRole('button', { name: 'Watch company work', exact: true }).waitFor({ state: 'visible' });
  await page.getByRole('button', { name: 'Watch company work', exact: true }).click();
  await page.getByRole('button', { name: 'Ask For a Task', exact: true }).waitFor();
  await page.getByText('Gents work log', { exact: true }).waitFor();
  await page.getByRole('heading', { name: 'Training Room', exact: true }).waitFor();
  await page.getByRole('heading', { name: 'CEO & all-hands', exact: true }).waitFor();
  await page.locator('.ops-map > summary').click();
  await page.getByRole('button', { name: 'CEO - Qwen3-8B', exact: false }).waitFor();
  const roster=page.locator('#org-chart');
  await roster.getByRole('button', { name: 'PM - Phi-4' }).click();
  await page.locator('#gent-drawer').waitFor({ state: 'visible' });
  await page.waitForFunction(()=>{const img=document.querySelector('.gent-portrait img');return img?.complete&&img.naturalWidth>0;});
  assert.equal(await page.locator('.gent-portrait').getAttribute('data-character'),'octopus');
  assert.equal(await page.locator('.gent-portrait').getAttribute('data-position'),'pm:worker');
  const initialLaw=await roster.getByRole('button',{name:'Law - Phi-4'}).getAttribute('data-character');
  const cto=await roster.getByRole('button',{name:'CTO - Qwen3-8B'}).getAttribute('data-character');
  const ceo=await roster.getByRole('button',{name:'CEO - Qwen3-8B'}).getAttribute('data-character');
  assert.notEqual(cto,ceo,'Positions sharing Qwen must have different default faces');
  await page.locator('.gent-wardrobe summary').click();
  assert.equal(await page.locator('.character-choice').count(),20);
  assert.ok(await page.locator('.character-choice:disabled').count()>=15,'Assigned faces must not be selectable by another Gent');
  await page.getByRole('button',{name:'Jade spirit',exact:true}).click();
  assert.equal(await page.locator('.gent-portrait').getAttribute('data-character'),'slime');
  assert.equal(await page.evaluate(()=>JSON.parse(localStorage.getItem('gents:unique-crew:v2'))['pm:worker']),'slime');
  assert.equal(await roster.getByRole('button',{name:'PM - Phi-4'}).getAttribute('data-character'),'slime');
  assert.equal(await roster.getByRole('button',{name:'Law - Phi-4'}).getAttribute('data-character'),initialLaw,'Changing PM must not change another Phi-4 position');
  await page.getByRole('button',{name:'Coral · Octopus',exact:true}).click();
  const pictures=await page.evaluate(async()=>{const module=await import('/gent-avatars.js');return module.characters.map(c=>c.src);});
  for(const src of pictures){const response=await page.request.get(src);assert.equal(response.status(),200);assert.match(response.headers()['content-type'],/image\/(jpeg|png)/);}
  await page.locator('[data-floor="2"]').click();
  await page.waitForFunction(()=>window.__gentArt.size>=2&&window.__gentLabels.has('ZZZZ'));
  assert.ok(await page.evaluate(()=>[...window.__gentArt].every(src=>/fullbody-\d-v1\.png$/.test(src))),'The scene must draw full-body sprites, never rectangular portrait JPGs');
  const sheets=await page.evaluate(async()=>{const module=await import('/gent-avatars.js');return module.spriteSheets;});
  for(const src of sheets){const response=await page.request.get(src);assert.equal(response.status(),200);assert.match(response.headers()['content-type'],/image\/png/);}
  assert.match(await page.locator('#company-name').textContent(), /We Need Gents LLC/i);
  assert.match(await page.locator('#company-context').textContent(), /PSTC/);
  assert.equal(await page.locator('#run').isDisabled(), false);
  assert.equal(await page.locator('#task').isEditable(), true);
  await page.locator('#close-gent').click();
  await page.locator('.ops-map > summary').click();
  await page.locator('[data-floor="all"]').click();
  await page.evaluate(()=>window.scrollTo(0,0));
  await page.screenshot({path:process.env.GENTS_SCREENSHOT||'data/office-ui.png',fullPage:false});
  assert.equal(await page.locator('#company-signal').getAttribute('data-state'),'red');
  await page.waitForFunction(()=>document.querySelector('#office').dataset.view==='building');
  assert.equal(await page.locator('#office').evaluate(c=>c.gentLayout.length),0);
  assert.equal(await page.locator('#office').evaluate(c=>c.floorHits.length),6);
  await page.locator('#office').screenshot({path:'/tmp/gents-tower-idle.png'});
  // Enter a floor by selecting the actual glazed window band.
  await page.locator('#office').scrollIntoViewIfNeeded();
  const windowPoint=await page.locator('#office').evaluate(c=>{const r=c.getBoundingClientRect(),h=c.floorHits.find(h=>h.id===2);return{x:r.left+(h.x+h.w/2)*r.width/c.width,y:r.top+(h.y+h.h/2)*r.height/c.height};});
  await page.mouse.click(windowPoint.x,windowPoint.y);
  await page.waitForFunction(()=>document.querySelector('#office').height===1000&&document.querySelector('#office').gentLayout.length===10);
  const layout=[];
  for(const floor of ['2','1']){
    await page.locator(`[data-floor="${floor}"]`).click();
    await page.waitForFunction(f=>document.querySelector('#office').gentLayout.length>0&&document.querySelector('#office').gentLayout.every(s=>s.floor===f),Number(floor));
    const visible=await page.locator('#office').evaluate(c=>c.gentLayout);layout.push(...visible);
    assert.ok(visible.every(s=>s.resting),'Idle Gents must be in recharge pods');
    for(let i=0;i<visible.length;i++)for(let j=i+1;j<visible.length;j++){const a=visible[i].hit,b=visible[j].hit;assert.ok(!(a.x<b.x+b.w&&a.x+a.w>b.x&&a.y<b.y+b.h&&a.y+a.h>b.y),'Painted Gents may not overlap');}
  }
  assert.equal(layout.length,16);assert.equal(new Set(layout.map(s=>s.character)).size,16);
  await page.locator('[data-floor="2"]').click();
  await page.waitForFunction(()=>document.querySelector('#office').gentLayout.length===10);
  assert.equal(await page.locator('#office').evaluate(c=>c.interiorDesign.camera),'oblique-35deg');
  await page.locator('#office').screenshot({path:'/tmp/gents-floor-recharge.png'});
  await page.locator('#office').scrollIntoViewIfNeeded();
  await page.evaluate(()=>new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve))));
  const hovered=await page.evaluate(async()=>{
    const canvas=document.querySelector('#office'),box=canvas.getBoundingClientRect(),s=canvas.gentLayout[0],h=s.hit;
    return {x:box.left+(h.x+h.w*.5)*box.width/canvas.width,y:box.top+(h.y+h.h*.4)*box.height/canvas.height,character:s.character};
  });
  await page.mouse.move(hovered.x,hovered.y);
  await page.locator('#gent-hover-preview').waitFor({state:'visible'});
  assert.equal(await page.locator('#gent-hover-preview').getAttribute('data-character'),hovered.character,'Hover portrait must match the actual full-body sprite');
  const hoveredPosition=await page.locator('#gent-hover-preview').getAttribute('data-position');
  await page.mouse.click(hovered.x,hovered.y);
  await page.locator('#gent-drawer').waitFor({state:'visible'});
  assert.equal(await page.locator('.gent-portrait').getAttribute('data-position'),hoveredPosition);
  assert.equal(await page.locator('.gent-portrait').getAttribute('data-character'),hovered.character);
  await page.locator('#close-gent').click();
  await page.locator('[data-floor="4"]').click();
  await page.locator('#preview-walking').click();
  await page.waitForFunction(()=>document.querySelector('#office').gentLayout.length>0&&document.querySelector('#office').gentLayout.every(s=>s.preview&&s.floor===4));
  assert.equal(await page.locator('#walking-preview-warning').isVisible(),true);
  assert.equal(await page.locator('#company-signal').getAttribute('data-state'),'red');
  await page.mouse.move(0,0);
  const beforeX=await page.locator('#office').evaluate(c=>c.gentLayout[0].hit.x);
  await page.waitForFunction(x=>Math.abs(document.querySelector('#office').gentLayout[0].hit.x-x)>4,beforeX);
  await page.locator('#office').screenshot({path:'/tmp/gents-floor-walking-preview.png'});
  const roomStyles=await page.locator('#office').evaluate(c=>c.interiorDesign.rooms.map(r=>r.id));
  for(const floor of ['6','5','3']){
    await page.locator(`[data-floor="${floor}"]`).click();await page.locator('#preview-walking').click();
    await page.waitForFunction(f=>document.querySelector('#office').interiorDesign?.floor===Number(f),floor);
    roomStyles.push(...await page.locator('#office').evaluate(c=>c.interiorDesign.rooms.map(r=>r.id)));
    const visible=await page.locator('#office').evaluate(c=>c.gentLayout);
    for(let i=0;i<visible.length;i++)for(let j=i+1;j<visible.length;j++){const a=visible[i].hit,b=visible[j].hit;assert.ok(!(a.x<b.x+b.w&&a.x+a.w>b.x&&a.y<b.y+b.h&&a.y+a.h>b.y),'Projected Gents may not overlap');}
    await page.locator('#office').screenshot({path:`/tmp/gents-interior-${floor}.png`});
  }
  assert.equal(new Set(roomStyles).size,8,'Every work room must have a different design');
  await page.locator('[data-floor="all"]').click();
  await page.evaluate(()=>window.scrollTo(0,0));

  const companyOps = [];
  let submittedRun;
  await page.route('**/api/companies/we-need-gents-lc', async route => {
    if (route.request().method() !== 'POST') return route.fallback();
    const body = route.request().postDataJSON();
    if (!['intake', 'approve'].includes(body.op)) return route.fallback();
    companyOps.push(body);
    await route.fulfill({
      contentType: 'application/json',
      body: JSON.stringify({ name: 'We Need Gents LLC', jobs: [{ run_id: body.run_id, digest: 'a'.repeat(64) }] }),
    });
  });
  await page.route('**/api/runs', async route => {
    if (route.request().method() !== 'POST') return route.fallback();
    submittedRun = route.request().postDataJSON();
    await route.fulfill({ contentType: 'application/json', body: JSON.stringify({ id: submittedRun.run_id }) });
  });
  let needsAnswer=false,answered=false;
  await page.route('**/api/runs/*', route => route.fulfill({
    contentType: 'application/json',
    body: JSON.stringify({ company_id: 'we-need-gents-lc', status: answered?'completed':'running', events: answered?[]:[{seq:0,timestamp_ms:Date.now(),call_id:'character-activity-fixture',agent:'Engineer Gent',role:'worker',room:'workshop',kind:'call_started',model:'local / Qwen3-Coder-30B-A3B-Instruct',message:'SIMULATED activity fixture'}] }),
  }));
  await page.route('**/api/runs/*/attention',async route=>{
    if(route.request().method()==='POST'){
      assert.equal(route.request().postDataJSON().answer,'Use the dedicated mailbox');answered=true;
      return route.fulfill({contentType:'application/json',body:JSON.stringify({answered:true})});
    }
    await route.fulfill({contentType:'application/json',body:JSON.stringify({attention:needsAnswer&&!answered?{id:'fixture-question',question:'Which mailbox should the company use?'}:null})});
  });
  await page.locator('#task').fill('Prepare a concise local-only project brief.');
  await page.locator('#consent').check();
  await page.getByRole('button', { name: 'Ask For a Task', exact: true }).click();
  await page.waitForFunction(()=>document.querySelector('#company-signal').dataset.state==='green');
  await page.locator('[data-floor="4"]').click();
  await page.waitForFunction(()=>window.__gentLabels.has('Working on the'));
  const workingLayout=await page.locator('#office').evaluate(c=>c.gentLayout);
  assert.equal(workingLayout.filter(s=>!s.resting).length,1);assert.equal(workingLayout.find(s=>!s.resting).floor,4);
  assert.equal(workingLayout[0].preview,false);assert.equal(await page.locator('#office').evaluate(c=>c.height),1000);
  await page.locator('#office').screenshot({path:'/tmp/gents-tower-working.png'});
  await page.locator('[data-floor="all"]').click();
  needsAnswer=true;
  await page.locator('#attention-panel').waitFor({state:'visible'});
  assert.equal(await page.locator('#company-signal').getAttribute('data-state'),'yellow');
  await page.locator('#attention-answer').fill('Use the dedicated mailbox');
  await page.getByRole('button',{name:'Answer & continue →'}).click();
  await page.getByText(/Run completed/).waitFor();
  assert.equal(await page.locator('#company-signal').getAttribute('data-state'),'red');
  assert.deepEqual(companyOps.map(operation => operation.op), ['intake', 'approve']);
  assert.equal(companyOps[0].run_id, companyOps[1].run_id);
  assert.equal(companyOps[1].digest, 'a'.repeat(64));
  assert.equal(submittedRun.company_id, 'we-need-gents-lc');
  assert.equal(submittedRun.run_id, companyOps[0].run_id);
  assert.equal(submittedRun.task, 'Prepare a concise local-only project brief.');
  assert.equal(submittedRun.consent, false);
  assert.deepEqual(errors, []);
  await page.setViewportSize({width:390,height:844});
  assert.equal(await page.evaluate(()=>document.documentElement.scrollWidth<=window.innerWidth),true);
  await page.screenshot({path:'data/office-mobile.png',fullPage:false});
  console.log('PASS: position-specific characters, independent wardrobe choices, sprite/hover/dossier identity, browser intake/approval/start, red/green/yellow states, answer/continue and responsive office');
} finally {
  await browser.close();
}
