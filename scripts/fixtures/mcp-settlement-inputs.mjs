// Auditable finite visible corpus and independent optimum-count oracle only.
// No settlement plan or candidate implementation is supplied.
export function balancedInputs(seed){
 if(!Number.isInteger(seed)||seed<0||seed>=512)throw Error('Settlement seed outside frozen corpus');
 const values=[];let remaining=seed,sum=0;
 for(let i=0;i<5;i++){const value=remaining%5-2;remaining=Math.floor(remaining/5);values.push(value);sum+=value;}
 values.push(sum===0?0:-sum);return values;
}
export function coverage(){return {visible:true,four_balance_grid_inputs:625,balanced_six_inputs:512,distinct_arrays:1137,oracle:'minimum count from exhaustive zero-sum subset partitions; no transfer plan generated'};}
export const oracleRust=`
fn balanced_inputs(mut seed:usize)->Vec<i64>{let mut values=Vec::new();let mut sum=0i64;for _ in 0..5{let value=(seed%5) as i64-2;seed/=5;values.push(value);sum+=value;}values.push(-sum);values}
fn optimum_count(input:&[i64])->usize{
 let values:Vec<i128>=input.iter().filter(|&&v|v!=0).map(|&v|v as i128).collect();let n=values.len();let size=1usize<<n;
 let mut sums=Vec::new();sums.resize(size,0i128);let mut groups=Vec::new();groups.resize(size,0usize);
 for mask in 1..size{let bit=mask.trailing_zeros() as usize;sums[mask]=sums[mask^(1usize<<bit)]+values[bit];
  if sums[mask]==0{let first=1usize<<bit;let mut subset=mask;while subset!=0{if subset&first!=0&&sums[subset]==0{groups[mask]=groups[mask].max(1+groups[mask^subset]);}subset=(subset-1)&mask;}}
 }
 n-groups[size-1]
}
`;
