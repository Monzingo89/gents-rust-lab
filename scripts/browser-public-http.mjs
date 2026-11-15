// A pinned anonymous HTTP transport; it does not launch or control a browser.
import https from 'node:https';
import { publicAddresses, publicUrl } from './public-network.mjs';

export async function fetchPublic(url,method,headers,body,adapters={}){
 const target=publicUrl(url);
 const host=target.hostname.replace(/^\[|\]$/g,'');
 const addresses=await publicAddresses(host,adapters.lookup);
 const safeHeaders=Object.fromEntries(Object.entries(headers).filter(([key])=>!['host','cookie','authorization','proxy-authorization'].includes(key.toLowerCase())));
 return await new Promise((resolve,reject)=>{
  const request=adapters.request||https.request;
  const req=request(target,{method,headers:safeHeaders,timeout:30000,
   lookup:(_hostname,options,callback)=>options.all?callback(null,[addresses[0]]):callback(null,addresses[0].address,addresses[0].family)},res=>{
    const chunks=[];let size=0;
    res.on('data',chunk=>{size+=chunk.length;if(size>8_000_000)req.destroy(new Error('Browser response too large'));else chunks.push(chunk);});
    res.on('end',()=>{
     const responseHeaders={...res.headers};
     delete responseHeaders['content-encoding'];delete responseHeaders['transfer-encoding'];delete responseHeaders['content-length'];
     resolve({status:res.statusCode,headers:responseHeaders,body:Buffer.concat(chunks)});
    });
    res.on('error',reject);
   });
  req.on('timeout',()=>req.destroy(new Error('Browser request timed out')));req.on('error',reject);
  if(body)req.write(body);req.end();
 });
}
