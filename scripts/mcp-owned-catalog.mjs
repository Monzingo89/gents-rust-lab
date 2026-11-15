// Private host metadata adapter. Source inspection only, never invocation.
import assert from 'node:assert/strict';
import {parseArgs} from 'node:util';
import {pathToFileURL} from 'node:url';
import {resolve,sep} from 'node:path';
import {createHash} from 'node:crypto';
import {canonical} from './fixtures/mcp-reconciliation.mjs';
import {listComponentIds,readOwnedEvidence,verifyComponentPackage} from './mcp-component-store.mjs';
import {probeComponent} from './mcp-component-probe.mjs';
const hash=bytes=>createHash('sha256').update(bytes).digest('hex');
export async function ownedComponentCatalog(root){
 const components=[],rejected=[];
 for(const id of await listComponentIds(root)){
  try{
   const packet=await verifyComponentPackage(root,id),company=packet.context.company,fixture=packet.context.fixture_id;
   assert(typeof company==='string'&&/^[a-z][a-z0-9-]{0,63}$/.test(company),'Source company identity required');
   assert(typeof fixture==='string'&&/^[a-z][a-z0-9-]{0,63}$/.test(fixture),'Reviewed fixture identity required');
   const qualification=JSON.parse(await readOwnedEvidence(root,`data/owned-rust-components/${id}/qualification.json`,8*1024*1024,true));
   assert(qualification.proof.status==='PASS'&&qualification.proof.source_is_local_model_authored===true&&qualification.proof.provider_cost_usd===0,'Historical locally authored fixture qualification required');
   assert.equal(qualification.proof.source_sha256,packet.context.source_sha256);
   const probe=await probeComponent(root,id),description=probe.description;
   assert.equal(description.source_sha256,packet.context.source_sha256);
   const item={component_id:id,origin_company:company,origin_fixture:fixture,source_sha256:description.source_sha256,api_sha256:description.api_sha256,
    abi:description.interface.abi,integer_encoding:description.interface.integer_encoding,functions:description.interface.functions,
    inspector_sha256:probe.helper_sha256,historical_qualification:'fixture_snapshot_only',integrity_checked_now:true,execution_authorized:false,promotion_authorized:false};
   assert(Buffer.byteLength(canonical(item))<=32768,'Component metadata exceeds its bound');
   assert(Buffer.byteLength(canonical([...components,item]))<=196608,'Catalog metadata exceeds its bound');components.push(item);
  }catch{rejected.push({component_id:id,status:'unavailable_integrity_qualification_or_interface'});}
 }
 const body={schema_version:1,components,rejected,execution_authorized:false,promotion_authorized:false,
  scope:'Private host inventory; caller must filter source-company ownership before disclosure. Exact stored-source interface and historical fixture metadata only. No source/test bodies, conversation payloads, candidate execution, fresh task qualification, authority or model learning.'};
 return {...body,catalog_sha256:hash(canonical(body))};
}
// Private host custody path, absent from the browser and actor MCP surfaces.
export async function ownedComponentSource(root,id){
 const packet=await verifyComponentPackage(root,id);
 const bytes=await readOwnedEvidence(root,`data/owned-rust-components/${id}/source.rs`,8192,true);
 assert.equal(hash(bytes),packet.context.source_sha256);
 const text=bytes.toString('utf8');assert(Buffer.from(text).equals(bytes));
 return {component_id:id,origin_company:packet.context.company,source_sha256:packet.context.source_sha256,text};
}
if(process.argv[1]&&import.meta.url===pathToFileURL(process.argv[1]).href){
 const {values}=parseArgs({options:{root:{type:'string'},source:{type:'string'}}});
 const root=values.root?pathToFileURL(resolve(values.root)+sep):new URL('../',import.meta.url);
 const result=values.source?await ownedComponentSource(root,values.source):await ownedComponentCatalog(root);assert(Buffer.byteLength(JSON.stringify(result))<=262144);console.log(JSON.stringify(result));
}
