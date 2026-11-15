import test from 'node:test';import assert from 'node:assert/strict';
import {basicAnswer,basicAnswers} from '../web/assets/basic-answers.js';
test('known setup answers are local and unknown/ambiguous questions are not guessed',()=>{
 for(const [question,answer]of basicAnswers){const result=basicAnswer(question);assert.equal(result.answer,answer);assert.equal(result.modelCalls,0);assert.equal(result.toolCalls,0);}
 assert.match(basicAnswer('IS METIVOLVE FREE?').answer,/MIT/);
 assert.equal(basicAnswer('Can you review my private banking records and certify me?'),null);
 assert.equal(basicAnswer('is it free and certified'),null);assert.equal(basicAnswer('x'.repeat(301)),null);assert.equal(basicAnswer(null),null);
});
