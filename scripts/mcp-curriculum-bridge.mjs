import {spawn} from 'node:child_process';
import {fileURLToPath} from 'node:url';
export async function journal(root,request){
 return new Promise((resolve,reject)=>{
  const child=spawn('python3',[fileURLToPath(new URL('./mcp-curriculum-journal.py',import.meta.url))],{stdio:['pipe','pipe','pipe']});let stdout='',stderr='';
  child.stdout.on('data',chunk=>{stdout+=chunk;if(stdout.length>4*1024*1024)child.kill();});child.stderr.on('data',chunk=>{if(stderr.length<4000)stderr+=chunk;});child.on('error',reject);
  child.on('close',code=>{try{const result=JSON.parse(stdout);if(code!==0||result.error)throw Error(result.error||stderr||'Curriculum journal failed');resolve(result);}catch(error){reject(error);}});
  child.stdin.end(JSON.stringify({...request,root:root instanceof URL?fileURLToPath(root):root}));
 });
}
