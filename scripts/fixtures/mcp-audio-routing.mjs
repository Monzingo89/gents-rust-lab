// Independent frozen verification data. Only input metadata and outcome
// requirements enter planning; expected results and graph templates stay private.
import {createHash} from 'node:crypto';
import audio from './mcp-audio-inspection.mjs';
const recording=audio.files[0],measurements=audio.expectedResults.get(recording.input);
const rows=[
 {channel:0,category:'capture',active:true},
 {channel:0,category:'capture',active:true},
 {channel:0,category:'disabled',active:false},
 {channel:1,category:'reference',active:true},
 {channel:'1',category:'wrong_type',active:true},
 {channel:1,category:'string_flag',active:'true'}
];
const text=JSON.stringify(rows,null,2)+'\n';
const routing={path:'routing.json',input:'routing-input',text,sha256:createHash('sha256').update(text).digest('hex')};
const columns=[{side:'left',field:'channel',as:'channel'},{side:'left',field:'rail_samples',as:'rail_samples'},{side:'right',field:'category',as:'category'},{side:'right',field:'active',as:'active'}];
const sourceInputs=[audio.sourceInputs[0],{id:routing.input,tool:'table_read_json',arguments:{path:routing.path,expected_sha256:routing.sha256}},
 {id:'mapped-input',tool:'table_join',arguments:{left_rows:[],right_rows:[],left_key:'channel',right_key:'channel',kind:'left',columns},derived_fields:['left_rows','right_rows']},
 {id:'summary-input',tool:'table_aggregate',arguments:{rows:[],filter:{field:'active',equals:true},group_by:'category',value_field:'rail_samples'},derived_fields:['rows']}];
const dependencies={
 [recording.input]:[],[routing.input]:[],
 'mapped-input':[{field:'left_rows',input:recording.input,pointer:'/channel_statistics',path:'channel_rows.json'},{field:'right_rows',input:routing.input,pointer:'/rows',path:'routing_rows.json'}],
 'summary-input':[{field:'rows',input:'mapped-input',pointer:'/rows',path:'mapped.json'}]
};
const expectedResults=new Map([
 [recording.input,measurements],
 [routing.input,{rows,source_path:routing.path,source_sha256:routing.sha256,source_bytes:Buffer.byteLength(text)}],
 ['mapped-input',{rows:[
  {channel:0,rail_samples:2,category:'capture',active:true},
  {channel:0,rail_samples:2,category:'capture',active:true},
  {channel:0,rail_samples:2,category:'disabled',active:false},
  {channel:1,rail_samples:0,category:'reference',active:true},
  {channel:1,rail_samples:0,category:'string_flag',active:'true'}
 ],left_rows:2,right_rows:6,matched_left_rows:2,unmatched_left_rows:0,exact:true}],
 ['summary-input',{input_rows:5,matched_rows:3,excluded_rows:2,groups:[{key:'capture',rows:2,total:4},{key:'reference',rows:1,total:0}],total:4,exact:true}]
]);
export default {
 id:'audio-routing',outcomeOnly:true,title:'Audio-to-table routing reconciliation with typed joins and exact aggregation',industry:'synthetic_media_operations',complexity:'hard',
 complexity_basis:'Relative to the audio batch: binary measurements feed a typed table join and filtered aggregation with duplicate routing entries, a wrong-type key and a string flag. Difficulty is scoped to this workflow family, not universal intelligence.',modalities:['audio','text'],modality_basis:audio.modality_basis,
 initial:'Reconcile my pinned artificial stereo recording with its local channel routing table and produce a source-bound rail-hit report. Clarify acceptance and choose a separate analyst and output actor. There is no speech or personal data.',
 requirements:'Keep recording.wav and routing.json unchanged. Measure each audio channel from the pinned recording; import the pinned JSON routing rows. Match EVERY channel measurement with EVERY routing record having exactly the same typed channel key. Numeric and string keys are distinct. Preserve duplicate matches and original channel/routing order; retain an unmatched audio channel with null category and active. The reconciled rows contain exactly channel, rail_samples, category and active. Include only rows whose active value is the JSON boolean true in the category summary. Group by category, count included joined rows and sum rail_samples, preserving zero-total groups. Deliver SIX files with no newline: channel_rows.json contains the complete native per-channel measurement array; routing_rows.json contains the original routing array; mapped.json contains the reconciled rows; rail_groups.json contains category summaries with key, rows and total sorted by canonical JSON keys; total_rails.txt contains only the included overall rail-sample integer; unmatched.txt contains only the count of original audio channels without a routing match. JSON is compact canonical JSON with sorted object keys. All data must come from actual pinned native results and dependencies, never planning calculations. Read back every output and verify exact equality and provenance. Select minimum owned tools and exactly two distinct ordered assignments: one analyst owns every source and transformation, followed by a separate writer owning all output writes/readbacks. Choose native source ids, dependency metadata, artifact projections and stage criteria yourself; no transformation graph is supplied. Host authorization and independent final QA remain required. Rail hits do not prove clipping. No speech, loudness, identity, diagnosis or compliance inference. No playback, network, downloads, credentials, memory tools, shell, code edits or source promotion.',
 files:[recording,routing],sourceInputs,planningInputs:sourceInputs.filter(input=>!input.derived_fields),planningConstraints:{separate_source_writers:true},dependencies,
 artifacts:[{path:'channel_rows.json',input:recording.input,pointer:'/channel_statistics',encoding:'json'},{path:'routing_rows.json',input:routing.input,pointer:'/rows',encoding:'json'},{path:'mapped.json',input:'mapped-input',pointer:'/rows',encoding:'json'},{path:'rail_groups.json',input:'summary-input',pointer:'/groups',encoding:'json'},{path:'total_rails.txt',input:'summary-input',pointer:'/total',encoding:'integer_text'},{path:'unmatched.txt',input:'mapped-input',pointer:'/unmatched_left_rows',encoding:'integer_text'}],expectedResults
};
