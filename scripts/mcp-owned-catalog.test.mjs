import test from 'node:test';
import assert from 'node:assert/strict';
import {mkdtemp,mkdir,copyFile,chmod,writeFile,rm,readdir,symlink} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {pathToFileURL} from 'node:url';
import {createHash} from 'node:crypto';
import {publishComponentReview,listComponentIds} from './mcp-component-store.mjs';
import {ownedComponentCatalog,ownedComponentSource} from './mcp-owned-catalog.mjs';
const sha=text=>createHash('sha256').update(text).digest('hex');
async function temporary(body){
 const folder=await mkdtemp(join(tmpdir(),'gents-owned-catalog-')),root=pathToFileURL(folder+'/');
 try{
  await mkdir(new URL('target/debug/',root),{recursive:true,mode:0o700});
  await copyFile(new URL('../target/debug/gents-rust-component',import.meta.url),new URL('target/debug/gents-rust-component',root));
  await body(root);
 }finally{
  const writable=async p=>{await chmod(p,0o700);for(const e of await readdir(p,{withFileTypes:true}))if(e.isDirectory())await writable(new URL(e.name+'/',p));};await writable(root);await rm(root,{recursive:true,force:true});
 }
}
async function publish(root,company,source){
 const digest=sha(source);
 return publishComponentReview(root,{'source.rs':source,'acceptance.rs':'PRIVATE_ACCEPTANCE_BODY','qualification.json':JSON.stringify({fixture_simulated:true,private_payload:'PRIVATE_CONVERSATION',proof:{status:'PASS',source_is_local_model_authored:true,source_sha256:digest,provider_cost_usd:0}})},{company,fixture_id:'protocol-fixture',source_sha256:digest});
}
test('empty catalog and source interface inspection never invoke a candidate or expose private bodies',async()=>temporary(async root=>{
 assert.deepEqual(await listComponentIds(root),[]);
 const packet=await publish(root,'one','pub fn must_not_run()->u64{loop{}} // PRIVATE_SOURCE_COMMENT');
 const c=await ownedComponentCatalog(root);assert.equal(c.components.length,1);assert.equal(c.components[0].component_id,packet.id);
 const privateSource=await ownedComponentSource(root,packet.id);assert.equal(privateSource.origin_company,'one');assert(privateSource.text.includes('PRIVATE_SOURCE_COMMENT'));
 assert.equal(c.components[0].functions[0].name,'must_not_run');assert.equal(c.execution_authorized,false);assert.equal(c.promotion_authorized,false);
 for(const marker of ['PRIVATE_SOURCE_COMMENT','PRIVATE_ACCEPTANCE_BODY','PRIVATE_CONVERSATION','loop{'])assert(!JSON.stringify(c).includes(marker));
 assert.equal((await ownedComponentCatalog(root)).catalog_sha256,c.catalog_sha256);
}));
test('tampering and package aliases are unavailable, never silently trusted or executed',async()=>temporary(async root=>{
 const good=await publish(root,'one','pub fn first()->u64{1}'),bad=await publish(root,'two','pub fn second()->u64{2}');
 const source=new URL(`data/owned-rust-components/${bad.id}/source.rs`,root);await chmod(source,0o600);await writeFile(source,'tampered');await chmod(source,0o400);
 await symlink(new URL(`data/owned-rust-components/${good.id}`,root),new URL('data/owned-rust-components/'+'0'.repeat(64),root));
 const c=await ownedComponentCatalog(root);assert.deepEqual(c.components.map(c=>c.component_id),[good.id]);assert.equal(c.rejected.length,2);
 await assert.rejects(ownedComponentSource(root,bad.id));await assert.rejects(ownedComponentSource(root,'0'.repeat(64)));
 assert(c.rejected.every(r=>r.status==='unavailable_integrity_qualification_or_interface'));
}));
