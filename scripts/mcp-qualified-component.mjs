// Host-only export of independently qualified local source for owner review.
// No model, candidate execution, global tool registration or promotion.
import assert from 'node:assert/strict';
import {parseArgs} from 'node:util';
import {spawnSync} from 'node:child_process';
import {createHash} from 'node:crypto';
import {fileURLToPath,pathToFileURL} from 'node:url';
import {relative,resolve,sep} from 'node:path';
import {fixtureFromId,fixtureDigest,canonical} from './fixtures/mcp-workflow-catalog.mjs';
import {verifyCodingWorkflow} from './mcp-coding-verifier.mjs';
import {probeBundle,bundleExecutable,verifyBundleBinding} from './mcp-runtime-bundle.mjs';
import {readOwnedEvidence,publishComponentReview,verifyComponentPackage} from './mcp-component-store.mjs';
const sha=bytes=>createHash('sha256').update(bytes).digest('hex');
const uuid=/^[a-f0-9]{8}(?:-[a-f0-9]{4}){3}-[a-f0-9]{12}$/;
const names=['report.json','planning.json','events.json','execution-review.json'];
const profilePath=/^data\/mcp-workflow-check\/[a-f0-9]{8}(?:-[a-f0-9]{4}){3}-[a-f0-9]{12}\/runtime-profile\.json$/;

export async function exportQualifiedComponent(root,reportDirectory){
 assert(/^data\/mcp-workflow-check\/[a-f0-9]{8}(?:-[a-f0-9]{4}){3}-[a-f0-9]{12}$/.test(reportDirectory),'Component export needs one existing workflow report directory');
 const reads=await Promise.allSettled(names.map(name=>readOwnedEvidence(root,reportDirectory+'/'+name)));
 for(const read of reads)if(read.status==='rejected')throw read.reason;
 const evidence=Object.fromEntries(names.map((name,i)=>[name,reads[i].value]));
 const report=JSON.parse(evidence['report.json']),plan=JSON.parse(evidence['planning.json']),saved=JSON.parse(evidence['events.json']),review=JSON.parse(evidence['execution-review.json']);
 assert.equal(report.status,'PASS','Incomplete/failed reports cannot become review packages');
 assert(uuid.test(report.run)&&/^[a-z][a-z0-9-]{0,63}$/.test(report.company),'Invalid source company/run identity');
 const fixture=fixtureFromId(report.fixture_id);
 assert(fixture.kind==='rust_library'&&report.fixture_sha===fixtureDigest(fixture),'Current reviewed Rust fixture required');
 const workspace=`data/companies/${report.company}/runs/${report.run}/workspace/`;
 const artifacts=await Promise.allSettled([readOwnedEvidence(root,workspace+fixture.sourcePath,8192),readOwnedEvidence(root,workspace+fixture.testsPath,8192)]);
 for(const artifact of artifacts)if(artifact.status==='rejected')throw artifact.reason;
 const [source,tests]=artifacts.map(result=>result.value);
 const proof=await verifyCodingWorkflow({root,fixture,plan,saved,review,company:report.company,run:report.run,runtimeProfile:report.planning_runtime_profile??report.runtime_profile});
 assert(proof.status==='PASS'&&proof.source_is_local_model_authored===true&&proof.promotion_authorized===false&&proof.provider_cost_usd===0);
 assert.equal(sha(source),proof.source_sha256,'Source changed during qualification capture');
 assert.equal(sha(tests),proof.tests_sha256,'Acceptance changed during qualification capture');
 const profiles=[report.runtime_profile,report.planning_runtime_profile??report.runtime_profile],profileHashes=[];
 for(const profile of profiles){
  assert(typeof profile?.frozen_path==='string'&&/^[a-f0-9]{64}$/.test(profile.sha256),'Frozen profile identity required');
  const name=relative(fileURLToPath(root),resolve(profile.frozen_path)).split(sep).join('/');assert(profilePath.test(name),'Profile outside workflow evidence');
  assert.equal(sha(await readOwnedEvidence(root,name,65536)),profile.sha256,'Frozen profile changed');profileHashes.push(profile.sha256);
 }
 // This read-only native probe checks the retained bundle, not historic process
 // identity. The original local execution receipts remain separate evidence.
 verifyBundleBinding({runtime_binding:report.runtime_bundle_binding},report.runtime_bundle);
 probeBundle(root,report.runtime_bundle);
 const env=Object.fromEntries(['PATH','LANG','LC_ALL','TMPDIR','TMP','TEMP','SYSTEMROOT','WINDIR'].filter(k=>k in process.env).map(k=>[k,process.env[k]]));
 const archiveResult=spawnSync('python3',[fileURLToPath(new URL('scripts/completed_memory_proof.py',root)),'--company',report.company,'--run',report.run],{cwd:root,env:{...env,GENTS_SQLITE_BRIDGE:bundleExecutable(root,report.runtime_bundle,'gents-sqlite-bridge')},encoding:'utf8',timeout:15000,maxBuffer:65536,windowsHide:true});
 assert(!archiveResult.error&&!archiveResult.signal&&archiveResult.status===0,'Completed archive integrity check failed; no component published');
 const archive=JSON.parse(archiveResult.stdout);
 assert(archive.status==='PASS'&&archive.run===report.run&&archive.contract_sha256===review.digest&&Number.isSafeInteger(archive.policy_revision)&&archive.policy_revision>0,'Completed source contract/archive identity mismatch');
 const unchanged=await Promise.allSettled(names.map(name=>readOwnedEvidence(root,reportDirectory+'/'+name)));
 for(let i=0;i<unchanged.length;i++){const value=unchanged[i];if(value.status==='rejected')throw value.reason;assert(value.value.equals(evidence[names[i]]),'Evidence changed during export');}
 const qualification={schema_version:1,source_report:reportDirectory+'/report.json',evidence_sha256:Object.fromEntries(names.map(name=>[name,sha(evidence[name])])),proof,completed_archive:archive,
  evaluation_host_observed_at_launch:report.evaluation_host??null,
  scope:'Independent unchanged synthetic fixture qualification and completed archive integrity, captured for review. No model quality ranking, training, industry assurance, tool activation or source promotion.'};
 const context={company:report.company,run:report.run,actor:proof.source_lineage.delivered_source_write_witnesses[0].actor,
  fixture_id:fixture.id,fixture_sha256:fixtureDigest(fixture),owner_contract_sha256:review.digest,policy_revision:archive.policy_revision,
  source_sha256:proof.source_sha256,tests_sha256:proof.tests_sha256,execution_profile_sha256:profileHashes[0],planning_profile_sha256:profileHashes[1],native_runtime_bundle_id:report.runtime_bundle.id,
  qualification_sha256:sha(canonical(qualification))};
 const result=await publishComponentReview(root,{'source.rs':source,'acceptance.rs':tests,'qualification.json':canonical(qualification)},context);
 return {component_id:result.id,manifest:result.manifest,status:result.status,source_sha256:proof.source_sha256,tests_sha256:proof.tests_sha256,source_models:[...new Set(proof.source_lineage.delivered_source_write_witnesses.map(w=>w.model))],execution_authorized:false,promotion_authorized:false,
  scope:'Owned content-addressed review package only. Reuse as a callable tool requires a separate reviewed execution adapter and fresh owner-scoped authorization; this file is never tool authority.'};
}

if(process.argv[1]&&import.meta.url===pathToFileURL(process.argv[1]).href){
 const {values}=parseArgs({options:{report:{type:'string'},verify:{type:'string'}}});
 assert.equal([values.report,values.verify].filter(Boolean).length,1,'Choose exactly one --report DIRECTORY or --verify COMPONENT_ID');
 const root=new URL('../',import.meta.url);
 if(values.report)console.log(JSON.stringify(await exportQualifiedComponent(root,values.report)));
 else{const packet=await verifyComponentPackage(root,values.verify);console.log(JSON.stringify({status:'PACKAGE_INTEGRITY_PASS',component_id:packet.id,manifest:packet.manifest,context:packet.context,execution_authorized:false,promotion_authorized:false,scope:'Stored review-package integrity only; no current source qualification, native acceptance rerun, tool activation or promotion.'}));}
}
