// Fixed synthetic inputs and independent answer oracle; no implementation or
// expected answer is supplied to the CEO/worker. Source comes from sealed custody.
const component='f995cd27fa8782a9ee59fb84286e997ddfb01f39fd3a4284d0d40f769fdcf14c';
const source='8a3ec5c2ca9ed65da60be122e0a1f740f2b739e3f98f97dbe8a4a4b7bc96e8a7';
const api='337a6eec5e59505b1527dcd712998c03f2357ae7697c0b29089a4c40d075f487';
const valid=[['0','3','5'],['3','6','5'],['0','6','9'],['6','8','2'],['8','9','1'],['6','9','3']];
const sourceInputs=[{id:'valid-case',tool:'owned_rust_call',arguments:{binding_id:'weighted-valid',arguments:[valid]}},
 {id:'invalid-case',tool:'owned_rust_call',arguments:{binding_id:'weighted-invalid',arguments:[[['2','2','5']]]}}];
const ownedComponents=sourceInputs.map(input=>({input:input.id,source_path:'weighted-library.rs',source_sha256:source,api_sha256:api,function:'select_slots',owner_declared_review_package_id:component}));
export default {
 id:'owned-weighted-reuse',title:'Stored local scheduling component: governed two-role reuse',outcomeOnly:true,
 industry:'synthetic_general_operations',complexity:'medium',modalities:['text'],
 complexity_basis:'Two frozen owned-library calls, a valid nontrivial optimum/tie case and invalid-data case, followed by separately scoped output publication. This is reuse, not new algorithm authoring or a universal difficulty score.',
 initial:'Use our stored locally authored scheduling component on the supplied inputs and save both results locally. Choose separate analysis and output actors and the minimum capabilities.',
 requirements:'Use the two fixed input entries unchanged. The owner-bound weighted-valid and weighted-invalid invocations use the stored select_slots function: each request is a half-open start/end/positive-weight tuple; choose compatible requests maximizing total weight, break equal-weight ties by the lexicographically smallest increasing vector of original indices, and return none for invalid requests. The native JSON ABI represents integers as decimal strings and optional results as {some:value} or {none:true}. Do not calculate answers in the plan or reimplement the component. One analysis actor must obtain both native results; choose a different output actor to save the complete output field of valid-case to selected.json and invalid-case to validation.json, as compact canonical JSON with sorted object keys and no newline. Read both files back and verify them against actual native results before completion. Choose the native graph, source owners, output writers and minimum capabilities yourself. The owner will freeze the source/API/function/input identities against your chosen analysis actor before approval. The library source is an immutable owner input, not an agent deliverable. All work stays local: no code authoring, shell, network, downloads, credentials, memory tools or promotion.',
 planningConstraints:{separate_source_writers:true},
 files:[{path:'weighted-library.rs',input:'owned-library',owned_component:component,sha256:source}],
 sourceInputs,planningInputs:sourceInputs,ownedComponents,
 dependencies:{'valid-case':[],'invalid-case':[]},
 artifacts:[{path:'selected.json',input:'valid-case',pointer:'/output',encoding:'json'},{path:'validation.json',input:'invalid-case',pointer:'/output',encoding:'json'}],
 expectedResults:new Map([['valid-case',{output:{some:['0','1','3','4']}}],['invalid-case',{output:{none:true}}]])
};
