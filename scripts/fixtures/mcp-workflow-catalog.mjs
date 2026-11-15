// Static reviewed fixtures only. Selection never imports model-supplied code.
import * as base from './mcp-reconciliation.mjs';
import depot from './mcp-depot-reconciliation.mjs';
import rustRanges from './mcp-rust-ranges.mjs';
import rustCents from './mcp-rust-cent-allocation.mjs';
import rustDag from './mcp-rust-dag-layers.mjs';
import rustSlots from './mcp-rust-weighted-slots.mjs';
import rustSettlement from './mcp-rust-minimum-settlement.mjs';
import audioInspection from './mcp-audio-inspection.mjs';
import audioBatch from './mcp-audio-batch.mjs';
import audioRouting from './mcp-audio-routing.mjs';
import ownedWeighted from './mcp-owned-weighted-reuse.mjs';
import ownedPrepared from './mcp-owned-prepared-reuse.mjs';
import ownedMixed from './mcp-owned-mixed-batch.mjs';
import ownedMapped from './mcp-owned-mapped-batch.mjs';
import ownedChain from './mcp-owned-allocation-chain.mjs';
import ownedBranched from './mcp-owned-branched-allocation.mjs';
import rustIndexWeights from './mcp-rust-index-weights.mjs';
import ownedMultiLibrary from './mcp-owned-multi-library.mjs';
import ownedCeoSelection from './mcp-owned-ceo-selection.mjs';
import ownedCeoBranches from './mcp-owned-ceo-branches.mjs';
import {fixtureFileSpec} from '../mcp-fixture-files.mjs';
import {createHash} from 'node:crypto';
import {readFileSync} from 'node:fs';
export const {canonical,project}=base;
const sales=[{sku:null,units:6},{sku:'A',units:8},{sku:'A',units:-8},{sku:0,units:-4},{sku:false,units:3},{sku:'0',units:100},{sku:'missing',units:60},{sku:'B',units:5},{sku:'B',units:-5},{sku:'inactive',units:9}];
const catalog=[{sku:null,category:'nullable',active:true},{sku:null,category:'shadow',active:false},{sku:'A',category:'zero',active:true},{sku:'A',category:'zero',active:true},{sku:0,category:'numeric',active:true},{sku:false,category:'boolean',active:true},{sku:'0',category:'string',active:'true'},{sku:'B',category:'cancelled',active:true},{sku:'inactive',category:'hidden',active:false}];
const files=base.files.map(f=>({...f,rows:f.input==='sales-input'?sales:catalog})).map(f=>({...f,text:JSON.stringify(f.rows,null,2)+'\n'})).map(f=>({...f,sha256:createHash('sha256').update(f.text).digest('hex')}));
const joined=[
 {sku:null,units:6,category:'nullable',active:true},{sku:null,units:6,category:'shadow',active:false},
 {sku:'A',units:8,category:'zero',active:true},{sku:'A',units:8,category:'zero',active:true},
 {sku:'A',units:-8,category:'zero',active:true},{sku:'A',units:-8,category:'zero',active:true},
 {sku:0,units:-4,category:'numeric',active:true},{sku:false,units:3,category:'boolean',active:true},
 {sku:'0',units:100,category:'string',active:'true'},{sku:'missing',units:60,category:null,active:null},
 {sku:'B',units:5,category:'cancelled',active:true},{sku:'B',units:-5,category:'cancelled',active:true},
 {sku:'inactive',units:9,category:'hidden',active:false}
];
const edges={...base,id:'reconciliation-edges',title:'Adversarial local reconciliation: nulls, multiplicity and cancellation',initial:base.initial+' Preserve null keys, duplicate multiplicity and zero-total groups.',requirements:base.requirements+' The sources also contain null keys, duplicate identical catalog records, string-valued active flags and cancelling signed movements. Null matches null under the installed JSON join contract. Preserve zero-total groups and every duplicate match; never coerce a string flag to boolean.',files,sourceInputs:base.sourceInputs.map(input=>{const f=files.find(f=>f.input===input.id);return f?{...input,arguments:{path:f.path,expected_sha256:f.sha256}}:input;}),expectedResults:new Map([
 ...files.map(f=>[f.input,{rows:f.rows,source_path:f.path,source_sha256:f.sha256,source_bytes:Buffer.byteLength(f.text)}]),
 ['join-input',{rows:joined,left_rows:10,right_rows:9,matched_left_rows:9,unmatched_left_rows:1,exact:true}],
 ['summary-input',{input_rows:13,matched_rows:9,excluded_rows:4,groups:[{key:'boolean',rows:1,total:3},{key:'cancelled',rows:2,total:0},{key:'nullable',rows:1,total:6},{key:'numeric',rows:1,total:-4},{key:'zero',rows:4,total:0}],total:5,exact:true}]
])};
const outcomeFiles=edges.files.map(file=>({...file,rows:file.input==='sales-input'?file.rows.map((row,index)=>index===0?{...row,units:7}:row):file.rows})).map(file=>({...file,text:JSON.stringify(file.rows,null,2)+'\n'})).map(file=>({...file,sha256:createHash('sha256').update(file.text).digest('hex')}));
const outcomeResults=new Map(edges.expectedResults);
for(const file of outcomeFiles)outcomeResults.set(file.input,{rows:file.rows,source_path:file.path,source_sha256:file.sha256,source_bytes:Buffer.byteLength(file.text)});
outcomeResults.set('join-input',{...outcomeResults.get('join-input'),rows:joined.map(row=>row.sku===null?{...row,units:7}:row)});
outcomeResults.set('summary-input',{...outcomeResults.get('summary-input'),groups:outcomeResults.get('summary-input').groups.map(group=>group.key==='nullable'?{...group,total:7}:group),total:6});
const outcome={...edges,id:'outcome-only-reconciliation',outcomeOnly:true,title:'CEO-designed reconciliation from file inputs and desired outputs',initial:'Reconcile the two pinned local data files for me. Choose an appropriate local workflow and independently scoped analysis and output roles.',requirements:`Use the unchanged pinned sales.json and catalog.json files. Sales records contain sku and signed units; catalog records contain sku, category and active. Reconcile each sales record with EVERY catalog record having exactly the same typed sku; numeric, boolean, string and null keys remain distinct, with null matching null. Preserve duplicate matches and input row order. Preserve unmatched sales records with null category and active. Include in the category summary only reconciled records whose active value is the JSON boolean true. Count included joined records and sum their signed units, retaining zero-total groups. Choose the minimum available local tools and staff yourself, with an analysis role followed by a separate output role. Deliver sales_rows.json and catalog_rows.json containing the respective original record arrays; joined.json containing the reconciled rows with sku, units, category and active; groups.json containing category summaries as objects with key, rows and total; total.txt containing only the included overall units integer; unmatched.txt containing only the count of original sales records without any catalog match. JSON files are compact canonical JSON with sorted object keys, summary groups sorted by their canonical JSON keys. All files have no newline and must be read back and verified against actual native results. Derive all data from the pinned input bytes. Select your own native source ids, tools, metadata and dependencies; I am not providing a tool graph or transformation templates. Do not calculate expected answers in the plan. All work stays local: no shell, network, downloads, credentials, memory or code promotion.`,files:outcomeFiles,sourceInputs:edges.sourceInputs.map(input=>{const file=outcomeFiles.find(file=>file.input===input.id);return file?{...input,arguments:{path:file.path,expected_sha256:file.sha256}}:input;}),expectedResults:outcomeResults};
outcome.planningInputs=outcome.sourceInputs.filter(input=>!input.derived_fields);
// Formalizes the already-stated independent analysis/output requirement.
outcome.planningConstraints={separate_source_writers:true};
const policy=JSON.parse(readFileSync(new URL('../../config/mcp-development-curriculum.json',import.meta.url),'utf8'));
const fixtures=new Map([['reconciliation',{...base,id:'reconciliation'}],[edges.id,edges],[outcome.id,outcome],[depot.id,depot],[rustRanges.id,rustRanges],[rustCents.id,rustCents],[rustDag.id,rustDag],[rustSlots.id,rustSlots],[rustSettlement.id,rustSettlement],[audioInspection.id,audioInspection],[audioBatch.id,audioBatch],[audioRouting.id,audioRouting],[ownedWeighted.id,ownedWeighted],[ownedPrepared.id,ownedPrepared],[ownedMixed.id,ownedMixed],[ownedMapped.id,ownedMapped],[ownedChain.id,ownedChain],[ownedBranched.id,ownedBranched],[rustIndexWeights.id,rustIndexWeights],[ownedMultiLibrary.id,ownedMultiLibrary],[ownedCeoSelection.id,ownedCeoSelection],[ownedCeoBranches.id,ownedCeoBranches]]);
const seen=new Map();
export const levels=policy.levels.map(level=>{if(seen.has(level.id)||!fixtures.has(level.id)||!Number.isSafeInteger(level.level)||level.level<1||!level.requires.every(id=>seen.has(id)&&seen.get(id)<level.level))throw Error('Invalid reviewed curriculum progression');seen.set(level.id,level.level);return {...level,fixture:fixtures.get(level.id)};});
export function fixtureFromId(id){const level=levels.find(level=>level.id===id);if(!level)throw Error('Unknown reviewed workflow fixture');return level.fixture;}
export function fixtureDigest(fixture){if(fixture.kind==='rust_library')return createHash('sha256').update(canonical({version:1,...fixture})).digest('hex');return createHash('sha256').update(canonical({version:1,id:fixture.id,title:fixture.title,initial:fixture.initial,requirements:fixture.requirements,files:fixture.files.map(fixtureFileSpec),sourceInputs:fixture.sourceInputs,dependencies:fixture.dependencies,artifacts:fixture.artifacts,expectedResults:[...fixture.expectedResults],...(fixture.ownedComponents?{ownedComponents:fixture.ownedComponents}:{}),...(fixture.ownerPreparation?{ownerPreparation:fixture.ownerPreparation}:{}),...(fixture.selection?{selection:fixture.selection}:{}),...(fixture.outcomeOnly?{planning_mode:'outcome_only_v1',planningInputs:fixture.planningInputs,...(fixture.planningConstraints?{planningConstraints:fixture.planningConstraints}:{})}:{})})).digest('hex');}
