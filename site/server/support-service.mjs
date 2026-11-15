import { createHmac, randomBytes, timingSafeEqual } from 'node:crypto';
import { isIP } from 'node:net';
const MAX_BODY=8192, MIN_WAIT=3000, LIFETIME=10*60*1000;
const topics={setup:'Local setup',feedback:'Feedback',privacy:'Privacy request',security:'Security concern'};
const email=value=>typeof value==='string'&&value.length<=254&&/^[^\s@<>\x00-\x1f\x7f]+@[^\s@<>\x00-\x1f\x7f]+\.[^\s@<>\x00-\x1f\x7f]+$/.test(value);
const mac=(secret,value)=>createHmac('sha256',secret).update(value).digest('base64url');
const same=(a,b)=>typeof a==='string'&&typeof b==='string'&&Buffer.byteLength(a)===Buffer.byteLength(b)&&timingSafeEqual(Buffer.from(a),Buffer.from(b));
const answer=(status,message)=>({status,body:{message}});

// Ports keep HTTP, validation/challenge policy and the delivery provider separate.
// No agent runtime, company credentials, database, SDK or user-content logging.
export function createSupportService({env,deliver,now=Date.now,nonce=()=>randomBytes(16).toString('hex'),claimDelivery=()=>true}) {
 return async ({method,headers,body})=>{
  if(method!=='POST')return answer(405,'Use the support form to send a message.');
  if(env.SUPPORT_ENABLED!=='1'||env.VERCEL!=='1'||!env.RESEND_API_KEY||!env.SUPPORT_SIGNING_SECRET||env.SUPPORT_SIGNING_SECRET.length<32||!email(env.SUPPORT_TO)||!env.SUPPORT_FROM)return answer(503,'The support form is unavailable. Please use the contact email on this page.');
  const ip=headers['x-vercel-forwarded-for'];
  if(typeof ip!=='string'||!isIP(ip))return answer(403,'The request could not be verified.');
  const origin=headers.origin;
  const allowed=[...(env.SUPPORT_ORIGIN||'').split(',').map(v=>v.trim()),env.VERCEL_URL&&`https://${env.VERCEL_URL}`].filter(Boolean);
  if(typeof origin!=='string'||!allowed.includes(origin)||!origin.startsWith('https://'))return answer(403,'Please send your message from this website.');
  if(!/^application\/json(?:;|$)/i.test(headers['content-type']||''))return answer(415,'Use the support form on this website.');
  let value;
  try {const raw=typeof body==='string'?body:JSON.stringify(body);if(Buffer.byteLength(raw)>MAX_BODY)return answer(413,'The message is too large.');value=JSON.parse(raw);if(!value||typeof value!=='object'||Array.isArray(value))throw Error();}
  catch{return answer(400,'Check the form fields and try again.');}
  const secret=env.SUPPORT_SIGNING_SECRET,ipHash=mac(secret,ip),current=now();
  if(value.action==='challenge') {
   if(Object.keys(value).length!==1)return answer(400,'Invalid support request.');
   const payload=Buffer.from(JSON.stringify({time:current,nonce:nonce(),ip:ipHash,origin})).toString('base64url');
   return {status:200,body:{token:`${payload}.${mac(secret,payload)}`,wait_ms:MIN_WAIT,expires_in_ms:LIFETIME}};
  }
  if(value.action!=='send'||Object.keys(value).some(k=>!['action','token','name','email','topic','message','website','consent'].includes(k)))return answer(400,'Invalid support request.');
  if(typeof value.website!=='string'||value.website!=='')return answer(400,'The request could not be verified.');
  if(value.consent!==true||!email(value.email)||typeof value.name!=='string'||value.name.length>80||/[\x00-\x1f\x7f]/.test(value.name)||!Object.hasOwn(topics,value.topic)||typeof value.message!=='string'||value.message.trim().length<20||value.message.length>3000||/[\x00-\x08\x0b-\x1f\x7f]/.test(value.message))return answer(400,'Use a valid email, topic and a message of 20–3,000 characters. Please consent to email processing.');
  if(typeof value.token!=='string'||value.token.length>1500)return answer(403,'Refresh the form verification and try again.');
  const [payload,signature,...extra]=value.token.split('.');
  if(extra.length||!same(signature,mac(secret,payload||'')))return answer(403,'Refresh the form verification and try again.');
  let claim;try{claim=JSON.parse(Buffer.from(payload,'base64url').toString());}catch{return answer(403,'Refresh the form verification and try again.');}
  const age=current-claim.time;
  if(!Number.isSafeInteger(claim.time)||age<MIN_WAIT||age>LIFETIME||claim.origin!==origin||!same(claim.ip,ipHash))return answer(403,'Refresh the form verification and try again.');
  // Resend retains idempotency keys for 24h, across function instances. One
  // distinct message per trusted IP per UTC day; no raw IP is sent to Resend.
  // This is an abuse ceiling, not bot-proofing or a global spend guarantee.
  const key=`gents-support-v1/${mac(secret,`${new Date(claim.time).toISOString().slice(0,10)}|${ipHash}`)}`;
  if(!claimDelivery(key))return answer(429,'A delivery was already attempted from this connection, or support is busy. Keep your message and use the contact email if needed.');
  const message={from:env.SUPPORT_FROM,to:[env.SUPPORT_TO],reply_to:value.email,subject:`Metivolve support: ${topics[value.topic]}`,text:`Name: ${value.name.trim()||'(not supplied)'}\nReply address: ${value.email}\nTopic: ${topics[value.topic]}\n\n${value.message.trim()}\n\nSubmitted through the Metivolve support form with processing consent.`};
  try {
   const result=await deliver(message,key);
   if(result===409||result===429)return answer(429,'A message from this connection has already been submitted today, or delivery is busy. Please use the contact email if needed.');
   if(result!==200&&result!==201)return answer(502,'The email service did not confirm acceptance. Keep your message and contact us by email.');
   return answer(202,'The email service accepted your message. Inbox delivery and a response are not guaranteed.');
  }catch{return answer(502,'Delivery is uncertain. Keep your message; it has not been automatically resent.');}
 };
}

export function resendDelivery({apiKey,fetcher=fetch}) {
 return async (message,key)=>{
  const response=await fetcher('https://api.resend.com/emails',{method:'POST',headers:{Authorization:`Bearer ${apiKey}`,'Content-Type':'application/json','Idempotency-Key':key},body:JSON.stringify(message),signal:AbortSignal.timeout(8000)});
  // Provider response bodies are deliberately not logged or sent to the browser.
  const status=response.status;await response.body?.cancel();return status;
 };
}
