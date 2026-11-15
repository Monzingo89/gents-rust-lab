// Address admission only. HTTP transport and browser execution stay separate.
import { readFileSync } from 'node:fs';
import dns from 'node:dns/promises';
import net from 'node:net';

const config=JSON.parse(readFileSync(new URL('../config/public-network-policy.json',import.meta.url),'utf8'));
if(config.version!==1||config.scheme!=='https'||config.port!==443)throw new Error('Unsupported public network policy');
function ranges(rows,family){
 if(!Array.isArray(rows)||rows.length<1||rows.length>128)throw new Error('Bounded address prefixes required');
 const list=new net.BlockList();
 for(const row of rows){
  const [address,prefix,...extra]=row.split('/'),bits=Number(prefix);
  if(extra.length||net.isIP(address)!==family||!Number.isInteger(bits)||bits<0||bits>(family===4?32:128))throw new Error('Invalid address prefix');
  list.addSubnet(address,bits,family===4?'ipv4':'ipv6');
 }
 return list;
}
const allow6=ranges(config.ipv6_allowed,6),deny4=ranges(config.deny_ipv4,4),deny6=ranges(config.deny_ipv6,6);
export const policyVersion=config.version;
export function publicAddress(address){
 if(typeof address!=='string'||address.includes('%'))return false;
 const family=net.isIP(address);
 return family===4?!deny4.check(address,'ipv4'):family===6&&allow6.check(address,'ipv6')&&!deny6.check(address,'ipv6');
}
export async function publicAddresses(host,lookup=dns.lookup){
 if(typeof host!=='string'||!host||host.includes('%'))throw new Error('Unscoped public hostname required');
 const answers=await lookup(host,{all:true});
 if(!answers.length||answers.length>64||answers.some(a=>net.isIP(a.address)!==a.family||!publicAddress(a.address)))throw new Error('Private or non-native public network blocked');
 return answers;
}
export function publicUrl(value){
 const target=new URL(value);
 if(target.protocol!=='https:'||target.username||target.password||(target.port&&target.port!=='443'))throw new Error('Public HTTPS on port 443 required');
 return target;
}
