// Reviewed synthetic binary inputs; literals independently produced with Python
// wave. Frozen expectations are private verification data, never model prompts.
import easy from './mcp-audio-inspection.mjs';
export const recordings=[
 {name:'session',channels:1,rate:44100,samples:[-32768,0,32767],bytes_base64:'UklGRioAAABXQVZFZm10IBAAAAABAAEARKwAAIhYAQACABAAZGF0YQYAAAAAgAAA/38=',sha256:'fda6a8995067c52e776f98a67c20e205868825f65e2575d5ed67e6d6b07c6d79',statistics:[{channel:0,minimum:-32768,maximum:32767,peak_abs:32768,sample_sum:-1,zero_samples:1,rail_samples:2,adjacent_sign_changes:0}]},
 {name:'silence',channels:2,rate:48000,samples:Array(24).fill(0),bytes_base64:'UklGRlQAAABXQVZFZm10IBAAAAABAAIAgLsAAADuAgAEABAAZGF0YTAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=',sha256:'1f88875c2a377ce07591cba3708c86fdfa8550d947b03a3ce4f4e22f6777c0dc',statistics:[0,1].map(channel=>({channel,minimum:0,maximum:0,peak_abs:0,sample_sum:0,zero_samples:12,rail_samples:0,adjacent_sign_changes:0}))},
 {name:'empty',channels:1,rate:8000,samples:[],bytes_base64:'UklGRiQAAABXQVZFZm10IBAAAAABAAEAQB8AAIA+AAACABAAZGF0YQAAAAA=',sha256:'4f8734c5e13ac599e168cf247a51c1dd0758537ce00bf16d7fed1a3d14d07041',statistics:[{channel:0,minimum:null,maximum:null,peak_abs:0,sample_sum:0,zero_samples:0,rail_samples:0,adjacent_sign_changes:0}]}
];
const files=recordings.map(r=>({path:r.name+'.wav',input:r.name+'-input',bytes_base64:r.bytes_base64,sha256:r.sha256}));
const sourceInputs=files.map(f=>({id:f.input,tool:'audio_inspect',arguments:{path:f.path,expected_sha256:f.sha256}}));
const artifacts=recordings.flatMap(r=>[
 {path:r.name+'_levels.json',input:r.name+'-input',pointer:'/channel_statistics',encoding:'json'},
 {path:r.name+'_duration.json',input:r.name+'-input',pointer:'/duration_seconds',encoding:'json'},
 {path:r.name+'_frames.txt',input:r.name+'-input',pointer:'/frame_count',encoding:'integer_text'}
]);
const measurementScope=easy.expectedResults.get('recording-input').measurement_scope;
const expectedResults=new Map(recordings.map((r,i)=>[files[i].input,{
 source_path:files[i].path,source_sha256:r.sha256,source_bytes:Buffer.from(r.bytes_base64,'base64').length,format:'wav_pcm16_le',channels:r.channels,sample_rate_hz:r.rate,bits_per_sample:16,frame_count:r.samples.length/r.channels,duration_seconds:{numerator:r.samples.length/r.channels,denominator:r.rate},channel_statistics:r.statistics,all_samples_zero:r.samples.every(value=>value===0),provider_cost_usd:0,measurement_scope:measurementScope
}]));
export default {
 id:'audio-batch',outcomeOnly:true,title:'Owned audio batch: separate provenance, empty and silent recordings',industry:'synthetic_media_operations',complexity:'medium',
 complexity_basis:'Relative to the first audio case: three distinct binary sources, different rates/channel counts, empty and silent edge cases, nine separately bound outputs. No universal difficulty or semantic audio competence inferred.',modalities:['audio','text'],modality_basis:easy.modality_basis,
 initial:'Inspect my three pinned local recordings and deliver a separate exact measurement packet for each. Clarify acceptance, then choose an analyst and a different output actor. These are artificial waveforms without speech or personal data.',
 requirements:'Keep session.wav, silence.wav and empty.wav unchanged and inspect each through its own pinned input. Do not merge, substitute or invent any recording. For EACH stem session, silence and empty, deliver three files with no newline: STEM_levels.json is the complete native per-channel measurement array in original channel order, retaining every field and null extrema when present; STEM_duration.json is the exact unreduced native duration object with numerator and denominator; STEM_frames.txt contains only the native decimal frame-count integer. Replace STEM with the actual recording stem in every requested filename. JSON is compact canonical JSON with sorted object keys. This yields exactly nine artifacts. Preserve each recording\'s own measurements and rational duration even when rates differ or there are no frames. Derive all contents from actual native results, not planning calculations; read back every artifact and verify exact equality and source provenance. Choose minimum tools and exactly two distinct ordered assignments: one analyst owns all three source calls, followed by a separate writer owning all output writes/readbacks. The host supplies independent authorization and final QA. Choose input owners, artifact metadata and stage criteria yourself; no transformation graph or output template is supplied. Do not infer speech, identity, perceived loudness, diagnosis or compliance; rail hits do not prove clipping. No playback, network, downloads, credentials, memory tools, shell, code edits or source promotion.',
 files,sourceInputs,planningInputs:sourceInputs,planningConstraints:{separate_source_writers:true},dependencies:Object.fromEntries(files.map(f=>[f.input,[]])),artifacts,expectedResults
};
