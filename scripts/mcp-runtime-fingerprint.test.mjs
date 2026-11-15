import test from 'node:test';import assert from 'node:assert/strict';
import {mkdtemp,mkdir,writeFile,rm,symlink} from 'node:fs/promises';import{tmpdir}from'node:os';import{join}from'node:path';import{pathToFileURL}from'node:url';
import{runtimeFingerprint}from'./mcp-runtime-fingerprint.mjs';
test('observed binary identity changes with bytes and rejects absent or symlink replacements',async()=>{
 const dir=await mkdtemp(join(tmpdir(),'gents-runtime-fingerprint-')),root=pathToFileURL(dir+'/');
 const file=name=>new URL(`target/debug/${name}${process.platform==='win32'?'.exe':''}`,root);
 try{
  await mkdir(new URL('target/debug/',root),{recursive:true});
  await assert.rejects(runtimeFingerprint(root));
  for(const name of ['gents-harness','gents-local-mcp','gents-sqlite-bridge'])await writeFile(file(name),`fixture ${name}`);
  const original=await runtimeFingerprint(root);await writeFile(file('gents-harness'),'changed executable fixture');
  const changed=await runtimeFingerprint(root),key=`target/debug/gents-harness${process.platform==='win32'?'.exe':''}`;
  assert.notEqual(original.binaries[key].sha256,changed.binaries[key].sha256);
  assert.equal(Object.keys(changed.binaries).length,3);
  await rm(file('gents-sqlite-bridge'));await symlink(file('gents-harness'),file('gents-sqlite-bridge'));
  await assert.rejects(runtimeFingerprint(root));
 }finally{await rm(dir,{recursive:true,force:true});}
});
