// Real office UI against the private HTTP fixture, without mocked page routes.
import assert from 'node:assert/strict';
import {mkdir} from 'node:fs/promises';
import {chromium} from '@playwright/test';

const base=process.env.GENTS_TASK_FIXTURE_URL;
const target=new URL(base);
assert.equal(target.protocol,'http:');assert.equal(target.hostname,'127.0.0.1');
assert.equal(target.username,'');assert.equal(target.password,'');
const browser=await chromium.launch({channel:'chrome',headless:true});
try {
 const page=await browser.newPage({viewport:{width:1440,height:1100}});
 const errors=[],posts=[];
 page.on('pageerror',e=>errors.push(e.message));
 page.on('request',r=>{if(r.method()==='POST')posts.push({url:r.url(),body:r.postDataJSON()});});
 await page.goto(base);
 await page.locator('.connection-settings > summary').click();
 await page.locator('#base').fill(base);
 await page.locator('#connect').click();
 await page.locator('#company-controls').waitFor({state:'visible'});
 await page.locator('#company-id').fill('one');
 await page.locator('#watch-company').click();
 const ask='Who was US president in 1976?';
 await page.locator('#task').fill(ask);
 await page.locator('#run').click();
 assert.equal(await page.locator('#ceo-plan-message').inputValue(),ask);
 const response=page.waitForResponse(r=>r.url().includes('/planning/')&&r.request().method()==='POST');
 await page.locator('#ceo-plan-send').click();
 const answer=await (await response).json();
 assert.equal(answer.status,'answered');
 assert.equal(answer.task_result.actual_path,'exact_answer_cache');
 assert.equal(answer.task_result.measurements.generative_model_calls,0);
 await page.locator('#ceo-plan-send:disabled').waitFor();
 assert.match(await page.locator('#ceo-plan-transcript').innerText(),/Gerald Ford/);
 assert.match(await page.locator('#ceo-plan-status').innerText(),/no model call/);
 assert.equal(await page.locator('#ceo-execution').isVisible(),false);
 await page.locator('#ceo-plan-transcript').scrollIntoViewIfNeeded();
 await mkdir(new URL('../data/task-architecture-verification/',import.meta.url),{recursive:true});
 await page.screenshot({path:new URL('../data/task-architecture-verification/ceo-fastpath.png',import.meta.url).pathname});

 // Extra criteria must survive the front-desk handoff and bypass this answer.
 await page.locator('#task').fill(ask);
 await page.locator('#dod').fill('Use two independent references');
 await page.locator('#run').click();
 const rejected=page.waitForResponse(r=>r.url().includes('/planning/')&&r.request().method()==='POST');
 await page.locator('#ceo-plan-send').click();
 assert.equal((await rejected).status(),400,'Fixture has no local CEO configured; it must fall through, never reuse the unconstrained answer');
 const request=posts.filter(r=>r.url.includes('/planning/')).at(-1).body;
 assert.equal(request.task_input.original_ask,ask);
 assert.deepEqual(request.task_input.proposed_acceptance,['Use two independent references']);
 assert.equal(await page.locator('#ceo-plan-transcript').innerText(),'');
 assert.equal(posts.filter(r=>new URL(r.url).pathname==='/api/runs').length,0);
 assert.deepEqual(errors,[]);
 console.log('PASS: actual office UI returns a saved exact-cache source answer with no execution controls/model call; Done means criteria reach typed intake and bypass that answer; no page errors or run dispatches.');
} finally {
 await browser.close();
}
