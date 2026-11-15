// Purpose-built vertical campus. Floors are ordered top-to-bottom so the
// exterior cutaway, controls, hit targets and room routing share one source.
import {officeAssignment} from './office-activity.js';
export const towerFloors=[
 {id:11,name:'LOCAL EXECUTIVE',rooms:['Gold command suite'],height:184,theme:'#dfb64f'},
 {id:10,name:'CEO',rooms:['Enterprise oversight office'],height:138,theme:'#78cfc0'},
 {id:9,name:'CFO',rooms:['Token finance office'],height:124,theme:'#c9a66c'},
 {id:8,name:'INTELLIGENCE / ARCHIVE',rooms:['Grand stacks & archives'],height:132,theme:'#aab98a'},
 {id:7,name:'INTELLIGENCE / READING',rooms:['Library reading hall'],height:132,theme:'#bdad7d'},
 {id:6,name:'ENGINEERING',rooms:['Dark code atelier'],height:138,theme:'#8e86ce'},
 {id:5,name:'OPERATIONS',rooms:['Live trading floor'],height:138,theme:'#55b9b4'},
 {id:4,name:'RECHARGE',rooms:['Futuristic nap-pod gallery'],height:144,theme:'#78a8bd'},
 {id:3,name:'CONFERENCE',rooms:['All-hands auditorium'],height:132,theme:'#d2a273'},
 {id:2,name:'TESTING',rooms:['Model testing laboratory'],height:138,theme:'#b9b7e8'},
 {id:1,name:'LOBBY',rooms:['Grand hotel reception'],height:154,theme:'#d5a45c'},
 {id:0,name:'MCP / SERVER BASEMENT',rooms:['Local tool registry & governed execution'],height:154,theme:'#75d5b5'},
];
// A room's elevation now has almost the same aspect as its architectural
// image. The former ~7:1 bands flattened furniture and made rooms unreadable.
for(const floor of towerFloors)floor.height=floor.id===11?540:floor.id===1?480:460;

export const crewKey=s=>`${s.id}:${s.role}`;
export function preferredRoom(s){
 const id=s.id.toLowerCase();
 if(id==='ceo')return [10,0];
 if(id==='cfo'||id.includes('accountant'))return [9,0];
 if(id.includes('compliance')||id.includes('qa')||s.role==='judge'||id.includes('training'))return [2,0];
 if(id.includes('law')||id.includes('research')||id.includes('hr'))return [8,0];
 if(id.includes('pm')||id==='cto'||s.role==='router')return [7,0];
 if(id.includes('admin')||id.includes('operations')||s.role==='tool')return [5,0];
 return [6,0];
}

export function reserveTowerSeats(crew,{conferenceActive=false,tour=false}={}){
 const occupied=new Map(),result=new Map();
 if(tour){
  for(const [index,s]of crew.entries()){
   const assignment=officeAssignment(s,index),floor=assignment.floor,seat=occupied.get(floor)||0;occupied.set(floor,seat+1);
   result.set(crewKey(s),{floor,room:0,seat,pod:assignment.pod??index,podFloor:4,state:assignment.state,activity:assignment.activity});
  }
  return result;
 }
 const workingFloors=[8,7,6,5,2];
 for(const [index,s]of crew.entries()){
  const pinned=s.id.toLowerCase()==='ceo';
  const resting=!pinned&&!s.busy&&!conferenceActive;
  const requested=conferenceActive?[3,0]:preferredRoom(s);
  const choices=[requested,...workingFloors.filter(f=>f!==requested[0]).map(f=>[f,0])];
  const selected=choices.find(([floor])=>(occupied.get(floor)||0)<12)||requested;
  const seat=occupied.get(selected[0])||0;occupied.set(selected[0],seat+1);
  result.set(crewKey(s),{floor:selected[0],room:0,seat,pod:index,podFloor:4});
 }
 return result;
}

export function rectsOverlap(a,b){return a.x<b.x+b.w&&a.x+a.w>b.x&&a.y<b.y+b.h&&a.y+a.h>b.y;}
export function detailStationRect(seat){
 const column=seat.seat%4,row=Math.floor(seat.seat/4);
 return {x:154+column*318,y:220+row*210,w:276,h:200};
}
// Photo backdrops use image-space floor lanes, not the legacy oblique desk
// coordinates. Keep occupants in the foreground rather than on a desktop.
export function photographicStationRect(seat){
 const column=seat.seat%4,row=Math.floor(seat.seat/4);
 let center=240+column*345,foot=865-row*205;
 if([2,5,8,9].includes(seat.floor))foot=([2,5].includes(seat.floor)?930:seat.floor===8?940:925)-row*205;
 if(seat.floor===3&&seat.seat<2){center=490+seat.seat*620;foot=875;}
 if(seat.floor===10&&seat.seat<2){center=245+seat.seat*265;foot=875;}
 if(seat.floor===11&&seat.seat<2){center=590+seat.seat*750;foot=885;}
 return {x:center-122,y:foot-180,w:244,h:200,screenSpace:true,scale:Math.max(.7,1-row*.14)};
}
export function detailPodRect(index){return {x:147+(index%5)*263,y:205+Math.floor(index/5)*160,w:226,h:145,pod:index};}
export function floorWalkPose(r,time,seed=0,reducedMotion=false){
 const phase=time/2400+seed*.93,offset=reducedMotion?0:Math.sin(phase)*42;
 return {x:r.x+r.w/2+offset,y:r.y+r.h-20+(reducedMotion?0:Math.sin(time/115+seed)*1.7),stride:reducedMotion?0:Math.sin(time/160+seed)*.13};
}

export const facadeTop=158;
export const facadeFloorRect=index=>({x:260,y:facadeTop+towerFloors.slice(0,index).reduce((sum,f)=>sum+f.height,0),w:900,h:towerFloors[index]?.height||132});
export const facadeHeight=towerFloors.reduce((sum,f)=>sum+f.height,0);

// Oblique architectural camera: the expanded 1320-unit floor plate gives each
// named room enough width and depth for purpose-built scenery.
export function interiorPoint(x,y,z=0){return{x:72+(x-126)*.96+(y-170)*.22,y:350-(x-126)*.04+(y-170)*.72-z};}
export function interiorPodRect(index){
 const r=detailPodRect(index),p=interiorPoint(r.x+r.w/2,r.y+r.h);
 return{x:p.x-82,y:p.y-108,w:164,h:108,pod:index};
}

export const roomDesigns={
 '0':{id:'mcp',material:'LOCAL RUST / GOVERNED FUNCTIONS / LOOPBACK',floor:'#172c30',tile:'#355054',wall:'#0a1e27',accent:'#75d5b5',desk:'#334c54',edge:'#5b8d88'},
 '11':{id:'codex',material:'GOLD / OBSIDIAN / QUANTUM GLASS',floor:'#604617',tile:'#98742b',wall:'#2b1d0d',accent:'#ffd868',desk:'#d7a936',edge:'#8d6820'},
 '10':{id:'ceo',material:'WALNUT / CALACATTA / LIVE ASSURANCE',floor:'#493c33',tile:'#6b5948',wall:'#182b31',accent:'#87ddca',desk:'#d8cfb9',edge:'#9b7d52'},
 '9':{id:'cfo',material:'SMOKED OAK / BRASS / TOKEN LEDGER',floor:'#403a35',tile:'#5d5248',wall:'#27292b',accent:'#d1ae73',desk:'#9e8565',edge:'#806443'},
 '8':{id:'archive',material:'DOUBLE-HEIGHT STACKS / BRONZE / OAK',floor:'#343a31',tile:'#535a49',wall:'#213229',accent:'#b6c58e',desk:'#79634a',edge:'#987b52'},
 '7':{id:'library',material:'GREEN VELVET / OAK / READING LIGHT',floor:'#3b3f36',tile:'#5b6050',wall:'#26372f',accent:'#d3c38c',desk:'#7c6650',edge:'#a78b61'},
 '6':{id:'engineering',material:'BLACK GLASS / VIOLET CODE / ZERO-G',floor:'#171923',tile:'#292d3e',wall:'#080d18',accent:'#aaa0ff',desk:'#293044',edge:'#655fa0'},
 '5':{id:'operations',material:'NYSE BLUE / MARKET GREEN / STEEL',floor:'#173038',tile:'#284e55',wall:'#0e2630',accent:'#70ddd2',desk:'#31545b',edge:'#5d9d9a'},
 '4':{id:'recharge',material:'QUIET GLASS / VISIBLE INDIVIDUAL PODS',floor:'#273e4a',tile:'#49616a',wall:'#142a37',accent:'#9bc7d7',desk:'#375463',edge:'#496f7d'},
 '3':{id:'conference',material:'CHARCOAL / CINEMA SCREEN / LEATHER',floor:'#312b30',tile:'#50454b',wall:'#201c24',accent:'#e0ad78',desk:'#563e37',edge:'#9b7455'},
 '2':{id:'testing',material:'PORCELAIN / LAB GLASS / VIOLET LIGHT',floor:'#57636c',tile:'#82909a',wall:'#243040',accent:'#d0c5ff',desk:'#c9d0d4',edge:'#8587b2'},
 '1':{id:'lobby',material:'WALNUT / GOLD / GRAND HOTEL MARBLE',floor:'#6b5034',tile:'#9d7a51',wall:'#3c2718',accent:'#efc06a',desk:'#87552d',edge:'#b07b3f'},
};
