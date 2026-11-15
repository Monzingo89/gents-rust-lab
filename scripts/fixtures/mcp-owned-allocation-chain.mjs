// Owned benchmark/oracle only. Domain source is the sealed, locally authored component.
const component='1ca45cd95e2ecebbf58213bc5b531298246b02b26be1316bf1ceaeb279af8087';
const source='2fa545e38918fc7e01e4b1524f29d699702dc4168ba642196ead6808071e23b5';
const api='363c8080343c39321d76c6282755aed0de37ae533cbc693b22146d39a61d3826';
// Independent exact quota/remainder oracle. Never sent in a model packet or staged file.
export function allocateOracle(total,weights){
 const t=BigInt(total),w=weights.map(BigInt);if(w.length>64)return {none:true};
 const sum=w.reduce((a,b)=>a+b,0n);if(sum===0n)return t===0n?{some:w.map(()=> '0')}:{none:true};
 const q=w.map((value,index)=>({index,whole:t*value/sum,remainder:t*value%sum}));
 const remaining=t-q.reduce((a,b)=>a+b.whole,0n);const order=[...q].sort((a,b)=>a.remainder===b.remainder?a.index-b.index:a.remainder>b.remainder?-1:1);
 for(let i=0n;i<remaining;i++)q[order[Number(i)].index].whole++;
 return {some:q.map(v=>String(v.whole))};
}
const first={id:'allocation-01',tool:'owned_rust_call',arguments:{binding_id:'allocation-01',arguments:['23',['2','3','5']]}};
const second={id:'allocation-02',tool:'owned_rust_call',arguments:{binding_id:'allocation-02',arguments:['101',[]]},argument_slots:[1]};
const initial=allocateOracle(...first.arguments.arguments),final=allocateOracle('101',initial.some);
export default {
 id:'owned-allocation-chain',title:'CEO-selected dependent reuse with one final publication',outcomeOnly:true,ownerPreparation:'retained_component_v1',
 industry:'synthetic_general_operations_and_fintech',complexity:'medium',modalities:['text'],
 complexity_basis:'New dependency capability relative to the curriculum: two owner-held invocations, one typed result-to-argument edge, one receipt-only intermediate and one final file. This is not universally harder than an eight-root batch, a multi-library qualification or industry assurance.',
 initial:'Use our two prepared local allocation operations as a dependent workflow. Choose separate computation and final-publication actors. Ask for exact output and acceptance requirements before work.',
 requirements:'Use every prepared input unchanged. allocation-01 allocates its total across its supplied weights using our stored allocate_cents function. Use the actual returned allocation vector from allocation-01 as the weights for allocation-02; its total is already supplied. Choose the native dependency and compatible return projection yourself from the inspected interface. Literal totals and the initial weights must not change. Both valid calls must succeed through the owned native adapter. Save only the complete native /output of allocation-02 to final-allocation.json as compact canonical JSON with sorted object keys and no newline, then read it back and compare it to that native result. Do not publish an intermediate file. Choose one computation actor for both invocations followed by a different final-publication actor, and minimum tools. The native function uses exact largest-remainder allocation, preserving recipient order and breaking equal remainders by smaller original index; it uses canonical decimal strings and {some:value}/{none:true} optional results. The selected valid inputs require a present returned vector for the dependency. Do not calculate answers, invent weights, supply allocation vectors, author or modify code, use shell/network/downloads/credentials/memory tools/paid services or promote sources. Source bytes stay in host custody and are staged only after approval. These fictional cent amounts are synthetic arithmetic only, with no real accounts, payments, advice or compliance certification.',
 planningConstraints:{separate_source_writers:true},sourceInputs:[first,second],planningInputs:[first,second],
 ownedComponents:[first,second].map(i=>({input:i.id,source_path:`owned-${component}.rs`,source_sha256:source,api_sha256:api,function:'allocate_cents',owner_declared_review_package_id:component})),
 files:[{path:`owned-${component}.rs`,input:'allocate_cents',owned_component:component,sha256:source}],
 dependencies:{'allocation-01':[],'allocation-02':[{field:'arguments',argument_index:1,input:'allocation-01',pointer:'/output/some'}]},
 artifacts:[{path:'final-allocation.json',input:'allocation-02',pointer:'/output',encoding:'json'}],
 expectedResults:new Map([['allocation-01',{output:initial}],['allocation-02',{output:final}]])
};
