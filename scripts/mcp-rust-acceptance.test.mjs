import test from 'node:test';import assert from 'node:assert/strict';
import {spawnSync} from 'node:child_process';import {fileURLToPath} from 'node:url';
import {acceptancePreflight} from './mcp-rust-acceptance.mjs';
import {levels} from './fixtures/mcp-workflow-catalog.mjs';
const root=new URL('../',import.meta.url);
test('all reviewed Rust acceptance fixtures pass the actual native parser before inference',()=>{
 for(const {fixture} of levels.filter(l=>l.fixture.kind==='rust_library')){
  const fact=acceptancePreflight(root,fixture);assert.equal(fact.passed,true,fixture.id);assert.equal(fact.tests_count,fixture.testsCount);
 }
 assert.equal(acceptancePreflight(root,{kind:'other'}),null);
});
test('Rust-compilable forbidden macros and malformed suites fail before task work',()=>{
 for(const testsText of ['#[test]fn check(){let v=std::vec![0;2];std::assert_eq!(v.len(),2);}','fn no_tests(){}','#[test]fn secret_marker(){secret_marker!()}']){
  assert.throws(()=>acceptancePreflight(root,{kind:'rust_library',testsText,testsCount:1}),/Owner-frozen acceptance rejected/);
 }
 assert.throws(()=>acceptancePreflight(root,{kind:'rust_library',testsText:' '.repeat(8193),testsCount:1}),/source bound/);
 assert.throws(()=>acceptancePreflight(root,{kind:'rust_library',testsText:'#[test]fn check(){std::assert!(true);}',testsCount:2}),/test count differs/);
});
test('private parser rejects oversized/non-UTF8 input without echo or execution',()=>{
 const binary=fileURLToPath(new URL(`target/debug/gents-rust-acceptance-check${process.platform==='win32'?'.exe':''}`,root));
 for(const input of [Buffer.alloc(8193,65),Buffer.from([0xff,0xfe]),Buffer.from('')]){
  const r=spawnSync(binary,[],{input,encoding:'utf8',timeout:5000,maxBuffer:16384});assert.equal(r.status,1);const fact=JSON.parse(r.stdout);assert.equal(fact.passed,false);assert(!r.stdout.includes('AAAA'));assert.equal(r.stderr,'');
 }
 const args=spawnSync(binary,['--unexpected'],{input:'#[test]fn check(){std::assert!(true);}',encoding:'utf8',timeout:5000,maxBuffer:16384});assert.equal(args.status,1);assert.equal(JSON.parse(args.stdout).passed,false);
});
