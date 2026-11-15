import {createHash} from 'node:crypto';
import {isIP} from 'node:net';

// A bounded, warm-process admission guard, deliberately independent of delivery.
// It is NOT a distributed quota: cold starts/regions have separate counters.
export function createRequestGuard({now=()=>performance.now(),windowMs=60000,perClient=8,total=60,concurrency=2,maxClients=512}={}) {
 for(const n of [windowMs,perClient,total,concurrency,maxClients])if(!Number.isSafeInteger(n)||n<1)throw Error('Invalid admission limit');
 const clients=new Map();let started=now(),count=0,active=0;
 return {
  acquire(identity){
   if(typeof identity!=='string'||!identity||identity.length>128)return null;
   const time=now();
   if(time-started>=windowMs){started=time;count=0;clients.clear();}
   if(count>=total||active>=concurrency||(clients.get(identity)||0)>=perClient||(!clients.has(identity)&&clients.size>=maxClients))return null;
   clients.set(identity,(clients.get(identity)||0)+1);count++;active++;
   let released=false;return ()=>{if(!released){released=true;active--;}};
  }
 };
}

// Validate cheap metadata before reading/allocating any request body.
export function supportPreflight({method,headers},env) {
 if(method!=='POST')return 405;
 const ip=headers['x-vercel-forwarded-for'];
 if(env.VERCEL!=='1'||typeof ip!=='string'||!isIP(ip))return 403;
 const allowed=(env.SUPPORT_ORIGIN||'').split(',').map(v=>v.trim());
 if(env.VERCEL_URL)allowed.push(`https://${env.VERCEL_URL}`);
 if(typeof headers.origin!=='string'||!headers.origin.startsWith('https://')||!allowed.includes(headers.origin))return 403;
 if(!/^application\/json(?:;|$)/i.test(headers['content-type']||''))return 415;
 if(headers['content-encoding']&&headers['content-encoding']!=='identity')return 415;
 const length=headers['content-length'];
 if(length!==undefined&&(!/^\d{1,10}$/.test(length)||Number(length)>8192))return 413;
 return 0;
}

export const clientIdentity=ip=>createHash('sha256').update(ip).digest('hex');

export async function readLimitedBody(request,{limit=8192,timeoutMs=2000}={}) {
 const reader=request.body?.getReader();if(!reader)return '';
 let timer,size=0;const chunks=[];
 const deadline=new Promise((_,reject)=>{timer=setTimeout(()=>{
  const error=new Error('Body deadline');error.status=408;reject(error);
  void reader.cancel().catch(()=>{});
 },timeoutMs);});
 try {
  for(;;){const {done,value}=await Promise.race([reader.read(),deadline]);if(done)break;
   size+=value.byteLength;if(size>limit){void reader.cancel().catch(()=>{});const error=new Error('Body limit');error.status=413;throw error;}
   chunks.push(Buffer.from(value));
  }
  return Buffer.concat(chunks).toString('utf8');
 }finally{clearTimeout(timer);}
}

// One delivery attempt per key per process for 24h, including uncertain results.
// Do not evict active entries to make room for attacker-controlled identities.
export function createDeliveryGuard({now=Date.now,maxEntries=256,lifetimeMs=86400000}={}) {
 const attempts=new Map();
 return key=>{
  const time=now();for(const [id,expires]of attempts)if(expires<=time)attempts.delete(id);
  if(attempts.has(key)||attempts.size>=maxEntries)return false;
  attempts.set(key,time+lifetimeMs);return true;
 };
}
