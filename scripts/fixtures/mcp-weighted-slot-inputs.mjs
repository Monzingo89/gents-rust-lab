// Only synthetic test inputs, never a scheduling implementation or answers.
const multiplier=6364136223846793005n,increment=1442695040888963407n,initial=11400714819323198485n;
export const enumerationRust=`fn enumeration_requests(seed:u64)->Vec<(u64,u64,u64)>{let mut state=seed.wrapping_add(${initial}u64);let mut requests=Vec::new();for _ in 0..8{state=state.wrapping_mul(${multiplier}u64).wrapping_add(${increment}u64);let start=(state>>32)%16;state=state.wrapping_mul(${multiplier}u64).wrapping_add(${increment}u64);let duration=(state>>32)%6+1;state=state.wrapping_mul(${multiplier}u64).wrapping_add(${increment}u64);let weight=(state>>32)%9+1;requests.push((start,start+duration,weight));}requests}`;
export function enumerationInputs(seed){
 if(!Number.isSafeInteger(seed)||seed<0||seed>=512)throw Error('Reviewed seed outside 0..512');
 let state=BigInt.asUintN(64,BigInt(seed)+initial);
 const next=mod=>{state=BigInt.asUintN(64,state*multiplier+increment);return Number((state>>32n)%BigInt(mod));};
 return Array.from({length:8},()=>{const start=next(16),duration=next(6)+1,weight=next(9)+1;return[start,start+duration,weight];});
}
export function enumerationCoverage(){
 const inputs=Array.from({length:512},(_,seed)=>enumerationInputs(seed));
 return{version:2,iterations:inputs.length,distinct_inputs:new Set(inputs.map(JSON.stringify)).size,requests_per_input:8,subsets_per_input:256,enumerated_subsets:inputs.length*256,scope:'Deterministic visible synthetic test inputs; not held-out evaluation, model training or compliance evidence'};
}
