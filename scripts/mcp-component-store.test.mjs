import test from 'node:test';
import assert from 'node:assert/strict';
import {mkdtemp,mkdir,writeFile,readFile,chmod,symlink,link,rm,stat,readdir,rename} from 'node:fs/promises';
import {createHash} from 'node:crypto';
import {tmpdir} from 'node:os';
import {pathToFileURL} from 'node:url';
import {join} from 'node:path';
import {readOwnedEvidence,publishComponentReview,verifyComponentPackage} from './mcp-component-store.mjs';
import {canonical} from './fixtures/mcp-reconciliation.mjs';
const payload=()=>({'source.rs':'protocol source data','acceptance.rs':'protocol acceptance data','qualification.json':'{"fixture_only":true}'});
async function temporary(t){
 const path=await mkdtemp(join(tmpdir(),'gents-review-store-')),root=pathToFileURL(path+'/');await chmod(root,0o700);
 t.after(async()=>{const writable=async url=>{await chmod(url,0o700);for(const entry of await readdir(url,{withFileTypes:true}))if(entry.isDirectory())await writable(new URL(entry.name+'/',url));};await writable(root);await rm(root,{recursive:true,force:true});});
 return root;
}

test('publishes stable non-executable review content; cached exports are verified rather than overwritten',async t=>{
 const root=await temporary(t),first=await publishComponentReview(root,payload(),{run:'protocol-fixture'}),again=await publishComponentReview(root,payload(),{run:'protocol-fixture'});
 assert.equal(first.id,again.id);assert.equal(first.execution_authorized,false);assert.equal(first.promotion_authorized,false);
 const folder=new URL('data/owned-rust-components/'+first.id+'/',root);
 assert.equal((await stat(folder)).mode&0o222,0);
 for(const name of ['source.rs','acceptance.rs','qualification.json','manifest.json'])assert.equal((await stat(new URL(name,folder))).mode&0o333,0);
 assert.equal((await readOwnedEvidence(root,'data/owned-rust-components/'+first.id+'/source.rs',100,true)).toString(),payload()['source.rs']);
 assert.equal((await verifyComponentPackage(root,first.id)).id,first.id);
 const changed=await publishComponentReview(root,{...payload(),'source.rs':'changed protocol bytes'},{run:'protocol-fixture'});assert.notEqual(changed.id,first.id);
});

test('rejects traversal, symlink parents/files, hard links, writable paths and oversized reads',async t=>{
 const root=await temporary(t);await mkdir(new URL('data/',root),{mode:0o700});await mkdir(new URL('data/real/',root),{mode:0o700});await writeFile(new URL('data/real/a',root),'bounded');
 await symlink('real',new URL('data/alias',root));await symlink('a',new URL('data/real/symlink',root));
 for(const path of ['../elsewhere','data/../real/a','data/%2e%2e/a','data/alias/a','data/real/symlink'])await assert.rejects(()=>readOwnedEvidence(root,path));
 await assert.rejects(()=>readOwnedEvidence(root,'data/real/a',2));
 await link(new URL('data/real/a',root),new URL('data/real/hardlink',root));await assert.rejects(()=>readOwnedEvidence(root,'data/real/a'));
 await chmod(new URL('data/real/',root),0o777);await assert.rejects(()=>readOwnedEvidence(root,'data/real/a'));
 const linkedRoot=new URL('linked-root/',root);await symlink('data',new URL('linked-root',root));await assert.rejects(()=>readOwnedEvidence(linkedRoot,'real/a'));
});

test('tampered cached content is rejected and retained; no replacement or silent resealing',async t=>{
 const root=await temporary(t),created=await publishComponentReview(root,payload(),{}),folder=new URL('data/owned-rust-components/'+created.id+'/',root),source=new URL('source.rs',folder);
 await chmod(source,0o600);await writeFile(source,'tampered');await chmod(source,0o400);
 await assert.rejects(()=>verifyComponentPackage(root,created.id),/digest/);
 await assert.rejects(()=>publishComponentReview(root,payload(),{}),/digest/);
 assert.equal(await readFile(source,'utf8'),'tampered');
 const entries=await readdir(new URL('data/owned-rust-components/',root));assert.deepEqual(entries,[created.id]);
});

test('rejects executable/extra payload entries before publication and manifest authority claims',async t=>{
 const root=await temporary(t);
 await assert.rejects(()=>publishComponentReview(root,{...payload(),'run.sh':'unexpected'},{}));
 const created=await publishComponentReview(root,payload(),{}),folder=new URL('data/owned-rust-components/'+created.id+'/',root),manifest=new URL('manifest.json',folder);
 const value=JSON.parse(await readFile(manifest,'utf8'));value.execution_authorized=true;
 const {id,...body}=value;value.id=createHash('sha256').update(canonical(body)).digest('hex');
 await chmod(manifest,0o600);await writeFile(manifest,JSON.stringify(value));await chmod(manifest,0o400);
 await rename(folder,new URL('data/owned-rust-components/'+value.id+'/',root));
 await assert.rejects(()=>verifyComponentPackage(root,value.id),/cannot grant execution/);
});
