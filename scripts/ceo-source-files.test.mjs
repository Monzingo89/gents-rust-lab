import test from 'node:test';
import assert from 'node:assert/strict';
import {prepareSourceFiles} from '../web/assets/ceo-source-files.js';
const file=(name,text)=>new File([text],name);
test('selected UTF-8 bytes survive without normalization and get bounded unique IDs',async()=>{
 const text='[ {"name":"café", "n":1} ]\r\n';
 const result=await prepareSourceFiles([file('input.json',text),file('other.json','[]')]);
 assert.equal(result[0].text,text);assert.equal(result[0].path,'input.json');assert.notEqual(result[0].id,result[1].id);
 const bom=await prepareSourceFiles([file('bom.json','\ufeff[]')]);assert.equal(bom[0].text,'\ufeff[]');
});
test('rejects invalid names, duplicate names, oversized and invalid UTF-8 files',async()=>{
 for(const files of [[file('../a.json','[]')],[file('a.json','[]'),file('a.json','[]')],[file('a.json',' '.repeat(8193))],Array.from({length:9},(_,i)=>file(`${i}.json`,'[]')),[new File([new Uint8Array([255])],'bad.json')]])await assert.rejects(prepareSourceFiles(files));
});
