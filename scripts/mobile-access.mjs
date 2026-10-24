import http from 'node:http';
import https from 'node:https';
import { randomBytes, timingSafeEqual, createHash, X509Certificate } from 'node:crypto';
import { mkdirSync, readFileSync, existsSync, chmodSync } from 'node:fs';
import { join } from 'node:path';
import { networkInterfaces, hostname } from 'node:os';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const escape=value=>String(value).replace(/[&<>"']/g,c=>({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[c]));
export function privateAddress(address=''){
 const ip=address.replace(/^::ffff:/,'');
 if(!/^\d+\.\d+\.\d+\.\d+$/.test(ip))return false;
 const p=ip.split('.').map(Number);if(p.some(n=>n>255))return false;
 return p[0]===10||p[0]===192&&p[1]===168||p[0]===172&&p[1]>=16&&p[1]<=31;
}
export function selectLanAddress(requested,interfaces=networkInterfaces()){
 const entries=Object.entries(interfaces).flatMap(([name,rows])=>(rows||[]).filter(r=>r.family==='IPv4'&&!r.internal&&privateAddress(r.address)&&!/^(utun|tun|tap|docker|bridge|tailscale|veth|vmnet)/i.test(name)).map(r=>({name,address:r.address})));
 if(requested){if(!entries.some(r=>r.address===requested))throw new Error('GENTS_MOBILE_HOST must be this laptop’s private Wi-Fi/Ethernet IPv4 address');return requested;}
 const chosen=entries.find(r=>r.name==='en0')||entries.find(r=>/^(en|wlan|wl|eth)/.test(r.name))||entries[0];
 if(!chosen)throw new Error('No private Wi-Fi/Ethernet address found. Connect the laptop to Wi-Fi first.');
 return chosen.address;
}
export function localCertificate(directory,address){
 mkdirSync(directory,{recursive:true,mode:0o700});chmodSync(directory,0o700);
 const keyFile=join(directory,'lan-key.pem'),certFile=join(directory,'lan-cert.pem');
 let valid=false;
 try{const cert=new X509Certificate(readFileSync(certFile));valid=!!cert.checkIP(address)&&Date.parse(cert.validTo)>Date.now()+86400_000&&existsSync(keyFile);}catch{}
 if(!valid){
  const result=spawnSync('openssl',['req','-config',fileURLToPath(new URL('../config/mobile-openssl.cnf',import.meta.url)),'-x509','-newkey','rsa:2048','-sha256','-nodes','-days','365','-subj','/CN=Gents Local Office','-addext',`subjectAltName=IP:${address}`,'-keyout',keyFile,'-out',certFile],{stdio:'ignore',timeout:30_000});
  if(result.error||result.status!==0)throw new Error('Could not generate the local HTTPS certificate. OpenSSL with -addext support is required.');
 }
 chmodSync(keyFile,0o600);chmodSync(certFile,0o600);
 const cert=readFileSync(certFile);
 return {key:readFileSync(keyFile),cert,fingerprint:new X509Certificate(cert).fingerprint256};
}
const page=(title,body)=>`<!doctype html><html lang="en"><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>${title}</title><style>body{margin:0;background:#ece7d7;color:#3f5141;font:17px system-ui}main{max-width:540px;margin:8vh auto;padding:28px;background:#fff9e9;border:1px solid #cdbd98;border-radius:22px;box-shadow:0 6px #d5c8aa}h1{font:700 32px Georgia}p{line-height:1.6}input,button{box-sizing:border-box;width:100%;padding:15px;font:inherit;border-radius:10px;border:1px solid #b8ae91;margin:8px 0}button{background:#6d8965;color:white;font-weight:bold;cursor:pointer}code{overflow-wrap:anywhere}small{line-height:1.6}#error{color:#a23d30}label{display:block;font-weight:bold}</style><main>${body}</main></html>`;
const loginPage=()=>page('Pair with the Gents laptop',`<small>PRIVATE WI-FI ACCESS</small><h1>Your laptop. Your Gents.</h1><p>Enter the pairing code shown on your laptop. Your phone controls the office; all models and work stay on the laptop.</p><form id="pair"><label for="code">Pairing code</label><input id="code" autocomplete="off" autocapitalize="none" spellcheck="false" required maxlength="80"><button>Connect to my laptop</button><p id="error" role="alert"></p></form><script>document.querySelector('#pair').onsubmit=async e=>{e.preventDefault();const button=e.currentTarget.querySelector('button');button.disabled=true;try{const r=await fetch('/mobile/session',{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({code:document.querySelector('#code').value})});if(!r.ok)throw Error(await r.text());location.replace('/?autoconnect=1');}catch(e){document.querySelector('#error').textContent=e.message;}finally{button.disabled=false;}};</script>`);
function securityHeaders(res){
 res.setHeader('Cache-Control','no-store');res.setHeader('Referrer-Policy','no-referrer');
 res.setHeader('X-Content-Type-Options','nosniff');res.setHeader('X-Frame-Options','DENY');
 res.setHeader('Cross-Origin-Resource-Policy','same-origin');
 res.setHeader('Content-Security-Policy',"default-src 'self'; img-src 'self' data:; style-src 'self' 'unsafe-inline'; script-src 'self' 'unsafe-inline'; connect-src 'self'; frame-ancestors 'none'; base-uri 'none'; form-action 'self'");
}
async function body(req,limit=16384){let size=0,chunks=[];for await(const chunk of req){size+=chunk.length;if(size>limit){const error=new Error('Request too large');error.status=413;throw error;}chunks.push(chunk);}return Buffer.concat(chunks);}
const digest=value=>createHash('sha256').update(value).digest();
export function createLanHandler({origin,upstream='http://127.0.0.1:8788',code,computer=hostname(),now=Date.now}){
 const publicUrl=new URL(origin),target=new URL(upstream);
 if(!['127.0.0.1','localhost','[::1]'].includes(target.hostname)||target.protocol!=='http:')throw new Error('Mobile gateway must target the laptop loopback harness');
 if(publicUrl.protocol!=='https:')throw new Error('Mobile access requires HTTPS');
 const sessions=new Map(),attempts=new Map();let burst=[];
 return async function handler(req,res){
  securityHeaders(res);
  const send=(status,text,type='text/plain; charset=utf-8')=>{res.writeHead(status,{'Content-Type':type});res.end(text);};
  try{
   const peer=(req.socket.remoteAddress||'').replace(/^::ffff:/,'');
   if(!(privateAddress(peer)||peer==='127.0.0.1'||peer==='::1'))return send(403,'Private Wi-Fi clients only');
   if(req.headers.host!==publicUrl.host)return send(403,'Unexpected office address');
   const safe=['GET','HEAD'].includes(req.method);
   if(!safe&&(req.headers.origin!==origin||req.headers['sec-fetch-site']==='cross-site'))return send(403,'Open this office directly before making changes');
   if(!safe&&req.method!=='DELETE'&&!String(req.headers['content-type']||'').toLowerCase().startsWith('application/json'))return send(415,'JSON requests required');
   const path=new URL(req.url,origin);if(path.origin!==origin)return send(400,'Invalid request target');
   const token=String(req.headers.cookie||'').split(';').map(v=>v.trim()).find(v=>v.startsWith('__Host-gents_mobile='))?.slice('__Host-gents_mobile='.length);
   const session=sessions.get(token);const authenticated=session&&session>now();
   for(const [id,expiry]of sessions)if(expiry<=now())sessions.delete(id);
   if(path.pathname==='/mobile/session'&&req.method==='POST'){
    const cutoff=now()-600_000;for(const [ip,a]of attempts)if(a.at<cutoff)attempts.delete(ip);
    burst=burst.filter(at=>at>now()-60_000);
    const a=attempts.get(peer)||{count:0,at:now()};
    if(a.count>=5||burst.length>=30||attempts.size>=1024)return send(429,'Too many pairing attempts. Wait ten minutes and try again.');
    a.count++;attempts.set(peer,a);burst.push(now());
    const submitted=JSON.parse((await body(req,1024)).toString()).code;
    if(typeof submitted!=='string'||!timingSafeEqual(digest(submitted.trim()),digest(code)))return send(401,'Incorrect pairing code');
    attempts.delete(peer);
    if(sessions.size>=100)sessions.delete(sessions.keys().next().value);
    const id=randomBytes(32).toString('base64url');sessions.set(id,now()+7*86400_000);
    res.setHeader('Set-Cookie',`__Host-gents_mobile=${id}; Secure; HttpOnly; SameSite=Strict; Path=/; Max-Age=604800`);
    return send(200,JSON.stringify({paired:true}),'application/json');
   }
   if(!authenticated){
    if(safe&&['/','/index.html'].includes(path.pathname))return send(200,loginPage(),'text/html; charset=utf-8');
    return send(401,'Pair this phone with the laptop first');
   }
   if(path.pathname==='/mobile/session'&&req.method==='DELETE'){
    sessions.delete(token);res.setHeader('Set-Cookie','__Host-gents_mobile=; Secure; HttpOnly; SameSite=Strict; Path=/; Max-Age=0');return send(200,'{}','application/json');
   }
   if(path.pathname==='/mobile/status')return send(200,JSON.stringify({mode:'wifi',execution:'laptop',computer}),'application/json');
   if(!['GET','HEAD','POST','DELETE'].includes(req.method))return send(405,'Method not allowed');
   const allowed=path.pathname==='/health'||path.pathname.startsWith('/api/')||path.pathname.startsWith('/assets/')||['/','/index.html','/app.js','/gent-avatars.js','/characters.css','/style.css','/favicon.png'].includes(path.pathname);
   if(!allowed)return send(404,'Not found');
   const data=safe?undefined:await body(req);
   const headers={};for(const key of ['accept','content-type','idempotency-key'])if(req.headers[key])headers[key]=req.headers[key];
   // Never forward phone cookies, credentials, identity/Host or routing headers.
   const remote=http.request(new URL(path.pathname+path.search,target),{method:req.method,headers},reply=>{
    for(const key of ['content-type','content-length','content-encoding'])if(reply.headers[key])res.setHeader(key,reply.headers[key]);
    res.writeHead(reply.statusCode);reply.pipe(res);
   });
   remote.setTimeout(90_000,()=>remote.destroy(new Error('Laptop response timed out')));
   remote.on('error',()=>{if(!res.headersSent)send(502,'The laptop office is unavailable or restarting. Reconnect shortly.');else res.destroy();});
   res.on('close',()=>remote.destroy());remote.end(data);
  }catch(error){if(!res.headersSent)send(error.status||400,error.status===413?'Request too large':'Invalid request');else res.destroy();}
 };
}
const listen=(server,port,host)=>new Promise((resolve,reject)=>{server.once('error',reject);server.listen(port,host,()=>{server.removeListener('error',reject);resolve();});});
export async function startMobileAccess({office='http://127.0.0.1:8788',directory='data/mobile',host=selectLanAddress(process.env.GENTS_MOBILE_HOST),port=8789,setupPort=8790}={}){
 if(!privateAddress(host))throw new Error('Mobile gateway must bind a private Wi-Fi address, never 0.0.0.0 or a public address');
 const tls=localCertificate(directory,host),origin=`https://${host}:${port}`,code=randomBytes(12).toString('base64url');
 const gateway=https.createServer(tls,createLanHandler({origin,code,upstream:office}));
 gateway.requestTimeout=30_000;gateway.headersTimeout=15_000;gateway.maxHeadersCount=50;
 const setup=http.createServer((req,res)=>{
  securityHeaders(res);
  // Pairing code is available only in this loopback-only laptop view, never logs.
  if(req.headers.host!==`127.0.0.1:${setupPort}`||req.method!=='GET'||req.url!=='/'||(req.headers['sec-fetch-site']&&!['none','same-origin'].includes(req.headers['sec-fetch-site']))){res.writeHead(403);return res.end('Laptop setup view only');}
  res.setHeader('Content-Type','text/html; charset=utf-8');
  res.end(page('Gents mobile setup',`<small>LAPTOP SETUP VIEW · PRIVATE WI-FI</small><h1>Your Gents, on your phone.</h1><p>On your phone, join the same trusted Wi-Fi network, then open:</p><p><a href="${origin}/?autoconnect=1"><code>${origin}</code></a></p><p>Pairing code (case-sensitive):</p><p><code id="pair-code">${escape(code)}</code></p><button id="copy">Copy pairing code</button><script>document.querySelector('#copy').onclick=()=>navigator.clipboard.writeText(document.querySelector('#pair-code').textContent);</script><p>The first visit shows a certificate warning because this is your laptop’s self-signed local certificate. Verify this exact Wi-Fi address before continuing. Do not bypass certificate warnings on other sites.</p><small>Certificate SHA-256 fingerprint:<br><code>${escape(tls.fingerprint)}</code></small><p>All tasks, models, email and training run on this laptop. Closing your phone browser does not stop work. Keep the laptop awake, plugged in, lid open, and its company terminal running. Guest Wi-Fi isolation or the Mac firewall may block access.</p><p>Stopping mobile access revokes all phone sessions. Starting it again creates a new pairing code. No API key, Tailscale, router port-forwarding or public tunnel is used.</p>`));
 });
 try{await listen(gateway,port,host);await listen(setup,setupPort,'127.0.0.1');}
 catch(error){gateway.close();setup.close();throw new Error(`Mobile access could not start: ${error.message}`);}
 return {url:origin,setupUrl:`http://127.0.0.1:${setupPort}`,close:()=>Promise.all([gateway,setup].map(server=>new Promise(resolve=>{server.close(resolve);server.closeAllConnections();})))};
}
