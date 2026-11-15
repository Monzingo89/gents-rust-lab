// Trusted synthetic benchmark. Expected outputs stay outside every model packet.
// Domain implementations come only from the two sealed locally authored packages.
const scheduling={id:'f995cd27fa8782a9ee59fb84286e997ddfb01f39fd3a4284d0d40f769fdcf14c',source:'8a3ec5c2ca9ed65da60be122e0a1f740f2b739e3f98f97dbe8a4a4b7bc96e8a7',api:'337a6eec5e59505b1527dcd712998c03f2357ae7697c0b29089a4c40d075f487',fn:'select_slots'};
const settlement={id:'12212c0f7d2fa0247840b2bbaed1e61381b7419f29d866e77f2e18f3edaf31e0',source:'72860e886ae61b78d54dec9ce063fc4b611143b617c0dc625c9d8206a2f455a9',api:'e18404ea206d3698e32629da19dfcbef766c261bc0d338e9c7e2677463e19c52',fn:'settle_balances'};
const specs=[
 ['schedule-overlap',scheduling,[['0','4','9'],['4','7','8'],['0','7','16'],['7','9','4'],['9','12','5'],['7','12','9'],['1','3','6'],['3','6','7'],['6','8','6'],['8','10','7'],['10','12','6'],['0','12','30']]],
 ['schedule-wide',scheduling,[['0','2','18446744073709551615'],['2','4','18446744073709551615'],['0','4','18446744073709551615'],['4','6','1'],['2','6','18446744073709551615'],['6','8','2'],['8','9','3'],['6','9','5']]],
 ['schedule-invalid',scheduling,[['4','4','2']]],['schedule-empty',scheduling,[]],
 ['settlement-branch',settlement,['-8','-7','15']],
 ['settlement-wide',settlement,['-9223372036854775808','9223372036854775807','1']],
 ['settlement-invalid',settlement,['1','2','-2']],['settlement-empty',settlement,[]]
].sort((a,b)=>a[0].localeCompare(b[0]));
// Finite owner-side exhaustive subset oracle, never a tool or model input.
export function finiteScheduling(rows){
 const input=rows.map(r=>r.map(BigInt));if(input.some(([s,e,w])=>e<=s||w===0n))return {none:true};
 const lex=(a,b)=>{for(let i=0;i<Math.min(a.length,b.length);i++)if(a[i]!==b[i])return a[i]<b[i];return a.length<b.length;};
 let best=[],score=-1n;
 for(let mask=0;mask<2**input.length;mask++){
  const chosen=input.map((_,i)=>i).filter(i=>mask&(1<<i));
  if(chosen.some(i=>chosen.some(j=>i<j&&input[i][0]<input[j][1]&&input[j][0]<input[i][1])))continue;
  const weight=chosen.reduce((sum,i)=>sum+input[i][2],0n);
  if(weight>score||weight===score&&lex(chosen,best)){score=weight;best=chosen;}
 }
 return {some:best.map(String)};
}
const settlements={
 'settlement-branch':{some:[['0','2','8'],['1','2','7']]},
 'settlement-wide':{some:[['0','1','9223372036854775807'],['0','2','1']]},
 'settlement-invalid':{none:true},'settlement-empty':{some:[]}
};
const sourceInputs=specs.map(([id,_,input])=>({id,tool:'owned_rust_call',arguments:{binding_id:id,arguments:[input]}}));
const ownedComponents=specs.map(([input,c])=>({input,source_path:`owned-${c.id}.rs`,source_sha256:c.source,api_sha256:c.api,function:c.fn,owner_declared_review_package_id:c.id}));
export default {
 id:'owned-mixed-batch',title:'Eight owner-prepared operations across scheduling and balance settlement',outcomeOnly:true,ownerPreparation:'retained_component_v1',
 industry:'synthetic_general_operations_and_fintech',complexity:'hard',modalities:['text'],
 complexity_basis:'Relative curriculum orchestration challenge: eight immutable source roots, two typed local libraries, two source-custody files, eight output projections, 24 exact native operations and separate analysis/output actors. Includes wide-integer and invalid-data cases. No universal difficulty, industry assurance, payments or clinical capability is inferred.',
 initial:'Process eight prepared local operations using our stored scheduling and balance-settlement components. Choose separate analysis and output actors and the minimum capabilities. Preserve every input and deliver all eight named JSON outputs.',
 requirements:'Use every prepared input unchanged. Inputs beginning schedule- use select_slots: each tuple is a half-open start/end/positive-weight request; maximize exact total weight, which may exceed u64, and break ties by the lexicographically smallest increasing vector of original indices. Touching endpoints are compatible; reject the complete request list if any end is not greater than start or weight is zero. Inputs beginning settlement- use settle_balances: negative entries pay positive entries, reject a nonzero aggregate, and otherwise fully settle with the globally minimum number of positive transfers sorted strictly by sender then recipient index. Original balances and requests must stay unchanged. Empty valid inputs return some empty; invalid data returns none. All integers use canonical decimal strings; optional results use {some:value} or {none:true}. Choose one analysis actor for all eight native calls, then a different output actor. For EACH prepared input id, save its complete native /output field to a file named exactly INPUT-ID.json, as compact canonical JSON with sorted object keys and no newline. Thus there must be eight output files. Read every file back and compare it to its actual corresponding native output before completion. Select source owners, artifact writers, native graph and minimum tools yourself. Do not compute answers, supply transfer plans or implement algorithms in planning. The host retains both library sources, derives exact bindings against your chosen source actor and stages source bytes only at approved admission. No manual source writes, code authoring, shell, network, downloads, credentials, memory tools, source promotion or paid services. These are fictional operational and balance cases only; no real payments, banking integration, advice or compliance certification.',
 planningConstraints:{separate_source_writers:true},
 sourceInputs,planningInputs:sourceInputs,ownedComponents,
 files:[scheduling,settlement].map(c=>({path:`owned-${c.id}.rs`,input:c.fn,owned_component:c.id,sha256:c.source})),
 dependencies:Object.fromEntries(specs.map(([id])=>[id,[]])),
 artifacts:specs.map(([input])=>({path:`${input}.json`,input,pointer:'/output',encoding:'json'})),
 expectedResults:new Map(specs.map(([id,c,input])=>[id,{output:c===scheduling?finiteScheduling(input):settlements[id]}]))
};
