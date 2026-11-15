// Actual office UI with explicitly simulated planning/review/prepare transport.
// Never approves or starts a real task, model or tool.
import assert from 'node:assert/strict';
import {chromium} from '@playwright/test';
import {writeFile} from 'node:fs/promises';
const browser=await chromium.launch({executablePath:chromium.executablePath(),headless:true});
try {
 const page=await browser.newPage({viewport:{width:1400,height:1000}}),errors=[],posts=[];
 page.on('pageerror',e=>errors.push(e.message));
 const session=crypto.randomUUID(),assessment=crypto.randomUUID(),actor='PM Gent';
 const proposal={status:'ready',question:'',definition_of_done:['Preserve the two supplied facts'],assignments:[{actor,task:'Summarize the synthetic note',tools:[],acceptance:['Preserve facts']}]};
 const planning={id:session,revision:2,status:'ready',planning_mode:'reasoning',proposal,messages:[{role:'user',message:'SIMULATED synthetic note:\n    rehearsal Tuesday; checklist under review.'}]};
 const review={id:assessment,status:'ready_for_owner_review',request:{speed:'patient',maximum_elapsed_seconds:14*86400},recommendation:{architecture:'ordered',agent_count:1,rationale:'SIMULATED minimal work'},review:{verdict:'accept',architecture:'ordered',agent_count:1,rationale:'SIMULATED review'},media_routing:{modalities:['text']}};
 const grant={tools:[],skills:[],memory_from:[]};
 const draft={task:planning.messages[0].message,criteria:proposal.definition_of_done,planning_mode:'reasoning',intake:{idea:planning.messages[0].message,acceptance:proposal.definition_of_done.join('\n'),ceo_plan:proposal,artifact_bindings:[],reasoning_acceptance:{reviewer:{id:'SIMULATED independent reviewer'},candidate_from:[actor]}},policy:{revision:1,master:actor,ceilings:{[actor]:grant},grants:{[actor]:grant}}};
 await page.route('**/api/companies/**',async route=>{
  const request=route.request(),url=new URL(request.url()),body=request.method()==='POST'?request.postDataJSON():null;
  const reply=value=>route.fulfill({json:value});
  if(request.method()==='GET' && url.pathname.endsWith('/planning'))return reply({sessions:[{id:session,status:'ready'}]});
  if(request.method()==='GET' && url.pathname.endsWith('/planning/'+session))return reply(planning);
  if(request.method()==='POST' && url.pathname.includes('/architecture/')){posts.push('architecture');return reply(review);}
  if(request.method()==='POST' && url.pathname.endsWith('/prepare-guided')){
   posts.push('prepare');assert.deepEqual(body,{expected_revision:2,architecture_assessment:assessment,data_classification:'synthetic',artifact_bindings:[]});
   return reply({draft,blueprint:{architecture:{id:'flow'}},candidate_only:false,execution_authorized:false});
  }
  if(request.method()==='POST' && body?.op==='intake'){
   posts.push('intake');assert.equal(body.intake.idea,draft.task);
   return reply({jobs:[{run_id:body.run_id,digest:'a'.repeat(64),state:'draft',intake:body.intake}]});
  }
  if(request.method()==='POST' && body?.op!=='status'){
   posts.push('UNEXPECTED_MUTATION');return route.abort();
  }
  return route.continue();
 });
 await page.route('**/api/runs',route=>{
  if(route.request().method()==='POST'){posts.push('UNEXPECTED_RUN');return route.abort();}
  return route.continue();
 });
 await page.goto('http://127.0.0.1:8788/?autoconnect=1',{waitUntil:'networkidle'});
 await page.locator('[data-floor="10"]').click();
 await page.locator('#ceo-plan-sessions').selectOption(session);
 await page.locator('#ceo-execution').waitFor({state:'visible'});
 await page.locator('#ceo-prepare').click();
 await page.getByText('Execution review unavailable: Ask the CEO and local judge to review this plan first.',{exact:true}).waitFor();
 assert.deepEqual(posts,[]);
 await page.getByRole('button',{name:'Ask CEO and local judge',exact:true}).click();
 await page.getByText('SIMULATED review',{exact:false}).waitFor();
 await page.locator('#ceo-prepare').click();
 await page.getByText('Execution review unavailable: Choose whether the task uses test, public, private or regulated information.',{exact:true}).waitFor();
 assert.deepEqual(posts,['architecture']);
 await page.locator('#ceo-data-classification').selectOption('synthetic');
 await page.locator('#ceo-prepare').click();
 await page.locator('#ceo-execution-consent-label').waitFor({state:'visible'});
 assert.match(await page.locator('#ceo-execution-summary').innerText(),/factory: flow/);
 assert.match(await page.locator('#ceo-execution-summary').innerText(),/No tools or historical memory/);
 assert.match(await page.locator('#ceo-execution-summary').innerText(),/none \(reasoning only\)/);
 await page.locator('#ceo-execution-consent').check();
 assert.equal(await page.locator('#ceo-execute').isEnabled(),true);
 await page.locator('#ceo-data-classification').selectOption('public');
 assert.equal(await page.locator('#ceo-execute').isVisible(),false);
 assert.equal(await page.locator('#ceo-execution-consent').isChecked(),false);
 assert.deepEqual(posts,['architecture','prepare','intake']);assert.deepEqual(errors,[]);
 const proof={status:'PASS',posts,actual_model_calls:0,actual_task_writes:0,scope:'Actual office DOM; SIMULATED canonical planning/review/prepare/intake responses only. Missing review/data scope gates, exact request, frozen review rendering and edit invalidation verified. No real approval, task, inference or tool execution.'};
 await writeFile('data/mcp-basement-verification/guided-factory-browser-proof.json',JSON.stringify(proof,null,2)+'\n');
 await page.screenshot({path:'data/mcp-basement-verification/guided-factory-browser.png'});
 console.log(JSON.stringify(proof));
} finally { await browser.close(); }
