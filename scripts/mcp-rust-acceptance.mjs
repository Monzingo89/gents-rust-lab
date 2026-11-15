// Private trusted-host preparation. No candidate implementation or model calls.
import assert from 'node:assert/strict';
import {spawnSync} from 'node:child_process';
import {lstatSync,readFileSync} from 'node:fs';
import {createHash} from 'node:crypto';
import {fileURLToPath,pathToFileURL} from 'node:url';
import {bundleExecutable} from './mcp-runtime-bundle.mjs';
const sha=value=>createHash('sha256').update(value).digest('hex');
export function acceptancePreflight(root,fixture,bundle=null){
 if(fixture.kind!=='rust_library')return null;
 const input=Buffer.from(fixture.testsText,'utf8');
 assert(input.length>0&&input.length<=8192,'Frozen acceptance exceeds native source bound');
 const binary=bundle?pathToFileURL(bundleExecutable(root,bundle,'gents-rust-acceptance-check')):new URL(`target/debug/gents-rust-acceptance-check${process.platform==='win32'?'.exe':''}`,root);
 const directories=bundle?[new URL('data/',root),new URL('data/mcp-runtime-bundles/',root),new URL('.',new URL(bundle.manifest,root))]:[new URL('target/',root),new URL('target/debug/',root)];
 for(const url of [...directories,binary]){
  const info=lstatSync(url);assert(!info.isSymbolicLink(),'Acceptance checker symlink rejected');
  if(url.href===binary.href)assert(info.isFile()&&info.size<=512*1024*1024&&(process.platform==='win32'||((info.mode&0o022)===0&&info.uid===process.getuid())),'Acceptance checker must be an owner-controlled bounded ordinary executable');
  else assert(info.isDirectory(),'Acceptance checker directory missing');
 }
 const env=Object.fromEntries(['PATH','LANG','LC_ALL','SYSTEMROOT','WINDIR'].filter(k=>k in process.env).map(k=>[k,process.env[k]]));
 const checked=spawnSync(fileURLToPath(binary),[],{input,encoding:'utf8',timeout:5000,maxBuffer:16384,env,windowsHide:true});
 assert(!checked.error&&checked.signal===null,'Acceptance checker did not produce a bounded terminal response');
 let fact;try{fact=JSON.parse(checked.stdout);}catch{throw Error('Native acceptance checker returned no valid protocol response');}
 assert.equal(fact.source_sha256,sha(input),'Acceptance preflight source identity mismatch');
 assert.equal(fact.parser_policy_sha256,sha(readFileSync(new URL('mcp/src/rust_verification/source_policy.rs',root))),'Rebuild the native checker: parser policy differs from current source');
 assert.equal(checked.status,0,'Owner-frozen acceptance rejected by the native dialect/test-count policy; no model/task failure established');
 assert.equal(fact.passed,true,'Native acceptance parser did not admit this fixture');
 assert.equal(fact.source_bytes,input.length);
 assert.equal(fact.tests_count,fixture.testsCount,'Frozen acceptance test count differs from declaration');
 return fact;
}
