// Inspect the exact Git index. Outputs paths/counts, never credential values.
import {execFileSync} from 'node:child_process';
import {readFile,lstat,mkdir,writeFile,rm} from 'node:fs/promises';
import {resolve,dirname} from 'node:path';
import {parseEnv} from 'node:util';
const root=resolve(import.meta.dirname,'..'),out=resolve(root,'data/release-session-1');
const paths=execFileSync('git',['ls-files','-z'],{cwd:root}).toString().split('\0').filter(Boolean);
const secrets=[];
for(const name of ['.env','.env.local','.env.company','.env.website.local']){
 try{for(const[key,value]of Object.entries(parseEnv(await readFile(resolve(root,name),'utf8'))))if(/(KEY|TOKEN|SECRET|PASSWORD)$/.test(key)&&value.length>=8)secrets.push(value);}catch(e){if(e.code!=='ENOENT')throw e;}
}
const snapshot=resolve(out,'indexed-source');await mkdir(out,{recursive:true});await rm(snapshot,{recursive:true,force:true});await mkdir(snapshot);
let bytes=0;
for(const path of paths){
 if(/^(models|data|node_modules|target|\.venv[^/]*|site\/(dist|deploy))\//.test(path)||/(\.gguf|\.safetensors|\.onnx|\.ckpt|\.p8|\.p12|\.pem|\.key|\.sqlite3?)$/i.test(path)||(/(^|\/)\.env($|\.)/.test(path)&&!path.endsWith('.example')))throw Error('Forbidden publication path: '+path);
 const stat=await lstat(resolve(root,path));if(!stat.isFile())throw Error('Non-regular publication path: '+path);
 const data=execFileSync('git',['show',':'+path],{cwd:root,maxBuffer:16*1024*1024});
 if(data.length>10*1024*1024)throw Error('Oversized source artifact: '+path);
 if(secrets.some(secret=>data.includes(Buffer.from(secret))))throw Error('Private local credential match in: '+path);
 const dest=resolve(snapshot,path);await mkdir(dirname(dest),{recursive:true});await writeFile(dest,data);bytes+=data.length;
}
console.log(JSON.stringify({files:paths.length,bytes,local_credentials_checked:secrets.length,secrets_printed:false,model_files_included:false,status:'PASS'}));
// Native scanner checks generic/provider secret patterns, with six reviewed false positives.
try{execFileSync('gitleaks',['dir','.', '--redact','--config',resolve(root,'.gitleaks.toml'),'--report-format','json','--report-path',resolve(out,'indexed-secrets.json')],{cwd:snapshot,stdio:['ignore','ignore','pipe']});}
catch{throw Error('Secret scanner rejected the index; inspect the private redacted report under data/release-session-1');}
console.log('PASS: indexed source secret scan; no known local credentials or detected secret patterns.');
