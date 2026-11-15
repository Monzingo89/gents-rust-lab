// Owner-controlled native executable snapshots. Not source/build attestation,
// permission grants, or isolation from a privileged process with this UID.
import {randomUUID,createHash} from 'node:crypto';
import {constants} from 'node:fs';
import {lstat,mkdir,open,chmod,rm,rename} from 'node:fs/promises';
import {fileURLToPath} from 'node:url';
import {spawnSync} from 'node:child_process';
export const nativeRoles=['gents-harness','gents-local-mcp','gents-sqlite-bridge','gents-rust-acceptance-check'];
const maxBinary=512*1024*1024;
const filename=role=>role+(process.platform==='win32'?'.exe':'');
const identity=s=>[s.dev,s.ino,s.size,s.mtimeMs,s.ctimeMs].join(':');
// POSIX lstat on a trailing-slash path may follow the final directory link.
const directoryInfo=url=>lstat(fileURLToPath(url).replace(/\/$/,'')||'/');
function owned(info,directory=false){
 if(info.isSymbolicLink()||!(directory?info.isDirectory():info.isFile())||(info.mode&0o022)!==0||info.uid!==process.getuid())throw Error('Runtime bundle requires owner-controlled ordinary paths');
}
async function directory(url,create=false){
 if(create){try{await mkdir(url,{mode:0o700});}catch(e){if(e.code!=='EEXIST')throw e;}}
 owned(await directoryInfo(url),true);
}
async function existing(folder,bytes,executables){
 const d=await directoryInfo(folder);owned(d,true);if(d.mode&0o222)throw Error('Published runtime bundle is writable');
 for(const [file,expected] of [['manifest.json',{bytes:bytes.length,sha256:createHash('sha256').update(bytes).digest('hex')}],...Object.values(executables).map(e=>[e.file,e])]){
  const url=new URL(file,folder),before=await lstat(url);owned(before);if(before.nlink!==1||before.size!==expected.bytes||(before.mode&0o222)!==0)throw Error('Cached runtime file metadata changed');
  const handle=await open(url,constants.O_RDONLY|constants.O_NOFOLLOW);
  try{if(identity(await handle.stat())!==identity(before))throw Error('Cached runtime file changed before read');const hash=createHash('sha256'),buffer=Buffer.alloc(65536);let size=0;for(;;){const {bytesRead}=await handle.read(buffer,0,buffer.length,null);if(!bytesRead)break;size+=bytesRead;if(size>expected.bytes)throw Error('Cached runtime file grew');hash.update(buffer.subarray(0,bytesRead));}if(size!==expected.bytes||hash.digest('hex')!==expected.sha256||identity(await handle.stat())!==identity(before)||identity(await lstat(url))!==identity(before))throw Error('Cached runtime file digest changed');}finally{await handle.close();}
 }
}
export async function createRuntimeBundle(root){
 if(process.platform==='win32')throw Error('Sealed native runtime bundles require an owner/ACL adapter on Windows; no unsealed fallback');
 await directory(root);await directory(new URL('target/',root));await directory(new URL('target/debug/',root));
 await directory(new URL('data/',root),true);await directory(new URL('data/mcp-runtime-bundles/',root),true);
 const temporary=randomUUID(),folder=new URL(`data/mcp-runtime-bundles/.draft-${temporary}/`,root);
 await mkdir(folder,{mode:0o700});
 const sourceStats=new Map(),executables={};let published=false;
 try{
  for(const role of nativeRoles){const source=new URL(`target/debug/${filename(role)}`,root),s=await lstat(source);owned(s);if(s.size<1||s.size>maxBinary)throw Error('Runtime executable outside bounded size');sourceStats.set(role,s);}
  for(const role of nativeRoles){
   const source=new URL(`target/debug/${filename(role)}`,root),destination=new URL(filename(role),folder);
   const input=await open(source,constants.O_RDONLY|constants.O_NOFOLLOW);let output;
   try{
    const before=await input.stat();if(identity(before)!==identity(sourceStats.get(role)))throw Error('Runtime source changed before snapshot');owned(before);
    output=await open(destination,constants.O_WRONLY|constants.O_CREAT|constants.O_EXCL|constants.O_NOFOLLOW,0o500);
    const hash=createHash('sha256'),buffer=Buffer.alloc(65536);let bytes=0;
    for(;;){const {bytesRead}=await input.read(buffer,0,buffer.length,null);if(!bytesRead)break;bytes+=bytesRead;if(bytes>maxBinary)throw Error('Runtime executable grew beyond size bound');const chunk=buffer.subarray(0,bytesRead);hash.update(chunk);let written=0;while(written<bytesRead){const result=await output.write(chunk,written,bytesRead-written,null);if(!result.bytesWritten)throw Error('Runtime snapshot write made no progress');written+=result.bytesWritten;}}
    if(bytes!==before.size||identity(await input.stat())!==identity(before)||identity(await lstat(source))!==identity(before))throw Error('Runtime source changed during snapshot');
    await output.sync();executables[role]={file:filename(role),bytes,sha256:hash.digest('hex')};
   }finally{await output?.close();await input.close();}
  }
  for(const role of nativeRoles)if(identity(await lstat(new URL(`target/debug/${filename(role)}`,root)))!==identity(sourceStats.get(role)))throw Error('Runtime source set changed before publishing snapshot');
  const id=createHash('sha256').update(JSON.stringify(executables)).digest('hex'),relative=`data/mcp-runtime-bundles/${id}/`,destination=new URL(relative,root);
  const manifest={schema_version:1,id,executables},bytes=Buffer.from(JSON.stringify(manifest));
  const out=await open(new URL('manifest.json',folder),constants.O_WRONLY|constants.O_CREAT|constants.O_EXCL|constants.O_NOFOLLOW,0o400);
  try{await out.writeFile(bytes);await out.sync();}finally{await out.close();}
  try{await lstat(destination);await existing(destination,bytes,executables);}catch(error){
   if(error.code!=='ENOENT')throw error;
   await chmod(folder,0o500);
   try{await rename(folder,destination);published=true;}catch(error){await chmod(folder,0o700);if(!['EEXIST','ENOTEMPTY'].includes(error.code))throw error;await existing(destination,bytes,executables);}
  }
  return {...manifest,manifest:relative+'manifest.json',manifest_sha256:createHash('sha256').update(bytes).digest('hex'),scope:'Owner-controlled read-only native executable copies; hashes checked before MCP child launch. Not source/build attestation, pinned models/compiler/shared libraries/Python helpers, or protection from a privileged same-user process.'};
 }finally{if(!published){try{await chmod(folder,0o700);}catch(e){if(e.code!=='ENOENT')throw e;}await rm(folder,{recursive:true,force:true});}}
}
function checkBundle(bundle){
 const hex=value=>typeof value==='string'&&/^[a-f0-9]{64}$/.test(value);
 if(bundle?.schema_version!==1||!hex(bundle.id)||bundle.manifest!==`data/mcp-runtime-bundles/${bundle.id}/manifest.json`||!hex(bundle.manifest_sha256)||Object.keys(bundle.executables??{}).length!==nativeRoles.length)throw Error('Invalid native bundle descriptor');
 for(const role of nativeRoles){const e=bundle.executables[role];if(e?.file!==filename(role)||!hex(e.sha256)||!Number.isSafeInteger(e.bytes)||e.bytes<1||e.bytes>maxBinary)throw Error('Invalid native bundle executable descriptor');}
}
export function bundleEnvironment(bundle){checkBundle(bundle);return {GENTS_RUNTIME_MANIFEST:bundle.manifest,GENTS_RUNTIME_MANIFEST_SHA256:bundle.manifest_sha256};}
export function bundleExecutable(root,bundle,role){checkBundle(bundle);if(!nativeRoles.includes(role))throw Error('Unknown native bundle role');return fileURLToPath(new URL(bundle.manifest.replace(/manifest\.json$/,'')+bundle.executables[role].file,root));}
export function verifyBundleBinding(catalog,bundle){
 checkBundle(bundle);
 const binding=catalog.runtime_binding,expected=bundle.manifest.replace(/manifest\.json$/,'')+bundle.executables['gents-local-mcp'].file;
 if(binding?.supported!==true||binding.mode!=='sealed_bundle'||binding.bundle_verified!==true||binding.manifest_sha256!==bundle.manifest_sha256||binding.mcp_executable!==expected)throw Error('Harness did not verify the selected native runtime bundle; rebuild the Rust workspace before model work');
 return binding;
}
export function probeBundle(root,bundle){
 const env=Object.fromEntries(['PATH','LANG','LC_ALL','SYSTEMROOT','WINDIR'].filter(k=>k in process.env).map(k=>[k,process.env[k]]));
 // Older harnesses ignore CLI arguments. An impossible CONFIG path makes
 // those known versions exit before configuration/recovery/model startup.
 const result=spawnSync(bundleExecutable(root,bundle,'gents-harness'),['--native-runtime-probe'],{cwd:root,env:{...env,...bundleEnvironment(bundle),CONFIG:fileURLToPath(new URL(bundle.manifest,root))+'/not-a-config.json'},encoding:'utf8',timeout:5000,maxBuffer:16384,windowsHide:true});
 if(result.error||result.signal||result.status!==0)throw Error('Bundled harness lacks valid runtime-probe support; rebuild the Rust workspace before starting models');
 let fact;try{fact=JSON.parse(result.stdout);}catch{throw Error('Bundled harness returned no valid runtime-probe response');}
 return verifyBundleBinding({runtime_binding:fact},bundle);
}
