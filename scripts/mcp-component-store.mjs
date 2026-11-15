// Owned, non-executable component review artifacts. Never an activation registry.
import assert from 'node:assert/strict';
import {constants} from 'node:fs';
import {lstat,open,mkdir,readdir,chmod,rename,rm} from 'node:fs/promises';
import {createHash,randomUUID} from 'node:crypto';
import {fileURLToPath} from 'node:url';
import {canonical} from './fixtures/mcp-reconciliation.mjs';
const sha=bytes=>createHash('sha256').update(bytes).digest('hex');
const files=['acceptance.rs','qualification.json','source.rs'];
const store='data/owned-rust-components/';
const maxBytes=8*1024*1024;
const identity=s=>[s.dev,s.ino,s.size,s.mode,s.uid,s.nlink,s.mtimeMs,s.ctimeMs].join(':');
const directoryInfo=url=>lstat(fileURLToPath(url).replace(/\/$/,'')||'/');
async function checkRoot(root){
 assert(root instanceof URL&&root.protocol==='file:'&&root.pathname.endsWith('/')&&!root.search&&!root.hash,'Component root must be an explicit local directory URL');
 owned(await directoryInfo(root),true);
}
function owned(info,directory=false){
 assert(process.platform!=='win32'&&typeof process.getuid==='function'&&typeof constants.O_NOFOLLOW==='number','Component storage needs a POSIX ownership adapter; Windows ACL support is not implemented');
 assert(!info.isSymbolicLink()&&(directory?info.isDirectory():info.isFile())&&info.uid===process.getuid()&&!(info.mode&0o022),'Component paths must be owner-controlled ordinary paths');
 if(!directory)assert.equal(info.nlink,1,'Hard-linked component/evidence file rejected');
}
async function parents(root,relative){
 assert(typeof relative==='string'&&relative.length<=400&&relative.split('/').every(p=>/^[A-Za-z0-9._-]+$/.test(p)&&p!=='.'&&p!=='..'),'Invalid relative component/evidence path');
 await checkRoot(root);
 const parts=relative.split('/');let url=root;
 for(const part of parts.slice(0,-1)){url=new URL(part+'/',url);owned(await directoryInfo(url),true);}
 return new URL(parts.at(-1),url);
}
export async function readOwnedEvidence(root,relative,limit=maxBytes,sealed=false){
 assert(Number.isSafeInteger(limit)&&limit>0&&limit<=maxBytes);
 const url=await parents(root,relative),before=await lstat(url);owned(before);
 assert(before.size<=limit&&(!sealed||!(before.mode&0o222)),'Evidence size or sealing policy failed');
 const handle=await open(url,constants.O_RDONLY|constants.O_NOFOLLOW);
 try{
  assert.equal(identity(await handle.stat()),identity(before),'Evidence changed before read');
  const chunks=[],buffer=Buffer.alloc(Math.min(limit+1,65536));let size=0;
  for(;;){const {bytesRead}=await handle.read(buffer,0,buffer.length,null);if(!bytesRead)break;size+=bytesRead;assert(size<=limit,'Evidence grew beyond bound');chunks.push(Buffer.from(buffer.subarray(0,bytesRead)));}
  assert.equal(size,before.size,'Evidence size changed');
  assert.equal(identity(await handle.stat()),identity(before),'Evidence changed during read');
  assert.equal(identity(await lstat(url)),identity(before),'Evidence path changed during read');
  await parents(root,relative);
  return Buffer.concat(chunks);
 }finally{await handle.close();}
}
async function directory(root,relative){
 const url=new URL(relative,root);
 try{await mkdir(url,{mode:0o700});}catch(error){if(error.code!=='EEXIST')throw error;}
 owned(await directoryInfo(url),true);
}
export async function verifyComponentPackage(root,id){
 assert(/^[a-f0-9]{64}$/.test(id),'Invalid component package ID');
 const relative=store+id+'/',folder=new URL(relative,root);
 const info=await directoryInfo(folder);owned(info,true);assert(!(info.mode&0o222),'Published component folder is writable');
 assert.deepEqual((await readdir(folder)).sort(),[...files,'manifest.json'].sort(),'Unexpected component package entry');
 const manifest=JSON.parse(await readOwnedEvidence(root,relative+'manifest.json',65536,true));
 const {id:claimed,...body}=manifest;
 assert.deepEqual(Object.keys(body).sort(),['context','execution_authorized','files','kind','promotion_authorized','schema_version','status']);
 assert.equal(claimed,id);assert.equal(sha(canonical(body)),id,'Component manifest identity mismatch');
 assert(body.schema_version===1&&body.kind==='owned_rust_component_review'&&body.status==='review_ready_fixture_only'&&body.execution_authorized===false&&body.promotion_authorized===false,'Review package cannot grant execution or promotion');
 assert.deepEqual(Object.keys(body.files).sort(),files);
 for(const name of files){const expected=body.files[name];assert(/^[a-f0-9]{64}$/.test(expected.sha256)&&Number.isSafeInteger(expected.bytes)&&expected.bytes>=0&&expected.bytes<=maxBytes);const bytes=await readOwnedEvidence(root,relative+name,maxBytes,true);assert(bytes.length===expected.bytes&&sha(bytes)===expected.sha256,'Component file digest mismatch');}
 return {...manifest,manifest:relative+'manifest.json'};
}
export async function listComponentIds(root){
 await checkRoot(root);
 for(const path of ['data/','data/owned-rust-components/']){
  try{owned(await directoryInfo(new URL(path,root)),true);}catch(error){if(error.code==='ENOENT')return [];throw error;}
 }
 const entries=await readdir(new URL(store,root),{withFileTypes:true});
 assert(entries.length<=64,'Component inventory exceeds the bounded catalog');
 const ids=entries.map(entry=>entry.name).filter(name=>/^[a-f0-9]{64}$/.test(name)).sort();
 assert(ids.length<=32,'At most thirty-two review packages per catalog snapshot');
 return ids;
}
export async function publishComponentReview(root,payload,context){
 assert.deepEqual(Object.keys(payload).sort(),files,'Review package requires exactly source, frozen acceptance and qualification data');
 const bytes=Object.fromEntries(files.map(name=>{const value=payload[name];assert(typeof value==='string'||Buffer.isBuffer(value));const data=Buffer.from(value);assert(data.length<=maxBytes,'Component data exceeds bound');return [name,data];}));
 const body={schema_version:1,kind:'owned_rust_component_review',status:'review_ready_fixture_only',execution_authorized:false,promotion_authorized:false,context,files:Object.fromEntries(files.map(name=>[name,{bytes:bytes[name].length,sha256:sha(bytes[name])}]))};
 const id=sha(canonical(body)),manifest=Buffer.from(canonical({...body,id}));assert(manifest.length<=65536,'Component manifest exceeds bound');
 await checkRoot(root);await directory(root,'data/');await directory(root,store);
 const destination=new URL(store+id+'/',root);
 try{await lstat(destination);return await verifyComponentPackage(root,id);}catch(error){if(error.code!=='ENOENT')throw error;}
 const relative=store+'.draft-'+randomUUID()+'/',draft=new URL(relative,root);await mkdir(draft,{mode:0o700});let published=false;
 try{
  for(const [name,value] of [...Object.entries(bytes),['manifest.json',manifest]]){
   const output=await open(new URL(name,draft),constants.O_WRONLY|constants.O_CREAT|constants.O_EXCL|constants.O_NOFOLLOW,0o400);
   try{await output.writeFile(value);await output.sync();}finally{await output.close();}
  }
  await chmod(draft,0o500);
  try{await rename(draft,destination);published=true;}catch(error){if(!['EEXIST','ENOTEMPTY'].includes(error.code))throw error;}
  return await verifyComponentPackage(root,id);
 }finally{if(!published){await chmod(draft,0o700);await rm(draft,{recursive:true,force:true});}}
}
