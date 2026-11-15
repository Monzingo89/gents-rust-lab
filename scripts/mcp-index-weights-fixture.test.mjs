import test from 'node:test';
import assert from 'node:assert/strict';
import fixture from './fixtures/mcp-rust-index-weights.mjs';
import {acceptancePreflight} from './mcp-rust-acceptance.mjs';
import {fixtureFromId,fixtureDigest} from './fixtures/mcp-workflow-catalog.mjs';
test('checked adapter authoring freezes portable boundary and finite invariant tests without source',()=>{
 assert.equal(fixture.kind,'rust_library');assert.equal(fixture.complexity,'easy');assert.equal(fixture.testsCount,6);
 const fact=acceptancePreflight(new URL('../',import.meta.url),fixture);assert.equal(fact.passed,true);assert.equal(fact.tests_count,6);
 assert(!('sourceText' in fixture));assert(!fixture.initial.includes('pub fn'));assert(!fixture.testsText.includes('pub fn index_weights'));
 assert(fixture.testsText.includes('usize::MAX')&&fixture.testsText.includes('u128')&&fixture.testsText.includes('0..8usize'));
 assert.equal(fixtureDigest(fixtureFromId('owned-branched-allocation')),'e81a2bc35c40c4464c6a797a2fbc2c65352a97d90f9da194e15cf6a29d36fcbb');
});
