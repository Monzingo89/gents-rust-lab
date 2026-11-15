// Reviewed owner fixture bytes only. No model-authored paths or implicit codec.
import {createHash} from 'node:crypto';
const maxBytes=16*1024*1024;
export function fixtureBytes(file){
 if(typeof file.path!=='string'||!/^[A-Za-z0-9][A-Za-z0-9_.-]{0,119}$/.test(file.path))throw Error('Fixture source requires a plain safe filename');
 const text=typeof file.text==='string',binary=typeof file.bytes_base64==='string';
 if(text===binary)throw Error('Fixture source requires exactly one text or binary encoding');
 if(text&&Buffer.byteLength(file.text)>maxBytes||binary&&file.bytes_base64.length>Math.ceil(maxBytes/3)*4)throw Error('Fixture source exceeds byte bound');
 const bytes=text?Buffer.from(file.text,'utf8'):Buffer.from(file.bytes_base64,'base64');
 if(bytes.length>maxBytes||binary&&bytes.toString('base64')!==file.bytes_base64)throw Error('Fixture binary encoding must be bounded canonical base64');
 if(!/^[a-f0-9]{64}$/.test(file.sha256??'')||createHash('sha256').update(bytes).digest('hex')!==file.sha256)throw Error('Fixture complete-file digest mismatch');
 return bytes;
}
// Preserve all earlier text fixture digests exactly; binary sources add an
// explicit encoding field instead of coercing arbitrary bytes through UTF-8.
export function fixtureFileSpec(file){
 if(file.owned_component){
  if(!/^[A-Za-z0-9][A-Za-z0-9_.-]{0,119}$/.test(file.path)||!/^[a-f0-9]{64}$/.test(file.owned_component)||!/^[a-f0-9]{64}$/.test(file.sha256)||file.bytes_base64!==undefined)throw Error('Invalid owned fixture source reference');
  if(file.text!==undefined)fixtureBytes(file);
  return {path:file.path,input:file.input,owned_component:file.owned_component,sha256:file.sha256};
 }
 fixtureBytes(file);return {path:file.path,...(typeof file.text==='string'?{text:file.text}:{bytes_base64:file.bytes_base64}),input:file.input};
}
