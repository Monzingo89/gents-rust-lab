import{importComputerReport,computerMessage}from'./computer-readiness.js';
export async function browserComputer(navigator){
 const result={threads:Number.isSafeInteger(navigator.hardwareConcurrency)?navigator.hardwareConcurrency:null,approximateMemoryGb:typeof navigator.deviceMemory==='number'?navigator.deviceMemory:null,gpu:null};
 try{const adapter=await Promise.race([navigator.gpu?.requestAdapter(),new Promise(resolve=>setTimeout(()=>resolve(null),2000))]);const info=adapter?.info;result.gpu=[info?.vendor,info?.architecture,info?.description].filter(v=>typeof v==='string'&&v.length<128).join(' ')||null;}catch{}
 return result;
}
const root=document.querySelector('#first-task');
if(root){
 let step=0,report=null;const panels=[...root.querySelectorAll('[data-step]')],steps=[...root.querySelectorAll('[data-step-label]')],status=root.querySelector('[data-wizard-status]');
 function show(next){step=next;panels.forEach((p,i)=>p.hidden=i!==step);steps.forEach((p,i)=>{if(i===step)p.setAttribute('aria-current','step');else p.removeAttribute('aria-current');});panels[step].querySelector('h3').focus();status.textContent=`Step ${step+1} of 4`;if(step===3)review();if(step===2)field('check-browser').click();}
 function field(id){return root.querySelector('#'+id);}
 function review(){const message=report?computerMessage(report):null;field('brief-summary').textContent=`Your goal: ${field('new-goal').value.trim()}\nDeliverable: ${field('new-output').value.trim()}\nDone when: ${field('new-done').value.trim()}`;field('readiness-summary').textContent=message?message.title+' '+message.detail:'Computer check pending. Open the local app for GPU and memory detection. This website cannot confirm model fit or completion time.';}
 root.querySelectorAll('[data-next]').forEach(b=>b.onclick=()=>{const required=[...panels[step].querySelectorAll('[required]')];if(!required.every(el=>el.reportValidity()&&el.value.trim()))return;show(step+1);});
 root.querySelectorAll('[data-back]').forEach(b=>b.onclick=()=>show(step-1));
 field('check-browser').onclick=async()=>{const button=field('check-browser');button.disabled=true;field('computer-result').textContent='Checking what this browser can report…';try{const value=await browserComputer(navigator);field('computer-result').textContent=[value.gpu?`Browser GPU: ${value.gpu}.`:'GPU details are hidden or unsupported in this browser.',value.threads?`${value.threads} browser-reported CPU threads.`:'CPU thread count unknown.',value.approximateMemoryGb?`Browser memory hint: approximately ${value.approximateMemoryGb} GB; this is capped for privacy and is not available GPU memory.`:'Actual available memory is unknown to this website.','Use the local app for the full computer check. No speed or finish time has been assumed.'].join(' ');}finally{button.disabled=false;}};
 field('computer-report').onchange=async event=>{report=null;try{const file=event.target.files[0];if(!file)return;if(file.size>16384)throw Error('Choose a report smaller than 16 KB.');report=importComputerReport(await file.text());const text=computerMessage(report);field('computer-result').textContent=text.title+' '+text.detail+' Imported file information is advisory and is not independently verified.';}catch(e){field('computer-result').textContent=e.message;}};
 field('save-brief').onclick=()=>{const brief={type:'metivolve-task-brief',version:1,goal:field('new-goal').value.trim(),output:field('new-output').value.trim(),definition_of_done:field('new-done').value.trim(),scope:'User request only. No tool/memory grants, execution approval or task acceptance.'};const url=URL.createObjectURL(new Blob([JSON.stringify(brief,null,2)],{type:'application/json'})),a=document.createElement('a');a.href=url;a.download='metivolve-task-brief.json';a.click();setTimeout(()=>URL.revokeObjectURL(url),1000);};
 root.querySelectorAll('button').forEach(button=>button.disabled=false);root.dataset.ready='true';

}
