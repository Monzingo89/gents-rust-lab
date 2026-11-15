// Deploy only the whitelisted introduction and one independent support function.
import {readFile,writeFile,mkdir,rm,copyFile,readdir,lstat} from 'node:fs/promises';
import {resolve,relative} from 'node:path';import {createHash} from 'node:crypto';import {parseEnv} from 'node:util';
import {pages} from '../site/content.mjs';
const root=resolve(import.meta.dirname,'..'),out=resolve(root,'site/deploy'),publicRoot=resolve(root,'site/dist');
const info=JSON.parse(await readFile(resolve(publicRoot,'build-info.json'),'utf8'));
if(!['production','vercel-evaluation-preview'].includes(info.mode))throw Error('Build the hosted evaluation preview first: node scripts/build-introduction.mjs --vercel-preview');
if(info.mode==='production'&&info.source_dirty)throw Error('Production packaging requires a clean committed source tree.');
const publicFiles=[...info.pages,'styles.css','app.mjs','calculator.mjs','support.mjs','onboarding.mjs','computer-readiness.js','basic-answers.js','favicon.ico','icon.svg','assets/office.png','assets/office.webp','assets/local-owl.png','LICENSE.txt','robots.txt','sitemap.xml','.well-known/security.txt','build-info.json'];
await rm(out,{recursive:true,force:true});await mkdir(resolve(out,'api'),{recursive:true});await mkdir(resolve(out,'assets'),{recursive:true});
for(const file of publicFiles){const source=resolve(publicRoot,file);if(!(await lstat(source)).isFile())throw Error('A public source is not a regular file.');await mkdir(resolve(out,file,'..'),{recursive:true});await copyFile(source,resolve(out,file));}
const policy=await readFile(resolve(root,'site/server/support-service.mjs'),'utf8');
const guard=await readFile(resolve(root,'site/server/request-guard.mjs'),'utf8');
const handler=(await readFile(resolve(root,'site/server/support-handler.mjs'),'utf8')).replace(/^import .* from '\.\/[^']+';\n/gm,'');
// Bundle isolated modules; keep Node import bindings distinct.
await writeFile(resolve(out,'api/support.js'),policy+'\n'+guard.replace("import {isIP} from 'node:net';\n",'')+'\n'+handler);
await writeFile(resolve(out,'package.json'),JSON.stringify({name:'gents-office-introduction',version:info.version,private:true,type:'module'},null,2)+'\n');
await writeFile(resolve(out,'vercel.json'),JSON.stringify({version:2,cleanUrls:true,trailingSlash:false,functions:{'api/support.js':{maxDuration:10}},headers:[{source:'/(.*)',headers:[{key:'Strict-Transport-Security',value:'max-age=31536000'},{key:'X-Content-Type-Options',value:'nosniff'},{key:'X-Frame-Options',value:'DENY'},{key:'Content-Security-Policy',value:"frame-ancestors 'none'"},{key:'Referrer-Policy',value:'no-referrer'},{key:'Permissions-Policy',value:'camera=(), microphone=(), geolocation=()'},{key:'X-Robots-Tag',value:info.indexable?'index, follow':'noindex, nofollow'}]}]},null,2)+'\n');
const allowed=new Set([...publicFiles,'api/support.js','package.json','vercel.json']);
async function walk(dir){let files=[];for(const entry of await readdir(dir,{withFileTypes:true})){const path=resolve(dir,entry.name);if(entry.isDirectory())files.push(...await walk(path));else{if(!entry.isFile())throw Error('Staged non-regular file.');files.push(path);}}return files;}
const secrets=[];for(const name of ['.env.local','.env.company','.env.website.local']){try{const env=parseEnv(await readFile(resolve(root,name),'utf8'));for(const [key,value]of Object.entries(env))if(/(KEY|TOKEN|SECRET|PASSWORD)$/.test(key)&&value.length>=8)secrets.push(value);}catch(error){if(error.code!=='ENOENT')throw error;}}
const manifest=[];for(const path of await walk(out)){const file=relative(out,path);if(!allowed.delete(file))throw Error('Unlisted deployment file.');const data=await readFile(path);if(secrets.some(value=>data.includes(Buffer.from(value))))throw Error('Private credential found in deployment artifact; upload forbidden.');manifest.push({file,size:data.length,sha256:createHash('sha256').update(data).digest('hex'),sha:createHash('sha1').update(data).digest('hex')});}
if(allowed.size)throw Error('Deployment file missing.');manifest.sort((a,b)=>a.file.localeCompare(b.file));
const sha256=createHash('sha256').update(JSON.stringify(manifest)).digest('hex');
await mkdir(resolve(root,'data/mcp-basement-verification'),{recursive:true});await writeFile(resolve(root,'data/mcp-basement-verification/vercel-introduction-artifact.json'),JSON.stringify({status:'PASS',mode:info.mode,sha256,files:manifest,private_credential_scan:'no matching private credential values',company_runtime_included:false,uploaded:false},null,2)+'\n');
console.log(JSON.stringify({status:'PASS',output:out,files:manifest.length,sha256,uploaded:false,company_runtime_included:false}));
