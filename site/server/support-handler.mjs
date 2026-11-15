import {createSupportService,resendDelivery} from './support-service.mjs';
import {createRequestGuard,createDeliveryGuard,supportPreflight,clientIdentity,readLimitedBody} from './request-guard.mjs';
const responseHeaders={'Cache-Control':'no-store','X-Content-Type-Options':'nosniff','Referrer-Policy':'no-referrer','Allow':'POST'};
export function createSupportHandler({env,admission=createRequestGuard(),claimDelivery=createDeliveryGuard(),deliver=resendDelivery({apiKey:env.RESEND_API_KEY})}) {return {
 async fetch(request) {
  const reply=(status,body)=>Response.json(body,{status,headers:{...responseHeaders,...(status===429?{'Retry-After':'60'}:{})}});
  const headers=Object.fromEntries(request.headers),rejected=supportPreflight({method:request.method,headers},env);
  if(rejected)return reply(rejected,{message:'Use the support form on this website.'});
  const release=admission.acquire(clientIdentity(headers['x-vercel-forwarded-for']));
  if(!release)return reply(429,{message:'Support is busy. Wait a minute or use the contact email.'});
  try {
   const body=await readLimitedBody(request);
   const service=createSupportService({env,deliver,claimDelivery});
   const result=await service({method:request.method,headers,body});
   return reply(result.status,result.body);
  }catch(error){return reply([408,413].includes(error.status)?error.status:400,{message:'The request could not be processed. Keep your message and use the contact email if needed.'});}
  finally{release();}
 }
};}
export default createSupportHandler({env:process.env});
