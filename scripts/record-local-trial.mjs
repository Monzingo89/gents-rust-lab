// Record actual local UI execution. No mocked requests, model/task dispatch or
// state changes: only select the owned trial company and observe its run.
import {chromium} from '@playwright/test';
import {readFile,writeFile,mkdir} from 'node:fs/promises';
import {resolve,join} from 'node:path';
const folder=resolve(process.argv[2]),freeze=JSON.parse(await readFile(join(folder,'freeze.json'),'utf8'));
if(freeze.base!=='http://127.0.0.1:8788'||!freeze.company.startsWith('architecture-native-'))throw Error('Expected an explicitly owned local architecture trial');
const frames=join(folder,'frames');await mkdir(frames,{recursive:true});
const browser=await chromium.launch({executablePath:chromium.executablePath(),headless:true});
const timeline=[],errors=[];
try {
 const page=await browser.newPage({viewport:{width:1440,height:1050}});page.on('pageerror',e=>errors.push(e.message));
 await page.goto(freeze.base+'/?autoconnect=1',{waitUntil:'networkidle'});
 await page.locator('#company-id').fill(freeze.company);
 await page.locator('#watch-company').click();
 await page.locator('#office').scrollIntoViewIfNeeded();
 await page.evaluate(()=>{const label=document.createElement('div');label.id='trial-recording-label';label.style.cssText='position:fixed;top:0;left:0;right:0;z-index:9999;padding:12px 20px;background:#08232a;color:#d4fff3;font:600 17px system-ui;box-shadow:0 2px 8px #0008';document.body.append(label);});
 const deadline=Date.now()+20*60*1000;let last;
 while(Date.now()<deadline){
  const response=await page.request.get(freeze.base+'/api/runs/'+freeze.run);if(!response.ok())throw Error('Original run observation failed; do not replay');
  last=await response.json();
  const calls=last.events.filter(e=>e.kind==='call_started');const current=calls.at(-1);
  const phase=last.status==='completed'?'Completed · independent local review accepted':last.status!=='running'?last.status:current?.role==='reasoning_acceptance'?'Independent local review':'Local agent writing';
  await page.locator('#trial-recording-label').evaluate((el,data)=>el.textContent=`Actual local trial · ${data.phase} · $0 provider fees · time-compressed recording`,{phase});
  if(timeline.length<150||last.status!=='running'){
   const name=`frame-${String(timeline.length).padStart(4,'0')}.png`;
   await page.screenshot({path:join(frames,name)});timeline.push({frame:name,observed_at_ms:Date.now(),status:last.status,phase,event_count:last.events.length});
  }
  if(last.status!=='running')break;
  await page.waitForTimeout(timeline.length<150?2000:10000);
 }
 await writeFile(join(folder,'recording.json'),JSON.stringify({run:freeze.run,terminal_status:last?.status,frames:timeline,errors,scope:'Real browser observation of the original local run only. No replay or mocked API/model events. Frames will be time-compressed when exported.'},null,2)+'\n');
 if(last?.status==='running')throw Error('Observation expired; original run may continue');
 if(errors.length)throw Error('Browser errors: '+errors.join('; '));
 console.log(JSON.stringify({run:freeze.run,status:last?.status,frames:timeline.length,folder}));
}finally{await browser.close();}
