// Explicit owner operation: website-only settings, no values in argv or output.
import {readFile} from 'node:fs/promises';
import {parseEnv} from 'node:util';
import {spawnSync} from 'node:child_process';
import {resolve} from 'node:path';
import {fileURLToPath} from 'node:url';

export function websiteSettings(env){
  const names=['RESEND_API_KEY','SUPPORT_SIGNING_SECRET','SUPPORT_ENABLED','SUPPORT_FROM','SUPPORT_TO','SUPPORT_ORIGIN'];
  const settings=Object.fromEntries(names.map(name=>[name,env[name]||'']));
  if(!['0','1'].includes(settings.SUPPORT_ENABLED))throw Error('SUPPORT_ENABLED must be 0 or 1');
  if(settings.SUPPORT_ENABLED==='1'){
    if(!/^re_[A-Za-z0-9_-]+$/.test(settings.RESEND_API_KEY)||settings.SUPPORT_SIGNING_SECRET.length<32)throw Error('Missing or invalid website credentials');
    if(!settings.SUPPORT_FROM||!settings.SUPPORT_TO)throw Error('Missing fixed mail sender or recipient');
  }
  const origins=settings.SUPPORT_ORIGIN.split(',').map(s=>s.trim());
  if(!origins.includes('https://metivolve.com')||origins.some(s=>{try{const u=new URL(s);return u.protocol!=='https:'||u.origin!==s;}catch{return true;}}))throw Error('Expected exact HTTPS origins including metivolve.com');
  for(const value of Object.values(settings))if(/[\r\n\0]/.test(value))throw Error('Multiline website settings are forbidden');
  return settings;
}

async function main(){
  const apply=process.argv.includes('--apply');
  if(process.argv.slice(2).some(arg=>arg!=='--apply'))throw Error('Usage: node scripts/sync-vercel-website-env.mjs [--apply]');
  const root=resolve(import.meta.dirname,'..');
  const settings=websiteSettings(parseEnv(await readFile(resolve(root,'.env.website.local'),'utf8')));
  console.log(JSON.stringify({target:'gents-office',team:'VCV Vercel Team',environment:'production',settings:Object.keys(settings),values:'redacted',apply}));
  if(!apply)return;
  for(const[name,value]of Object.entries(settings)){
    if(!value)continue;
    const secret=['RESEND_API_KEY','SUPPORT_SIGNING_SECRET'].includes(name);
    const result=spawnSync('vercel',['env','add',name,'production','--project','prj_hOje1Hn6D2nUBFJsThaaPLRfOSt3','--scope','team_OW7ea1MdQ4ReouXZAgvJFElL','--force','--yes',secret?'--sensitive':'--no-sensitive'],{input:value,encoding:'utf8',cwd:root,timeout:60000,maxBuffer:1024*1024});
    if(result.error||result.status!==0)throw Error(`Vercel update failed for ${name}; values and provider output suppressed`);
    console.log(`${name}: synced (${secret?'Secret':'Config'})`);
  }
  console.log('Production settings updated. Redeploy the reviewed website artifact to apply them.');
}
if(process.argv[1]&&resolve(process.argv[1])===fileURLToPath(import.meta.url))main().catch(()=>{console.error('Website environment sync failed; check local configuration and Vercel login. No values printed.');process.exitCode=1;});
