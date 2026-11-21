// Canonical bodies remain in Git. Never replace existing links or settings.
import {lstat, mkdir, readFile, realpath, symlink} from 'node:fs/promises';
import {homedir} from 'node:os';
import {dirname, join, relative, resolve} from 'node:path';
import {fileURLToPath} from 'node:url';

const root=resolve(dirname(fileURLToPath(import.meta.url)),'..');
const names=['repo-simplify','harness-routing','context-governance','building-interactions','local-evaluate'];
const args=process.argv.slice(2);
if(args.some(a=>!['--install','--codegpt'].includes(a)))throw new Error('Usage: node scripts/repo-skills.mjs [--install] [--codegpt]');
const install=args.includes('--install');
const codegpt=resolve(process.env.CODEGPT_HOME||join(homedir(),'.codegpt'));
const links=[];
for(const name of names){
 const target=join(root,'.agents/skills',name);
 const body=await readFile(join(target,'SKILL.md'),'utf8');
 if(!body.startsWith(`---\nname: ${name}\n`)||!/^description: .+$/m.test(body))throw new Error(`Invalid discovery metadata: ${name}`);
 links.push({link:join(root,'.claude/skills',name),target});
 if(args.includes('--codegpt'))links.push({link:join(codegpt,'skills',name),target});
}
// Preflight the complete set before creating anything; no settings file edits.
for(const {link,target} of links){
 let info;
 try{info=await lstat(link);}catch(error){if(error.code!=='ENOENT')throw error;}
 if(info){
  if(!info.isSymbolicLink()||await realpath(link)!==await realpath(target))throw new Error(`Preserving conflicting existing entry: ${link}`);
 }else if(!install)throw new Error(`Missing discovery link: ${link}`);
}
for(const {link,target} of links){
 if(install){
  await mkdir(dirname(link),{recursive:true});
  try{await symlink(relative(dirname(link),target),link,'dir');}
  catch(error){if(error.code!=='EEXIST')throw error;}
 }
 if(await realpath(link)!==await realpath(target))throw new Error(`Discovery mismatch: ${link}`);
}
console.log(JSON.stringify({canonical:names,claude:'five resolved project links',kimi:'native .agents/skills root; no duplicate adapter',codegpt:args.includes('--codegpt')?'five resolved user catalog links; existing toggles preserved':'not checked; use --codegpt',live_chat_invocation:'not performed; no model calls'}));
