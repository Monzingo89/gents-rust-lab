// Independent synthetic benchmark only; retained local Rust source stays unchanged.
import base,{allocateOracle} from './mcp-owned-allocation-chain.mjs';
const definitions=[
 ['a-root','18446744073709551615',null],
 ['b-regional','997','a-root'],
 ['c-regional','18446744073709551615','b-regional'],
 ['d-regional','65537','c-regional'],
 ['e-reserve','101','a-root'],
 ['f-reserve','65521','e-reserve'],
 ['g-reserve','37','f-reserve'],
 ['h-reserve','1000003','g-reserve']
];
const weights=['18446744073709551615','18446744073709551615','0','3','5','7','11','13','17','19','23','29','31','37','41','43'];
const sourceInputs=definitions.map(([id,total,parent])=>({id,tool:'owned_rust_call',arguments:{binding_id:id,arguments:[total,parent?[]:weights]},...(parent?{argument_slots:[1]}:{})}));
const expectedResults=new Map();
for(const [id,total,parent] of definitions)expectedResults.set(id,{output:allocateOracle(total,parent?expectedResults.get(parent).output.some:weights)});
export default {
 ...base,id:'owned-branched-allocation',title:'Shared owned receipt across two deep allocation branches',complexity:'hard',
 complexity_basis:'Relative to the two-node owned chain: eight invocations, seven native positional edges, a shared parent, branch depths four and five, exact u64-boundary arithmetic and two final publications. Same retained library, synthetic text arithmetic only; no multi-library or industry assurance claim.',
 initial:'Use our eight prepared local allocation operations to produce regional and reserve outputs through a shared dependent workflow. Choose separate computation and publication actors. Ask for exact output and acceptance requirements before work.',
 requirements:'Use every prepared invocation unchanged with our stored allocate_cents function. a-root uses its supplied total and weights. b-regional uses the actual returned vector from a-root as weights; c-regional uses the actual returned vector from b-regional; d-regional uses the actual returned vector from c-regional. Independently, e-reserve uses the same actual returned vector from a-root as weights; f-reserve uses the actual returned vector from e-reserve; g-reserve uses the actual returned vector from f-reserve; h-reserve uses the actual returned vector from g-reserve. Keep every supplied total and initial weight unchanged. Choose each compatible native dependency and return projection yourself from the inspected interface; no supplied or calculated replacement vectors. Every call must succeed and return a present vector. Save only the complete native /output of d-regional to regional-allocation.json and the complete native /output of h-reserve to reserve-allocation.json, as compact canonical JSON with sorted object keys and no newline; read both back and compare to their respective native results. Do not publish intermediate files or invoke an operation twice. Choose one computation actor for all operations followed by a distinct publication actor, with minimum tools and acceptance scoped to its own stage. Exact largest-remainder allocation preserves original recipient order and breaks equal remainders by smaller original index; optional values use {some:value}/{none:true} and integers use canonical decimal strings. Choose your own actors, native edges and publication mapping. Source bytes stay in owner custody and are staged only after approval. Do not calculate answers, author or modify code, promote sources, use shell/network/downloads/credentials/memory tools or paid services. These fictional allocations are synthetic arithmetic only, without real accounts, payments, advice or compliance certification.',
 sourceInputs,planningInputs:sourceInputs,
 ownedComponents:sourceInputs.map(i=>({...base.ownedComponents[0],input:i.id})),
 dependencies:Object.fromEntries(definitions.map(([id,,parent])=>[id,parent?[{field:'arguments',argument_index:1,input:parent,pointer:'/output/some'}]:[]])),
 artifacts:[{path:'regional-allocation.json',input:'d-regional',pointer:'/output',encoding:'json'},{path:'reserve-allocation.json',input:'h-reserve',pointer:'/output',encoding:'json'}],expectedResults
};
