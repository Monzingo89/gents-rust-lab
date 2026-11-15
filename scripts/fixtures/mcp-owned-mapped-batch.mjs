// Trusted owner-side variation; no domain implementation or answers enter model packets.
import base,{finiteScheduling} from './mcp-owned-mixed-batch.mjs';
const schedule=base.ownedComponents.find(c=>c.function==='select_slots');
const settle=base.ownedComponents.find(c=>c.function==='settle_balances');
const specs=[
 ['case-a',schedule,[['0','3','8'],['3','5','7'],['0','5','15'],['5','8','9'],['8','10','6'],['5','10','15'],['1','2','5'],['2','4','6'],['4','6','7'],['6','7','4'],['7','9','6'],['9','11','7'],['11','13','8'],['10','13','12'],['0','13','40'],['13','15','9']],'dispatch-report.json'],
 ['case-b',settle,['0','12','-30','0','8','10'],'payments-synthetic.json'],
 ['case-c',schedule,[['0','1','18446744073709551615'],['1','2','18446744073709551615'],['0','2','18446744073709551615'],['2','3','18446744073709551615'],['3','4','18446744073709551615'],['2','4','18446744073709551615']],'capacity-report.json'],
 ['case-d',settle,['-9','0','-11','0','25','-5'],'counterparty-report.json'],
 ['case-e',schedule,[['0','2','3'],['2','4','0']],'rejected-capacity.json'],
 ['case-f',settle,['9223372036854775807','0','1','-9223372036854775808'],'wide-balance-report.json'],
 ['case-g',schedule,[['0','2','4'],['2','4','4'],['0','4','8'],['4','6','5'],['6','8','5'],['4','8','10']],'priority-report.json'],
 ['case-h',settle,['0','-4','3','0'],'rejected-balance.json']
];
// These valid settlement cases have one sender or one recipient. Each nonzero
// counterparty forces exactly one edge, so the minimum and amounts are unique.
const expectedSettlement=new Map([
 ['case-b',{some:[['2','1','12'],['2','4','8'],['2','5','10']]}],
 ['case-d',{some:[['0','4','9'],['2','4','11'],['5','4','5']]}],
 ['case-f',{some:[['3','0','9223372036854775807'],['3','2','1']]}],
 ['case-h',{none:true}]
]);
const sourceInputs=specs.map(([id,,input])=>({id,tool:'owned_rust_call',arguments:{binding_id:id,arguments:[input]}}));
const mapping=specs.map(([id,c,,path])=>`${id}: ${c.function}, destination ${path}`).join('; ');
export default {
 ...base,id:'owned-mapped-batch',title:'Fresh mixed inputs with explicit destination mapping',
 initial:'Process eight fresh owner-prepared operations from our stored components. Choose separate analysis and output actors. Ask for the output and acceptance requirements before work.',
 requirements:base.requirements.replace('Inputs beginning schedule- use select_slots:','Prepared inputs whose inspected function is select_slots:').replace('Inputs beginning settlement- use settle_balances:','Prepared inputs whose inspected function is settle_balances:').replace('For EACH prepared input id, save its complete native /output field to a file named exactly INPUT-ID.json,','For EACH prepared input, save its complete native /output field to its exact destination in this mapping: '+mapping+'. Write each file'),
 complexity_basis:'Relative curriculum variation: eight independent roots and 24 native operations, a sixteen-request scheduling case with ties, exact wide integers, zero-weight rejection, balance cases with internal zeros and eight explicitly mapped destinations unrelated to input ids. No general intelligence, real payments or compliance assurance is inferred.',
 sourceInputs,planningInputs:sourceInputs,
 ownedComponents:specs.map(([input,c])=>({...c,input})),
 dependencies:Object.fromEntries(specs.map(([id])=>[id,[]])),
 artifacts:specs.map(([input,,,path])=>({path,input,pointer:'/output',encoding:'json'})),
 expectedResults:new Map(specs.map(([id,c,input])=>[id,{output:c.function==='select_slots'?finiteScheduling(input):expectedSettlement.get(id)}]))
};
