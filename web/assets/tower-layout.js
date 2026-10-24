// Reserved walking lanes: Gents can move without crossing another silhouette.
export const towerFloors=[
 {id:6,name:'EXECUTIVE',rooms:['Strategy & leadership','Planning & all-hands']},
 {id:5,name:'INTELLIGENCE',rooms:['Knowledge & research','Training laboratory']},
 {id:4,name:'ENGINEERING',rooms:['Architecture & production','Quality & security']},
 {id:3,name:'OPERATIONS',rooms:['Systems & operations','Admin review']},
 {id:2,name:'RECHARGE / A',rooms:['Sleep pods · 01—10']},
 {id:1,name:'RECHARGE / B',rooms:['Sleep pods · 11—20']},
];
export const crewKey=s=>`${s.id}:${s.role}`;
export function preferredRoom(s){
 const id=s.id.toLowerCase();
 if(id.includes('admin'))return [3,1];if(id.includes('operations'))return [3,0];
 if(id.includes('training'))return [5,1];if(id.includes('law')||id.includes('research'))return [5,0];
 if(id==='ceo'||id==='cto'||s.role==='router')return [6,0];
 if(id.includes('pm')||id.includes('hr')||id.includes('cfo')||id.includes('accountant'))return [6,1];
 if(id.includes('compliance')||id.includes('qa')||s.role==='judge')return [4,1];
 return [4,0];
}
export function reserveTowerSeats(crew){
 const occupied=new Map(),result=new Map();
 for(const [index,s]of crew.entries()){
  const preferred=preferredRoom(s),rooms=[preferred,...towerFloors.filter(f=>f.id>=3).flatMap(f=>[0,1].map(room=>[f.id,room])).filter(([f,r])=>f!==preferred[0]||r!==preferred[1])];
  const selected=rooms.find(([f,r])=>(occupied.get(`${f}:${r}`)||0)<4);
  if(!selected)continue;
  const roomKey=selected.join(':'),seat=occupied.get(roomKey)||0;occupied.set(roomKey,seat+1);
  result.set(crewKey(s),{floor:selected[0],room:selected[1],seat,pod:index,podFloor:index<10?2:1});
 }
 return result;
}
export function rectsOverlap(a,b){return a.x<b.x+b.w&&a.x+a.w>b.x&&a.y<b.y+b.h&&a.y+a.h>b.y;}
export function detailStationRect(seat){return {x:164+seat.room*660+(seat.seat%2)*310,y:230+Math.floor(seat.seat/2)*310,w:280,h:246};}
export function detailPodRect(index){return {x:163+(index%5)*263,y:223+(Math.floor(index%10/5))*312,w:226,h:226,pod:index};}
export function floorWalkPose(r,time,seed=0,reducedMotion=false){
 const phase=time/2400+seed*.93,offset=reducedMotion?0:Math.sin(phase)*51;
 return {x:r.x+r.w/2+offset,y:r.y+208+(reducedMotion?0:Math.sin(time/115+seed)*1.7),
  stride:reducedMotion?0:Math.sin(time/160+seed)*.13};
}
export const facadeFloorRect=index=>({x:425,y:191+index*134,w:700,h:134});

// Oblique architectural camera: floor depth is visible, sprites remain upright.
// Drawing, movement, labels and hit targets share this exact projection.
export function interiorPoint(x,y,z=0){return{x:96+(x-126)*.91+(y-180)*.16,y:350-(x-126)*.045+(y-180)*.74-z};}
export function interiorPodRect(index){
 const r=detailPodRect(index),p=interiorPoint(r.x+r.w/2,r.y+r.h);
 return{x:p.x-99,y:p.y-181,w:198,h:181,pod:index};
}
export const roomDesigns={
 '6:0':{id:'executive',material:'WALNUT / CALACATTA / CHAMPAGNE BRASS',floor:'#514136',tile:'#705744',wall:'#242b30',accent:'#d9b67a',desk:'#ded7c5',edge:'#7d684e'},
 '6:1':{id:'boardroom',material:'NERO MARQUINA / COGNAC LEATHER',floor:'#29292d',tile:'#44404a',wall:'#2f2930',accent:'#d8aa83',desk:'#343139',edge:'#a27451'},
 '5:0':{id:'library',material:'SMOKED OAK / FOREST VELVET / BRONZE',floor:'#3b3f36',tile:'#5b6050',wall:'#26372f',accent:'#bdc49a',desk:'#7c6650',edge:'#a78b61'},
 '5:1':{id:'training',material:'PORCELAIN / PLATINUM / VIOLET GLASS',floor:'#52616c',tile:'#7b8a95',wall:'#242f44',accent:'#c7bbf5',desk:'#c5ccd2',edge:'#8a88bd'},
 '4:0':{id:'atelier',material:'BASALT / BRUSHED COPPER / SMOKED GLASS',floor:'#303b43',tile:'#485b65',wall:'#263442',accent:'#dea77b',desk:'#4b5862',edge:'#b17d59'},
 '4:1':{id:'quality',material:'PEARL TERRAZZO / SAGE / TITANIUM',floor:'#70817d',tile:'#94a49b',wall:'#304847',accent:'#b7deca',desk:'#cddbd4',edge:'#90ada1'},
 '3:0':{id:'control',material:'BLACK ONYX / TEAL LIGHT / GUNMETAL',floor:'#1c3038',tile:'#34515c',wall:'#172e3a',accent:'#76dacf',desk:'#3a5461',edge:'#639b9e'},
 '3:1':{id:'review',material:'BURGUNDY MARBLE / BRONZE / FLUTED GLASS',floor:'#49373e',tile:'#67505a',wall:'#362b38',accent:'#d8bd8c',desk:'#67494d',edge:'#ab8d60'},
};
