// Explicit synthetic user and independent expected results. Nothing from
// expectedResults/artifact byte oracles is supplied to the model prompts.
import {createHash} from 'node:crypto';
export const canonical=value=>JSON.stringify(value,(_,v)=>v&&typeof v==='object'&&!Array.isArray(v)?Object.fromEntries(Object.keys(v).sort().map(k=>[k,v[k]])):v);
export const project=(value,pointer)=>pointer.slice(1).split('/').reduce((v,k)=>v[k.replaceAll('~1','/').replaceAll('~0','~')],value);
const sales=[{sku:'A',units:8},{sku:'B',units:-2},{sku:'A',units:3},{sku:'missing',units:50},{sku:1,units:7},{sku:true,units:9}];
const catalog=[{sku:'A',category:'hardware',active:true},{sku:'B',category:'returns',active:true},{sku:'A',category:'legacy',active:false},{sku:'1',category:'wrong_type',active:true},{sku:1,category:'numeric',active:true},{sku:true,category:'boolean',active:true}];
export const files=[{path:'sales.json',rows:sales,input:'sales-input'},{path:'catalog.json',rows:catalog,input:'catalog-input'}].map(f=>({...f,text:JSON.stringify(f.rows,null,2)+'\n'})).map(f=>({...f,sha256:createHash('sha256').update(f.text).digest('hex')}));
const columns=[{side:'left',field:'sku',as:'sku'},{side:'left',field:'units',as:'units'},{side:'right',field:'category',as:'category'},{side:'right',field:'active',as:'active'}];
export const sourceInputs=[...files.map(f=>({id:f.input,tool:'table_read_json',arguments:{path:f.path,expected_sha256:f.sha256}})),{id:'join-input',tool:'table_join',arguments:{left_rows:[],right_rows:[],left_key:'sku',right_key:'sku',kind:'left',columns},derived_fields:['left_rows','right_rows']},{id:'summary-input',tool:'table_aggregate',arguments:{rows:[],filter:{field:'active',equals:true},group_by:'category',value_field:'units'},derived_fields:['rows']}];
export const dependencies={
 'sales-input':[], 'catalog-input':[],
 'join-input':[{field:'left_rows',input:'sales-input',pointer:'/rows',path:'sales_rows.json'},{field:'right_rows',input:'catalog-input',pointer:'/rows',path:'catalog_rows.json'}],
 'summary-input':[{field:'rows',input:'join-input',pointer:'/rows',path:'joined.json'}]
};
export const artifacts=[
 {path:'sales_rows.json',input:'sales-input',pointer:'/rows',encoding:'json'},
 {path:'catalog_rows.json',input:'catalog-input',pointer:'/rows',encoding:'json'},
 {path:'joined.json',input:'join-input',pointer:'/rows',encoding:'json'},
 {path:'groups.json',input:'summary-input',pointer:'/groups',encoding:'json'},
 {path:'total.txt',input:'summary-input',pointer:'/total',encoding:'integer_text'},
 {path:'unmatched.txt',input:'join-input',pointer:'/unmatched_left_rows',encoding:'integer_text'}
];
const joined=[
 {sku:'A',units:8,category:'hardware',active:true}, {sku:'A',units:8,category:'legacy',active:false},
 {sku:'B',units:-2,category:'returns',active:true},
 {sku:'A',units:3,category:'hardware',active:true}, {sku:'A',units:3,category:'legacy',active:false},
 {sku:'missing',units:50,category:null,active:null},
 {sku:1,units:7,category:'numeric',active:true}, {sku:true,units:9,category:'boolean',active:true}
];
export const expectedResults=new Map([
 ...files.map(f=>[f.input,{rows:f.rows,source_path:f.path,source_sha256:f.sha256,source_bytes:Buffer.byteLength(f.text)}]),
 ['join-input',{rows:joined,left_rows:6,right_rows:6,matched_left_rows:5,unmatched_left_rows:1,exact:true}],
 ['summary-input',{input_rows:8,matched_rows:5,excluded_rows:3,groups:[{key:'boolean',rows:1,total:9},{key:'hardware',rows:2,total:11},{key:'numeric',rows:1,total:7},{key:'returns',rows:1,total:-2}],total:25,exact:true}]
]);
export const title='Pinned local sales/catalog reconciliation';
export const initial='Reconcile two operator-provided local JSON tables and produce exact report artifacts through independent analyst and writer roles.';
export const requirements=`Use exactly TWO distinct ordered staffing assignments. The analyst owns ALL FOUR native source calls in its single stage: import sales-input and catalog-input using table_read_json with the attached hashes, then join-input using table_join, then summary-input using table_aggregate. It has table_read_json, table_join and table_aggregate only, no general file access. A separate writer has files:write and files:read. Reference the FOUR immutable input ids rather than copying their tools or arguments. On join-input declare argument_sources: left_rows comes from the sales source /rows, right_rows comes from the catalog source /rows. On summary-input, rows comes from the join source /rows. Choose native source ids and use those ids in dependency declarations. The left join uses exact typed sku equality, preserving every left row and every duplicate match; unmatched right columns become null. Numeric, boolean and string keys are distinct. Aggregate only joined rows with active equal to JSON boolean true, group by category and sum signed units. Save SIX exact no-newline files and read back all: sales_rows.json is sales source /rows; catalog_rows.json is catalog source /rows; joined.json is join /rows; groups.json is summary /groups; total.txt is summary /total; unmatched.txt is join /unmatched_left_rows. Arrays use compact canonical JSON with sorted object keys; integers use integer_text. Keep original sales.json and catalog.json unchanged. Preserve duplicate and source row order. Do not calculate answers or expected totals during planning. Choose actors, minimal tools, concise stage criteria and complete definition of done. No shell, network, downloads, memory, extra tests or promotion.`;
