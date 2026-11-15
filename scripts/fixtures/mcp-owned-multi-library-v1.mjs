// Host-only synthetic benchmark. All three function bodies were authored locally.
import scheduling from './mcp-owned-weighted-reuse.mjs';
import allocation,{allocateOracle} from './mcp-owned-allocation-chain.mjs';
import {finiteScheduling} from './mcp-owned-mixed-batch.mjs';
const adapter={source_path:'owned-index-weights.rs',source_sha256:'fa4d378c0b06789dcac956c115fe733e49bea02829293bacaa1b451878b69d11',api_sha256:'68df0a91377cabc8ddac45691a8a61abe28b49061f6bad0a6dbc795f38e4ae1e',function:'index_weights',owner_declared_review_package_id:'b303664178bcff536110b89b6624515065a4955c13de38cd8fc77cc8d21cf856'};
const requestsA=[['0','2','5'],['2','5','7'],['0','5','12'],['5','8','9'],['8','11','7'],['5','11','16'],['1','3','6'],['3','6','7'],['6','9','8'],['9','12','9'],['12','14','4'],['14','17','6'],['12','17','10'],['0','17','42'],['17','19','8'],['19','22','9']];
const maximum='18446744073709551615';
const requestsB=[['0','1',maximum],['1','3',maximum],['0','3',maximum],['3','5','11'],['5','8','13'],['3','8','24'],['8','10','17'],['10','12','19'],['8','12','36'],['12','18446744073709551614','5'],['18446744073709551614',maximum,'7'],['12',maximum,'12']];
const definitions=[
 ['a-schedule',scheduling.ownedComponents[0],[requestsA],null],
 ['b-weights',adapter,[[]],'a-schedule'],
 ['c-distribute',allocation.ownedComponents[0],[maximum,[]],'b-weights'],
 ['d-rebalance',allocation.ownedComponents[0],['999983',[]],'c-distribute'],
 ['e-schedule',scheduling.ownedComponents[0],[requestsB],null],
 ['f-weights',adapter,[[]],'e-schedule'],
 ['g-distribute',allocation.ownedComponents[0],['1009',[]],'f-weights'],
 ['h-rebalance',allocation.ownedComponents[0],['65537',[]],'g-distribute']
];
const sourceInputs=definitions.map(([id,spec,args,parent])=>({id,tool:'owned_rust_call',arguments:{binding_id:id,arguments:args},...(parent?{argument_slots:[spec.function==='index_weights'?0:1]}:{})}));
// Independent finite exhaustive scheduling, mathematical one-based conversion,
// and exact allocation oracles. No expected results enter model fields or files.
const expectedResults=new Map();
for(const [id,spec,args,parent] of definitions){
 const values=parent?expectedResults.get(parent).output.some:null;
 const output=spec.function==='select_slots'?finiteScheduling(args[0]):spec.function==='index_weights'?{some:values.map(i=>String(BigInt(i)+1n))}:allocateOracle(args[0],values);
 expectedResults.set(id,{output});
}
const ownedComponents=definitions.map(([input,spec])=>({...spec,input}));
const files=[scheduling.files[0],allocation.files[0],{path:adapter.source_path,input:'owned-index-weights',owned_component:adapter.owner_declared_review_package_id,sha256:adapter.source_sha256}];
export default {
 ...allocation,id:'owned-multi-library',title:'Locally authored adapter joins scheduling and allocation libraries',industry:'synthetic_operations_interoperability',complexity:'hard',
 complexity_basis:'Relative to the single-library branched workflow: three separately qualified local libraries, two distinct numeric wire types joined by an explicitly authored checked adapter, eight calls, six native edges, two four-node chains and exact wide arithmetic. Same node count, a new composition dimension; no universal extremely-hard rating or industry assurance.',
 initial:'Use our prepared scheduling, index-to-weight and allocation operations to produce two final local reports. Choose the actors and dependency plan. Ask for exact output and acceptance requirements before work.',
 requirements:'Use every prepared invocation unchanged. a-schedule and e-schedule use our stored select_slots function on their respective supplied requests. b-weights uses our stored index_weights function on the actual selected-index vector returned by a-schedule; f-weights uses index_weights on the actual selected-index vector returned by e-schedule. This explicit owned adapter maps zero-based indices to positive one-based u64 weights with checked conversion; never bypass it with an implicit cast. c-distribute uses our stored allocate_cents function with its supplied total and the actual weights returned by b-weights; d-rebalance uses allocate_cents with its supplied total and the actual allocation vector returned by c-distribute. Independently, g-distribute uses allocate_cents with its supplied total and actual weights returned by f-weights; h-rebalance uses allocate_cents with its supplied total and the actual allocation vector returned by g-distribute. Keep every supplied request and total unchanged. Choose compatible native result projections and positional references yourself from the inspected interfaces. All eight valid calls must return present results. Publish only the complete native /output of d-rebalance to dispatch-share.json and the complete native /output of h-rebalance to reserve-share.json, as compact canonical JSON with sorted object keys and no newline; read both back and compare each to its own native result. No intermediate files or repeated invocations. Choose one computation actor for all calls followed by a distinct publication actor; choose minimum tools and stage-scoped acceptance criteria. Scheduling uses nonoverlapping half-open intervals and exact maximum priority, breaking ties by the lexicographically smallest increasing original-index vector. Allocation uses exact largest remainders, preserving recipient order and breaking equal remainders by smaller original index. Integers use canonical decimal strings; optional results use {some:value}/{none:true}. Do not calculate answers, invent vectors, supply code, modify sources, promote sources, use shell/network/downloads/credentials/memory tools or paid services. Source bodies remain in owner custody and are staged only after approval. These fictional operations are synthetic arithmetic and interoperability only, without real bookings, accounts, payments, clinical advice or compliance certification.',
 sourceInputs,planningInputs:sourceInputs,ownedComponents,files,
 dependencies:Object.fromEntries(definitions.map(([id,spec,,parent])=>[id,parent?[{field:'arguments',argument_index:spec.function==='index_weights'?0:1,input:parent,pointer:'/output/some'}]:[]])),
 artifacts:[{path:'dispatch-share.json',input:'d-rebalance',pointer:'/output',encoding:'json'},{path:'reserve-share.json',input:'h-rebalance',pointer:'/output',encoding:'json'}],expectedResults
};
