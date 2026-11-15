import test from 'node:test';import assert from 'node:assert/strict';
import {mkdtemp,mkdir,writeFile,readFile,rm,chmod,symlink,lstat,rename} from 'node:fs/promises';import {tmpdir} from 'node:os';import {pathToFileURL} from 'node:url';import {join} from 'node:path';import {createHash} from 'node:crypto';
import {createRuntimeBundle,nativeRoles,bundleEnvironment,bundleExecutable,verifyBundleBinding} from './mcp-runtime-bundle.mjs';
test('sealed copies survive source rebuilds and bind exact private manifest bytes',{skip:process.platform==='win32'},async()=>{
 const path=await mkdtemp(join(tmpdir(),'gents-bundle-')),root=pathToFileURL(path+'/');let bundle;
 try{
  await mkdir(new URL('target/debug/',root),{recursive:true});for(const role of nativeRoles)await writeFile(new URL('target/debug/'+role,root),'original '+role,{mode:0o700});
  bundle=await createRuntimeBundle(root);const manifest=await readFile(new URL(bundle.manifest,root));assert.equal(bundle.manifest_sha256,createHash('sha256').update(manifest).digest('hex'));
  const reused=await createRuntimeBundle(root);assert.equal(reused.id,bundle.id);assert.equal(reused.manifest_sha256,bundle.manifest_sha256);
  assert.equal(bundleEnvironment(bundle).GENTS_RUNTIME_MANIFEST,bundle.manifest);assert.equal(Object.keys(bundle.executables).length,4);
  for(const role of nativeRoles){await writeFile(new URL('target/debug/'+role,root),'rebuilt '+role);const actual=await readFile(bundleExecutable(root,bundle,role));assert.equal(actual.toString(),'original '+role);assert.equal(createHash('sha256').update(actual).digest('hex'),bundle.executables[role].sha256);assert.equal((await lstat(bundleExecutable(root,bundle,role))).mode&0o222,0);}
  assert.throws(()=>bundleExecutable(root,bundle,'unregistered'));
 }finally{if(bundle)await chmod(new URL('.',new URL(bundle.manifest,root)),0o700);await rm(path,{recursive:true,force:true});}
});
test('missing and symlinked native sources fail without publishing a bundle',{skip:process.platform==='win32'},async()=>{
 const path=await mkdtemp(join(tmpdir(),'gents-bundle-reject-')),root=pathToFileURL(path+'/');
 try{
  await mkdir(new URL('target/debug/',root),{recursive:true});await assert.rejects(createRuntimeBundle(root));
  for(const role of nativeRoles)await writeFile(new URL('target/debug/'+role,root),'fixture',{mode:0o700});
  await rm(new URL('target/debug/gents-local-mcp',root));await symlink(new URL('target/debug/gents-harness',root),new URL('target/debug/gents-local-mcp',root));await assert.rejects(createRuntimeBundle(root),/owner-controlled/);
 }finally{await rm(path,{recursive:true,force:true});}
});
test('cached copies are verified before reuse and corruption never falls back',{skip:process.platform==='win32'},async()=>{
 const path=await mkdtemp(join(tmpdir(),'gents-bundle-tamper-')),root=pathToFileURL(path+'/');let bundle;
 try{await mkdir(new URL('target/debug/',root),{recursive:true});for(const role of nativeRoles)await writeFile(new URL('target/debug/'+role,root),'original',{mode:0o700});bundle=await createRuntimeBundle(root);const binary=bundleExecutable(root,bundle,'gents-local-mcp');await chmod(binary,0o700);await writeFile(binary,'tampered');await chmod(binary,0o500);await assert.rejects(createRuntimeBundle(root),/digest changed/);}finally{if(bundle)await chmod(new URL('.',new URL(bundle.manifest,root)),0o700);await rm(path,{recursive:true,force:true});}
});
test('directory aliases cannot become ordinary native source directories through trailing slashes',{skip:process.platform==='win32'},async()=>{
 const path=await mkdtemp(join(tmpdir(),'gents-bundle-directory-')),root=pathToFileURL(path+'/');let unexpected;
 try{
  await mkdir(new URL('target/actual/',root),{recursive:true});for(const role of nativeRoles)await writeFile(new URL('target/actual/'+role,root),'directory protocol fixture',{mode:0o700});
  await symlink('actual',new URL('target/debug',root));
  await assert.rejects(async()=>{unexpected=await createRuntimeBundle(root);},/owner-controlled/);
 }finally{if(unexpected)await chmod(new URL('.',new URL(unexpected.manifest,root)),0o700);await rm(path,{recursive:true,force:true});}
});
test('a cached native bundle directory cannot be replaced by an alias to matching sealed bytes',{skip:process.platform==='win32'},async()=>{
 const path=await mkdtemp(join(tmpdir(),'gents-bundle-cached-alias-')),root=pathToFileURL(path+'/');let moved;
 try{
  await mkdir(new URL('target/debug/',root),{recursive:true});for(const role of nativeRoles)await writeFile(new URL('target/debug/'+role,root),'directory protocol fixture',{mode:0o700});
  const bundle=await createRuntimeBundle(root),folder=new URL('.',new URL(bundle.manifest,root));moved=new URL('data/mcp-runtime-bundles/owned-moved/',root);
  await rename(folder,moved);await symlink('owned-moved',new URL('data/mcp-runtime-bundles/'+bundle.id,root));
  await assert.rejects(createRuntimeBundle(root),/owner-controlled/);
 }finally{if(moved)await chmod(moved,0o700);await rm(path,{recursive:true,force:true});}
});
test('a legacy or mismatched harness cannot start model work while silently ignoring bundle selection',()=>{
 const bundle={schema_version:1,id:'a'.repeat(64),manifest:'data/mcp-runtime-bundles/'+'a'.repeat(64)+'/manifest.json',manifest_sha256:'b'.repeat(64),executables:Object.fromEntries(nativeRoles.map(role=>[role,{file:role+(process.platform==='win32'?'.exe':''),bytes:1,sha256:'c'.repeat(64)}]))};
 const binding={supported:true,mode:'sealed_bundle',bundle_verified:true,manifest_sha256:bundle.manifest_sha256,mcp_executable:bundle.manifest.replace('manifest.json',bundle.executables['gents-local-mcp'].file)};
 assert.equal(verifyBundleBinding({runtime_binding:binding},bundle),binding);
 for(const changed of [undefined,{...binding,supported:false},{...binding,bundle_verified:false},{...binding,mode:'development_directory'},{...binding,manifest_sha256:'c'.repeat(64)},{...binding,mcp_executable:'target/debug/gents-local-mcp'}])assert.throws(()=>verifyBundleBinding({runtime_binding:changed},bundle),/did not verify/);
 assert.throws(()=>bundleEnvironment({...bundle,manifest:'../outside/manifest.json'}),/Invalid/);
 assert.throws(()=>bundleExecutable(new URL('./',import.meta.url),{...bundle,executables:{...bundle.executables,'gents-local-mcp':{...bundle.executables['gents-local-mcp'],file:'../../outside'}}},'gents-local-mcp'),/Invalid/);
});
