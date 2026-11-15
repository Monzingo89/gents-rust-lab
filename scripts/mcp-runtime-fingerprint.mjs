// Observed executable bytes, never proof of source/build correspondence or
// authority. Missing historical fingerprints remain unknown, not reconstructed.
import {createHash} from 'node:crypto';
import {constants} from 'node:fs';
import {lstat,open} from 'node:fs/promises';

export async function runtimeFingerprint(root){
 const binaries={};
 for(const name of ['gents-harness','gents-local-mcp','gents-sqlite-bridge']){
  const path=`target/debug/${name}${process.platform==='win32'?'.exe':''}`,url=new URL(path,root);
  const observed=await lstat(url);if(!observed.isFile()||observed.isSymbolicLink()||observed.size>512*1024*1024)throw Error('Runtime fingerprint requires bounded ordinary executables');
  const file=await open(url,constants.O_RDONLY|(constants.O_NOFOLLOW??0));
  try{
   const before=await file.stat();if(!before.isFile()||before.dev!==observed.dev||before.ino!==observed.ino)throw Error('Runtime executable changed before fingerprint');
   const hash=createHash('sha256');let bytes=0;
   for await(const chunk of file.createReadStream({autoClose:false})){bytes+=chunk.length;if(bytes>512*1024*1024)throw Error('Runtime executable exceeds fingerprint bound');hash.update(chunk);}
   const after=await file.stat();if(before.size!==after.size||bytes!==after.size||before.mtimeMs!==after.mtimeMs||before.ctimeMs!==after.ctimeMs)throw Error('Runtime executable changed during fingerprint');
   binaries[path]={sha256:hash.digest('hex'),bytes};
  }finally{await file.close();}
 }
 return {schema_version:1,binaries,scope:'Observed local executable files before launch; not reproducible-build proof, model provenance, running-process attestation or historical reconstruction'};
}
