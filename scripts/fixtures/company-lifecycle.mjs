// Synthetic launcher oracle: real processes/HTTP, no models, tools or email.
import {createServer} from 'node:http';
import {appendFileSync,existsSync,writeFileSync} from 'node:fs';
import {spawn} from 'node:child_process';
import {fileURLToPath} from 'node:url';
const {GENTS_FIXTURE_MODE:mode,GENTS_FIXTURE_STATE:state,GENTS_FIXTURE_PORT:port}=process.env;
appendFileSync(state+'.starts','start\n');
if(mode==='exit-before-ready')process.exit(9);
if(mode==='orphan-parent'){
 const child=spawn(process.execPath,[fileURLToPath(import.meta.url)],{
  env:{...process.env,GENTS_FIXTURE_MODE:'stubborn-child'},stdio:'inherit',
 });
 process.on('SIGTERM',()=>process.exit(0));
 child.on('error',()=>process.exit(2));
 setInterval(()=>{},1000);
}else if(mode==='never-ready'){
 writeFileSync(state+'.waiting',String(process.pid));
 process.on('SIGTERM',()=>{});
 setInterval(()=>{},1000);
}else{
 const server=createServer((request,response)=>{
  response.setHeader('Content-Type','application/json');
  if(request.url==='/api/config')response.end(JSON.stringify({auth:'local',local_only:mode!=='cloud-enabled',company_mode:true,company_tools:true,startup_id:mode==='foreign-instance'?'another-instance':process.env.GENTS_STARTUP_ID}));
  else if(request.url==='/api/mcp/catalog')response.end(JSON.stringify({harness_transport:'local_stdio_mcp',runtime_binding:{supported:true,mode:'sealed_bundle',bundle_verified:mode!=='unsealed'}}));
  else{response.statusCode=404;response.end('{}');}
 });
 server.listen(Number(port)||0,'127.0.0.1',()=>{
  writeFileSync(state+'.listener',JSON.stringify({pid:process.pid,port:server.address().port}));
  if(mode==='restart-once'&&!existsSync(state+'.restarted')){
   writeFileSync(state+'.restarted','1');setTimeout(()=>process.exit(75),750);
  }
  if(mode==='crash-after-ready')setTimeout(()=>process.exit(2),750);
 });
 if(mode==='stubborn-child')process.on('SIGTERM',()=>{});
 else process.on('SIGTERM',()=>server.close(()=>process.exit(0)));
}
