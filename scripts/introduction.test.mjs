import test from 'node:test';
import assert from 'node:assert/strict';
import { estimate, duration, ownership } from '../site/src/calculator.mjs';
test('scenario math uses supplied input/generation speeds and separate setup time',()=>{
  assert.deepEqual(estimate({inputTokens:1000,outputTokens:100,prefillRate:100,generationRate:10,overheadSeconds:5}),{seconds:25,providerToolFeesUsd:0});
  assert.equal(duration(90),'1.5 minutes');
  assert.equal(duration(7200),'2.0 hours');
});
test('unknown, negative, zero or non-finite speed never invents a time',()=>{
  for (const value of ['',0,-1,Infinity,NaN,'not measured']) {
    assert.equal(estimate({inputTokens:100,outputTokens:20,prefillRate:value,generationRate:10}),null);
    assert.equal(estimate({inputTokens:100,outputTokens:20,prefillRate:10,generationRate:value}),null);
  }
  assert.equal(estimate({inputTokens:-1,outputTokens:20,prefillRate:10,generationRate:10}),null);
  assert.equal(estimate({inputTokens:10,outputTokens:20,prefillRate:10,generationRate:10,overheadSeconds:-1}),null);
  assert.equal(duration(Infinity),'Unknown');
});

test('ownership scenarios include electricity and never invent a positive payback',()=>{
 const result=ownership({hardwareCost:1000,monthlyApiSpend:50,powerWatts:100,monthlyHours:20,electricityPriceKwh:0.2});
 assert.equal(result.monthlyElectricity,0.4);assert.equal(result.monthlySavings,49.6);assert.ok(Math.abs(result.breakEvenMonths-1000/49.6)<1e-8);
 assert.equal(ownership({hardwareCost:1000,monthlyApiSpend:0,powerWatts:100,monthlyHours:20,electricityPriceKwh:0.2}).breakEvenMonths,null);
 assert.equal(ownership({hardwareCost:1e308,monthlyApiSpend:50,powerWatts:1e308,monthlyHours:744,electricityPriceKwh:1e308}),null);
 assert.equal(ownership({hardwareCost:1e308,monthlyApiSpend:1e-300,powerWatts:0,monthlyHours:0,electricityPriceKwh:0}),null);
 assert.equal(ownership({hardwareCost:'',monthlyApiSpend:50,powerWatts:100,monthlyHours:20,electricityPriceKwh:0.2}),null);
 assert.equal(ownership({hardwareCost:1000,monthlyApiSpend:50,powerWatts:100,monthlyHours:745,electricityPriceKwh:0.2}),null);
});
