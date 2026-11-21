// The direct and supervised entrypoints share configuration and child ownership.
import {fileURLToPath} from 'node:url';
import {loadCompanyRuntimeEnv} from './company-env.mjs';
import {startModelSecurityService} from './model-security-service.mjs';
import {startOwnedProcess} from './company-lifecycle.mjs';
import {createRuntimeBundle,bundleEnvironment,bundleExecutable,probeBundle} from './mcp-runtime-bundle.mjs';

const root=new URL('../',import.meta.url);
loadCompanyRuntimeEnv({root:fileURLToPath(root)});
const bundle=await createRuntimeBundle(root);
probeBundle(root,bundle);
// Under the supervisor all descendants stay in its owned process group. Direct
// launches own their child groups here instead; nested groups would escape a
// supervisor's forced cleanup if this wrapper were interrupted mid-shutdown.
const ownGroup=!(process.env.GENTS_SUPERVISED==='1'&&process.env.GENTS_SUPERVISOR_PID===String(process.ppid));
const child=startOwnedProcess(bundleExecutable(root,bundle,'gents-harness'),[],{
 cwd:root,stdio:'inherit',ownGroup,env:{...process.env,...bundleEnvironment(bundle),
  GENTS_SQLITE_BRIDGE:bundleExecutable(root,bundle,'gents-sqlite-bridge')},
});
let scannerChild;
startModelSecurityService({cwd:root,spawnProcess:(command,args,options)=>{
 scannerChild=startOwnedProcess(command,args,{...options,ownGroup});return scannerChild.child;
}});
const stop=signal=>{for(const handle of [child,scannerChild])void handle.stop(signal).catch(error=>console.error(error.message));};
const interrupt=()=>stop('SIGINT'),terminate=()=>stop('SIGTERM');
process.on('SIGINT',interrupt);process.on('SIGTERM',terminate);
try{
 const result=await child.exited;
 if(result.error)console.error('Could not start the company harness');
 process.exitCode=result.error?1:result.code??(result.signal?1:0);
}finally{
 await Promise.all([child,scannerChild].map(handle=>handle.stop()));
 process.off('SIGINT',interrupt);process.off('SIGTERM',terminate);
}
