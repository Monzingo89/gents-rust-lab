const form=document.querySelector('#support-form');
if(form){
 const status=document.querySelector('#support-status'),button=form.querySelector('button');let challenge=null;
 async function call(body){const reply=await fetch('/api/support',{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify(body),signal:AbortSignal.timeout(12000)});if(!reply.headers.get('content-type')?.includes('application/json'))throw Error('The form is unavailable here. Please use the contact email.');const data=await reply.json();if(!reply.ok)throw Error(data.message||'Support is unavailable.');return data;}
 form.addEventListener('submit',async event=>{
  event.preventDefault();if(button.disabled)return;button.disabled=true;
  try{
   status.textContent='Verifying this request…';
   if(!challenge||Date.now()>challenge.expires){const value=await call({action:'challenge'});challenge={token:value.token,ready:Date.now()+value.wait_ms+100,expires:Date.now()+value.expires_in_ms};}
   await new Promise(resolve=>setTimeout(resolve,Math.max(0,challenge.ready-Date.now())));
   const fields=Object.fromEntries(new FormData(form));
   status.textContent='Sending your support message…';
   const result=await call({action:'send',token:challenge.token,name:fields.name,email:fields.email,topic:fields.topic,message:fields.message,website:fields.website,consent:fields.consent==='yes'});
   status.textContent=result.message;form.reset();challenge=null;
  }catch(error){status.textContent=`${error.name==='TimeoutError'?'Delivery is uncertain. Check your email or use the contact address before retrying.':error.message} Your message stays in this page; it is not automatically resent.`;}
  finally{button.disabled=false;}
 });
}
