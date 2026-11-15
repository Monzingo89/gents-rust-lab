// Private acceptance for repeated-function discovery; no supplied runtime graph.
import previous from './mcp-owned-ceo-selection.mjs';
import {finiteScheduling} from './mcp-owned-mixed-batch.mjs';
import {allocateOracle} from './mcp-owned-allocation-chain.mjs';
const data={
 'north-requests':[['0','2','5'],['2','4','5'],['0','4','10'],['4','7','8'],['7','10','6'],['4','10','14'],['10','12','4'],['0','12','27']],
 'south-requests':[['0','3','6'],['3','6','7'],['0','6','13'],['6','9','4'],['9','11','8'],['6','11','12'],['11','14','9'],['0','14','33']],
 'north-budget':'1013','south-budget':'2'
};
const roles=[],expectedResults=new Map();
for(const branch of ['north','south']){
 const selected=finiteScheduling(data[branch+'-requests']),weights={some:selected.some.map(i=>String(BigInt(i)+1n))};
 roles.push(
  {role:branch+'-schedule',function:'select_slots',arguments:[{kind:'data',data:branch+'-requests',pointer:''}]},
  {role:branch+'-weights',function:'index_weights',arguments:[{kind:'result',source_role:branch+'-schedule',pointer:'/some'}]},
  {role:branch+'-allocation',function:'allocate_cents',arguments:[{kind:'data',data:branch+'-budget',pointer:''},{kind:'result',source_role:branch+'-weights',pointer:'/some'}]}
 );
 expectedResults.set(branch+'-schedule',{output:selected});expectedResults.set(branch+'-weights',{output:weights});expectedResults.set(branch+'-allocation',{output:allocateOracle(data[branch+'-budget'],weights.some)});
}
export default {
 ...previous,id:'owned-ceo-branches',title:'CEO discovers repeated functions for two independent branches',complexity:'hard',
 complexity_basis:'Relative to the qualified three-call discovery: six model-selected calls, four typed edges, two independent chains reusing three interfaces, two final publications and a scarce-budget zero-allocation boundary. No supplied invocation graph. Not universal task mastery or industry assurance.',
 initial:'Use our owned libraries and named data to prepare separate fair distribution reports for the north and south scheduling requests. Ask me to confirm outputs and done criteria, then propose the local calls yourself.',
 requirements:'Process north and south independently. In each branch, select a nonoverlapping subset of its requests with maximum exact total priority; intervals are half-open and ties choose the lexicographically smallest increasing vector of original indices. Convert that branch\'s selected zero-based indices to positive one-based u64 weights using our owned checked adapter. Allocate only that branch\'s budget by exact weighted quotas and largest fractional remainders, breaking ties by smaller original recipient index. Preserve zero allocations when a budget is smaller than the recipient count. Use actual prior native results within the same branch; never cross-connect branches, merge budgets, drop recipients, invent a vector or use an implicit numeric cast. Choose functions, argument references and unique call IDs from the inspected catalog yourself; no invocation IDs, function selections or prepared graph are supplied. Propose the minimum necessary calls for review. After I adopt the exact calls, choose one computation actor followed by a distinct publication actor with minimum tools. Every operation must succeed with present results. Publish only complete native /output objects of the final allocations to north.json and south.json respectively, compact canonical JSON with sorted object keys and no newline. Read each back and compare against its own native result. No intermediate files, source changes, reimplementation, shell, network, downloads, credentials, memory tools, paid services or source promotion. Source bodies remain in owner custody until approved staging. These fictional inputs are synthetic scheduling and integer arithmetic, with no actual bookings, payments, accounts, clinical advice or compliance certification.',
 selection:{data,roles,adoption_message:previous.selection.adoption_message},
 artifacts:[{path:'north.json',input:'north-allocation',pointer:'/output',encoding:'json'},{path:'south.json',input:'south-allocation',pointer:'/output',encoding:'json'}],
 expectedResults
};
