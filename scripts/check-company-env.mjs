import { existsSync, readFileSync, statSync } from 'node:fs';
import { resolve } from 'node:path';
import { parseEnv } from 'node:util';
import { fileURLToPath } from 'node:url';

export function auditCompanyEnvironment(local, company) {
  const effective = { ...local, ...company };
  const issues=[];
  if(effective.GENTS_LOCAL_ONLY !== '1') issues.push('Set GENTS_LOCAL_ONLY=1 for the free local company.');
  if(effective.AUTH_MODE && effective.AUTH_MODE !== 'local') issues.push('The default local company requires AUTH_MODE=local.');
  if(effective.BIND && (!/^127\.0\.0\.1:\d{1,5}$/.test(effective.BIND) || Number(effective.BIND.split(':')[1])<1 || Number(effective.BIND.split(':')[1])>65535)) issues.push('Local company binding must use loopback.');
  if(!['stdio','sse'].includes(effective.GENTS_MCP_TRANSPORT || 'stdio')) issues.push('MCP transport must be stdio or sse.');
  if(!['0','1'].includes(effective.GENTS_EMAIL_AUTO_START || '0')) issues.push('Email auto-start must be 0 or 1.');
  for(const key of Object.keys(effective)) if(/^(VITE_|NEXT_PUBLIC_)/.test(key) && /(KEY|TOKEN|SECRET|PASSWORD)/.test(key)) issues.push(`Client-exposed credential setting is forbidden: ${key}.`);
  const duplicateCredentials=Object.keys(company).filter(k=>/(KEY|TOKEN|SECRET|PASSWORD)$/.test(k) && k in local);
  return { issues,required_api_keys:[],local_inference_key_required:false,static_site_key_required:false,
    local_only:effective.GENTS_LOCAL_ONLY==='1',transport:effective.GENTS_MCP_TRANSPORT||'stdio',
    email_auto_start:effective.GENTS_EMAIL_AUTO_START==='1',duplicate_credential_names:duplicateCredentials,
    optional_credentials:Object.fromEntries(['HF_TOKEN','RESEND_API_KEY','GENTS_EMAIL_PASSWORD'].map(k=>[k,{present:!!effective[k],required_for_local_tasks:false}])),
    scope:'Presence/configuration only. No credential values, network validation, provider calls or task execution.' };
}

if(process.argv[1] && resolve(process.argv[1])===fileURLToPath(import.meta.url)) {
  const root=resolve(fileURLToPath(new URL('..',import.meta.url)));
  const read=name=>existsSync(resolve(root,name))?parseEnv(readFileSync(resolve(root,name),'utf8')):{};
  const report=auditCompanyEnvironment(read('.env.local'),read('.env.company'));
  if(existsSync(resolve(root,'.env.compny')))report.issues.push('Misspelled .env.compny is not loaded; use canonical .env.company.');
  if(process.platform!=='win32')for(const name of ['.env.local','.env.company'])if(existsSync(resolve(root,name)) && (statSync(resolve(root,name)).mode & 0o077)) report.issues.push(`${name} must be private to its owner (chmod 600).`);
  report.status=report.issues.length?'REVIEW_REQUIRED':'PASS';
  console.log(JSON.stringify(report,null,2));
  if(report.issues.length)process.exitCode=1;
}
