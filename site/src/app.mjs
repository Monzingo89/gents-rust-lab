import { estimate, duration, ownership } from './calculator.mjs';
import {mountBasicAnswers} from './basic-answers.js';
for(const root of document.querySelectorAll('[data-basic-answers]'))mountBasicAnswers(root);
const form = document.querySelector('#scenario');
if (form) {
  form.addEventListener('submit', event => {
    event.preventDefault();
    const values = Object.fromEntries(new FormData(form));
    const result = estimate(values);
    const target = document.querySelector('#estimate');
    target.replaceChildren();
    const title = document.createElement('strong');
    title.textContent = result ? duration(result.seconds) : 'Add measured speeds to estimate time';
    const explanation = document.createElement('p');
    explanation.textContent = result ? 'Scenario only: input processing + generation + your setup allowance. Local provider/tool fees: $0. Hardware and electricity are separate. Retries, model swaps and extra work can take longer; completion is not guaranteed.' : 'Enter positive input-processing and generation speeds from a local benchmark. Device names alone cannot supply reliable timings.';
    target.append(title, explanation);
  });
}
const investment=document.querySelector('#ownership');
if(investment)investment.addEventListener('submit',event=>{
 event.preventDefault();const result=ownership(Object.fromEntries(new FormData(investment)));
 const target=document.querySelector('#ownership-result');target.replaceChildren();
 const title=document.createElement('strong'),detail=document.createElement('p');
 if(!result){title.textContent='Add your own costs and usage first.';detail.textContent='Use nonnegative amounts and no more than 744 compute hours in a month.';}
 else {title.textContent=result.breakEvenMonths===null?'No positive monthly saving in this scenario':`Illustrative payback: ${result.breakEvenMonths.toFixed(1)} months`;
 detail.textContent=`Estimated monthly electricity: $${result.monthlyElectricity.toFixed(2)}. Difference from your entered API spending: $${result.monthlySavings.toFixed(2)} / month. Local provider/tool fees: $0. This excludes maintenance, cooling, financing, hardware wear, upgrades and frontier development. Equal task quality and volume are assumptions, not measured facts.`;}
 target.append(title,detail);
});
