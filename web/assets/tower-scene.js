import {towerFloors,reserveTowerSeats,detailStationRect,photographicStationRect,interiorPodRect,interiorPoint,floorWalkPose,facadeFloorRect,facadeHeight,facadeTop,crewKey} from './tower-layout.js';
import {renderInterior,renderInteriorPreview} from './tower-interiors.js';
import {elevatorPose} from './office-activity.js';
import {paintScreens} from './office-dashboards.js';
import {roomActivity,projectRoomPoint} from './room-activity.js';
const roomClocks=new Map();
function roomClock(floor,time,paused){
 const clock=roomClocks.get(floor)||{value:0,last:time};
 if(!paused)clock.value+=Math.min(100,Math.max(0,time-clock.last));clock.last=time;roomClocks.set(floor,clock);return clock.value;
}

const specialImages=new Map();
if(typeof Image!=='undefined')for(const [id,src]of [['codex',new URL('./gents/codex-executive.png',import.meta.url).href],['receptionist',new URL('./gents/peacock-receptionist.png',import.meta.url).href]]){const image=new Image();image.src=src;specialImages.set(id,image);}

function round(ctx,x,y,w,h,r,fill,stroke){ctx.beginPath();ctx.roundRect(x,y,w,h,r);if(fill){ctx.fillStyle=fill;ctx.fill();}if(stroke){ctx.strokeStyle=stroke;ctx.lineWidth=1;ctx.stroke();}}
function text(ctx,value,x,y,size=12,color='#9db4c2',align='left',weight=500){ctx.font=`${weight} ${size}px ui-sans-serif, system-ui, sans-serif`;ctx.fillStyle=color;ctx.textAlign=align;ctx.fillText(value,x,y);ctx.textAlign='left';}
function line(ctx,x,y,toX,toY,color,width=1){ctx.beginPath();ctx.moveTo(x,y);ctx.lineTo(toX,toY);ctx.strokeStyle=color;ctx.lineWidth=width;ctx.stroke();}
function polygon(ctx,points,fill,stroke){ctx.beginPath();points.forEach(([x,y],i)=>i?ctx.lineTo(x,y):ctx.moveTo(x,y));ctx.closePath();ctx.fillStyle=fill;ctx.fill();if(stroke){ctx.strokeStyle=stroke;ctx.stroke();}}
function background(ctx,g){
 const sky=ctx.createLinearGradient(0,0,g.width,g.height);sky.addColorStop(0,'#06121e');sky.addColorStop(.5,'#102b3b');sky.addColorStop(1,'#071019');ctx.fillStyle=sky;ctx.fillRect(0,0,g.width,g.height);
 for(let i=0;i<26;i++){
  const x=(i*173)%1600,w=30+(i*29)%90,h=80+(i*53)%350,y=g.height-h;
  ctx.fillStyle=i%2?'#0e2635':'#0b202e';ctx.fillRect(x,y,w,h);
  for(let row=y+12;row<g.height;row+=18)for(let col=x+8;col<x+w-4;col+=12)if((col+row+i)%7>3){ctx.fillStyle='#274f5e';ctx.fillRect(col,row,3,5);}
 }
 const haze=ctx.createRadialGradient(800,g.height,40,800,g.height,760);haze.addColorStop(0,'#3a8d981c');haze.addColorStop(1,'#16384b00');ctx.fillStyle=haze;ctx.fillRect(0,0,g.width,g.height);
 for(let y=34;y<g.height;y+=44)line(ctx,18,y,60,y,'#63858f20');
 text(ctx,'THE GENTS',128,41,22,'#f1f7f7','left',650);text(ctx,'TOWER / 01',128,62,10,'#76aaa9');
 text(ctx,'LIVE SYSTEMS  /  LOCAL MINDS',1470,42,11,'#87aaa9','right');
}
function actorName(s,look){const position=s.id.replace('Compliance Security & QA Engineer','QA').replace('Lead Architect Gent','Architect').replace(' Gent','');return `${position}${s.role==='planner'?' · P':s.role==='worker'?' · W':''} / ${look.name.split(' · ')[0]}`;}
function thought(ctx,s,r){
 const words=(s.activity||'Working').split(' '),lines=[''];
 for(const word of words){let i=lines.length-1;if((lines[i]+' '+word).trim().length>20&&lines[i])lines.push(word);else lines[i]=(lines[i]+' '+word).trim();}
 const shown=lines.slice(0,2),h=shown.length*13+11,x=r.x-4,y=r.y-40;
 round(ctx,x,y,r.w+8,h,5,'#173d43','#78cbbc');
 shown.forEach((value,i)=>text(ctx,value,r.x+r.w/2,y+14+i*13,10,'#d7fff4','center',550));
}
function windowInterior(ctx,px,py,col,activeCrew,time){
 const worker=activeCrew[col%Math.max(1,activeCrew.length)],working=!!worker&&col<activeCrew.length;
 // The curtain wall is genuinely translucent: room walls, floor, desks and
 // working silhouettes are painted behind a very light glass/reflection pass.
 const room=ctx.createLinearGradient(px,py,px,py+115);room.addColorStop(0,working?'#244b47':'#142733');room.addColorStop(1,working?'#173730':'#0b1822');
 ctx.fillStyle=room;ctx.fillRect(px+2,py+2,86,111);
 ctx.fillStyle=working?'#d7f4c922':'#a8c5d00c';ctx.fillRect(px+5,py+6,80,46);
 ctx.fillStyle=working?'#a8d8bd':'#405968';ctx.fillRect(px+12,py+79,65,3);ctx.fillRect(px+18,py+82,3,18);ctx.fillRect(px+70,py+82,3,18);
 if(working){
  const pulse=.72+Math.sin(time/430+col)*.18,cx=px+45;
  ctx.fillStyle=`rgba(174,239,202,${pulse})`;ctx.beginPath();ctx.arc(cx,py+55,8,0,Math.PI*2);ctx.fill();
  round(ctx,cx-10,py+63,20,27,7,`rgba(80,151,125,${pulse})`);
  ctx.fillStyle=`rgba(181,246,211,${.18+pulse*.12})`;ctx.fillRect(px+4,py+3,82,106);
 }
 return working;
}
function exterior(ctx,crew,seats,hoverFloor,time,phoneActive,tour=false){
 const g={width:1600,height:facadeTop+facadeHeight+105};background(ctx,g);
 const x=260,y=facadeTop,w=900,h=facadeHeight,depth=240,rise=92;
 // Open-front architectural cutaway: retain the frame and side elevation, but
 // omit the near curtain wall so live rooms and working silhouettes stay clear.
 polygon(ctx,[[x-15,y-7],[x+depth-15,y-rise-20],[x+w+depth+15,y-rise-20],[x+w+15,y-7]],'#355362','#63828f');
 polygon(ctx,[[x+w,y],[x+w+depth,y-rise],[x+w+depth,y+h-rise],[x+w,y+h]],'#173b5066','#638391');
 round(ctx,x,y,w,h,0,'#07151b35','#6e9aab');
 for(let i=0;i<towerFloors.length;i++){
  const f=towerFloors[i],r=facadeFloorRect(i),selected=Number(hoverFloor)===f.id;
  const activeCrew=crew.filter(s=>(tour||s.busy)&&seats.get(crewKey(s))?.floor===f.id),active=activeCrew.length;
  const residents=crew.filter(s=>{const a=seats.get(crewKey(s));return a&&(s.busy?a.floor:a.podFloor)===f.id;}).length;
  round(ctx,x+3,r.y+4,w-6,r.h-10,0,active?`${f.theme}3d`:'#0919235c',active?f.theme:'#355767');
  const furnished=renderInteriorPreview(ctx,f,r);
  for(let col=0;!furnished&&col<9;col++){
   const px=x+col*100+5,py=r.y+8;
   const occupied=windowInterior(ctx,px,py,col,tour?[]:activeCrew,time);
   // Structural mullions remain as floor-selection cues; there is no opaque
   // pane over the room. A faint edge glow marks occupied bays.
   round(ctx,px,py,90,r.h-19,0,null,occupied?f.theme:'#456b7e');
   line(ctx,px+3,py+r.h-48,px+87,py+r.h-48,'#bad7df30');
  }
  for(let col=0;col<3;col++){
   const t=col/3,nt=(col+1)/3;
   polygon(ctx,[[x+w+depth*t, r.y-rise*t+6],[x+w+depth*nt,r.y-rise*nt+6],[x+w+depth*nt,r.y+r.h-10-rise*nt],[x+w+depth*t,r.y+r.h-10-rise*t]],col%2?'#82b4cc20':'#447a9740','#6b91a26b');
  }
  line(ctx,x,r.y+r.h-2,x+w,r.y+r.h-2,'#91b5c680',3);
  if(selected){round(ctx,r.x,r.y,r.w,r.h,0,'#b4eada16','#a0edda');}
  if(active){
   const pulse=.45+Math.sin(time/500+i)*.12;
   round(ctx,x+738,r.y+15,142,24,12,`rgba(20,58,49,${.84+pulse*.1})`,f.theme);
   ctx.fillStyle=f.theme;ctx.beginPath();ctx.arc(x+753,r.y+27,5,0,Math.PI*2);ctx.fill();
   text(ctx,`${tour?'ON THIS FLOOR':'WORKING NOW'} · ${active}`,x+765,r.y+31,9,'#effff8','left',700);
  }
  text(ctx,String(f.id).padStart(2,'0'),x-35,r.y+57,29,selected?'#c5fff0':'#a8c9d5','right',300);
  text(ctx,f.name,x-35,r.y+79,10,'#85a4b4','right');
  const lx=1420;line(ctx,x+w+depth+10,r.y+47-rise,lx-10,r.y+47-rise,selected?'#a0edda':'#476c7c');
  text(ctx,f.name,lx,r.y+47-rise,12,selected?'#ccfff1':'#c5dce3');
  text(ctx,active?`${active} ${tour?'Gents':'active'} · select floor`:f.id===4?`${residents} resting · select floor`:'Standby · select floor',lx,r.y+66-rise,10,active?'#a2dfc2':'#7e9eae');
  if(f.id===11){const glow=phoneActive ? .92 : .42;round(ctx,x+28,r.y+25,48,36,7,`rgba(196,35,31,${glow})`,'#ff796c');text(ctx,'☎',52+x,r.y+51,22,'#fff4e9','center',750);}
 }
 // Columns make the missing front wall read as an intentional dollhouse cutaway.
 for(let px=x;px<=x+w;px+=300)line(ctx,px,y,px,y+h,'#9fc6ce52',2);
 polygon(ctx,[[x-20,y+h],[x+w+20,y+h],[x+w+depth+20,y+h-rise],[x+w+depth+20,y+h-rise+26],[x+w+20,y+h+26],[x-20,y+h+26]],'#263f4d','#647f8a');
 round(ctx,x+280,y+h-68,140,68,2,'#142b38','#6e8d97');line(ctx,x+350,y+h-68,x+350,y+h,'#93adba');
 line(ctx,x-100,y+h+34,x+w+depth+100,y+h+34,'#65879645',2);
 text(ctx,'G E N T S   /   H E A D Q U A R T E R S',x+w/2,y-30,15,'#d1e8eb','center',550);
 text(ctx,'OPEN CUTAWAY  /  LIVE FLOOR ACTIVITY',x+w/2,y-11,10,'#83bdb6','center',650);
 line(ctx,x+w/2+depth*.5,y-rise-20,x+w/2+depth*.5,y-rise-75,'#aac8d1',2);
 text(ctx,'SELECT A WINDOW BAND OR FLOOR BUTTON TO ENTER',800,g.height-35,14,'#a4cdc9','center');
 ctx.canvas.floorHits=towerFloors.map((f,i)=>({id:f.id,...facadeFloorRect(i)}));
 ctx.canvas.facadeMode='open-cutaway';
}

function specialActor(ctx,id,x,y,w,h,labelText,screenSpace=false){
 const image=specialImages.get(id),p=screenSpace?{x,y}:interiorPoint(x,y),left=p.x-w/2,top=p.y-h;
 if(image?.complete&&image.naturalWidth)ctx.drawImage(image,left,top,w,h);
 else{round(ctx,left+w*.2,top,w*.6,h,24,id==='codex'?'#c89c35':'#126f83',id==='codex'?'#ffe19a':'#56d6ce');}
 round(ctx,left-8,p.y+7,w+16,22,5,'#07141dcc','#67828f66');text(ctx,labelText,p.x,p.y+22,10,'#e7f0ef','center',650);
}
function floorSpecials(ctx,f,time,phoneActive,conferenceActive){
 if(f.id===11){const photographic=ctx.canvas.interiorDesign?.rendering==='photoreal-architectural';if(photographic)specialActor(ctx,'codex',1460,700,110,210,'LOCAL EXECUTIVE · 30B',true);else specialActor(ctx,'codex',1180,780,150,280,'LOCAL EXECUTIVE · 30B');const pulse=phoneActive ? .65+Math.sin(time/220)*.25 : .2;
  if(ctx.canvas.interiorDesign?.rendering==='photoreal-architectural'){round(ctx,85,688,116,89,12,`rgba(190,35,30,${pulse*.2})`,phoneActive?'#ff6a5e':'#ff6a5e40');}
  else{round(ctx,1035,636,58,38,7,`rgba(190,35,30,${pulse})`,'#ff6a5e');text(ctx,'☎',1064,664,24,'#fff4e6','center',750);}
 }
 if(f.id===1){
  if(ctx.canvas.interiorDesign?.rendering==='photoreal-architectural'){
   ctx.save();ctx.beginPath();ctx.rect(690,240,220,192);ctx.clip();specialActor(ctx,'receptionist',800,520,165,245,'RECEPTION · NPC',true);ctx.restore();
  }else specialActor(ctx,'receptionist',785,612,165,245,'RECEPTION · NPC');
 }
}
function paintWalker(ctx,look,frame,foot,h,stride){
 const w=frame?Math.min(h*.97,frame.w*h/frame.h):h*.59,x=foot.x-w/2,y=foot.y-h;
 ctx.fillStyle='#0005';ctx.beginPath();ctx.ellipse(foot.x,foot.y+4,43,8,-.05,0,Math.PI*2);ctx.fill();
 if(frame){
  const split=Math.floor(frame.h*.68),topH=h*split/frame.h,legH=h-topH;
  // Rig the two leg regions around their hips; the untouched upper silhouette
  // keeps the exact supplied face and suit while the sprite genuinely travels.
  for(let side=0;side<2;side++){
   ctx.save();ctx.translate(x+w*(side?.75:.25),y+topH);ctx.rotate((side?-1:1)*stride);
   ctx.drawImage(frame.image,frame.x+side*frame.w/2,frame.y+split,frame.w/2,frame.h-split,-w/4,0,w/2,legH);ctx.restore();
  }
  ctx.drawImage(frame.image,frame.x,frame.y,frame.w,split,x,y,w,topH);
 }else{round(ctx,x,y+h*.32,w,h*.68,5,'#284954');round(ctx,x+w*.15,y,w*.7,h*.43,20,look.color);}
 return {hit:{x:x-5,y:y-3,w:w+10,h:h+10},screen:{x:foot.x,y:foot.y,top:y}};
}
function walkingActor(ctx,s,look,frame,r,pose){
 Object.assign(s,paintWalker(ctx,look,frame,r.screenSpace?pose:interiorPoint(pose.x,pose.y),155*(r.scale||1),pose.stride));
 const label=r.screenSpace?{x:r.x+r.w/2,y:r.y+r.h-20}:interiorPoint(r.x+r.w/2,r.y+208);
 round(ctx,label.x-116,label.y+10,232,23,5,'#0a1825bb','#73939a44');
 text(ctx,actorName(s,look).slice(0,30),label.x,label.y+26,12,'#dce7e6','center',550);
}
function facadeWalkers(ctx,crew,seats,characterFor,spriteFrames,time,hoverGent,reducedMotion){
 for(const [floorIndex,f]of towerFloors.entries()){
  const floorCrew=crew.filter(s=>seats.get(crewKey(s))?.floor===f.id),r=facadeFloorRect(floorIndex),lane=(r.w-60)/Math.max(1,floorCrew.length);
  ctx.save();ctx.beginPath();ctx.rect(r.x+3,r.y+4,r.w-6,r.h-10);ctx.clip();
  const clock=roomClock(f.id,time,reducedMotion||floorCrew.some(s=>hoverGent===crewKey(s)));
  for(const [index,s]of floorCrew.entries()){
   const look=characterFor(s.id,s.role);if(!look)continue;
   const delta=Math.min(100,Math.max(0,time-(s.lastVisualTime??time)));s.lastVisualTime=time;
   s.walkClock=(s.walkClock??crew.indexOf(s)*631)+(hoverGent===crewKey(s)||reducedMotion?0:delta);
   const assignment=seats.get(crewKey(s)),pose=roomActivity(f.id,assignment.seat,clock,{occupants:floorCrew.length,reducedMotion}),foot=pose?projectRoomPoint(r,pose):{x:r.x+30+lane*(index+.5),y:r.y+r.h-30};
   if(assignment.state==='sleeping')detailPod(ctx,s,look,spriteFrames.get(look.id),{x:foot.x-68,y:foot.y-110,w:136,h:100,pod:assignment.pod});
   else if(pose)roomActor(ctx,s,look,spriteFrames.get(look.id),{...pose,...foot,clipY:pose.clipY==null?null:projectRoomPoint(r,{x:pose.x,y:pose.clipY,h:pose.h}).y},f.id,clock);
   else Object.assign(s,paintWalker(ctx,look,spriteFrames.get(look.id),foot,110,0));
   s.paintOrder=floorIndex*20+index;s.towerFloor=f.id;s.characterId=look.id;s.sleeping=assignment.state==='sleeping';s.visualState=pose?.state||assignment.state;s.officeActivity=pose?.activity||assignment.activity;s.roomObject=pose?.object||'nap-pod';s.preview=false;s.touring=true;s.reservedRect={x:r.x+30+lane*index,y:r.y,w:lane,h:r.h};
  }
  ctx.restore();
 }
 ctx.canvas.gentLayout=crew.filter(s=>s.hit).map(s=>({key:crewKey(s),character:s.characterId,floor:s.towerFloor,state:s.visualState,object:s.roomObject,resting:s.sleeping,touring:true,preview:false,hit:{...s.hit},reservation:s.reservedRect}));
}
function roomActor(ctx,s,look,frame,pose,floor,time){
 ctx.save();
 if(pose.clipY!==null&&pose.clipY!==undefined){ctx.beginPath();ctx.rect(0,0,1600,pose.clipY);ctx.clip();}
 if(pose.state==='seated'){
  const crop=.73,h=pose.h,w=frame?frame.w*h/(frame.h*crop):h*.58;
  if(frame)ctx.drawImage(frame.image,frame.x,frame.y,frame.w,Math.floor(frame.h*crop),pose.x-w/2,pose.y-h,w,h);
  else round(ctx,pose.x-w/2,pose.y-h,w,h,18,look.color);
  s.hit={x:pose.x-w/2-5,y:pose.y-h,w:w+10,h};s.screen={x:pose.x,y:pose.y,top:pose.y-h};
 }else Object.assign(s,paintWalker(ctx,look,frame,pose,pose.h,pose.stride));
 if(pose.state==='interacting'||pose.state==='seated'&&[7,8].includes(floor)){
  const x=pose.x+pose.h*.12,y=pose.y-pose.h*.24+(Math.sin(time/350)*.6),size=Math.max(10,pose.h*.16);
  if(floor===2){round(ctx,x,y-size,size*.3,size,2,'#a7d7d844','#cdeff2');round(ctx,x+1,y-size*.5,size*.3-2,size*.45,1,'#9b88d8aa');}
  else{round(ctx,x-size/2,y-size*.6,size,size*.65,1,[7,8,9].includes(floor)?'#bda47c':'#102a36','#d1cab3');line(ctx,x,y-size*.6,x,y,'#76694b',.7);}
 }
 ctx.restore();
}
function sittingActor(ctx,s,look,frame,foot,h=145){
 const w=h*.68,x=foot.x-w/2,y=foot.y-h;
 const leather=ctx.createLinearGradient(x,0,x+w,0);leather.addColorStop(0,'#201e1c');leather.addColorStop(.4,'#786255');leather.addColorStop(1,'#292722');
 round(ctx,x-8,y+25,w+16,h-23,12,leather,'#b19b7955');
 for(const dx of [-w*.48,w*.48]){line(ctx,foot.x+dx,foot.y-25,foot.x+dx,foot.y+10,'#9a9e9b',3);line(ctx,foot.x+dx,foot.y+10,foot.x+dx*1.4,foot.y+12,'#454d50',4);}
 if(frame){const crop=Math.floor(frame.h*.73);ctx.drawImage(frame.image,frame.x,frame.y,frame.w,crop,x,y,w,h-10);}
 else{round(ctx,x+10,y,w-20,h-12,15,look.color);}
 round(ctx,x-13,foot.y-40,17,10,4,'#493b31','#8b817355');round(ctx,x+w-4,foot.y-40,17,10,4,'#493b31','#8b817355');
 s.hit={x:x-13,y,w:w+26,h:h+15};s.screen={x:foot.x,y:foot.y,top:y};
}
function elevator(ctx,time,building,reducedMotion,floor){
 if(!building){
  const pose=elevatorPose(time,towerFloors,reducedMotion),x=1452,y=218,w=104,h=282;
  const steel=ctx.createLinearGradient(x,0,x+w,0);steel.addColorStop(0,'#3d4749');steel.addColorStop(.25,'#bcc2bd');steel.addColorStop(.46,'#77837d');steel.addColorStop(.51,'#384544');steel.addColorStop(.8,'#a8b2aa');steel.addColorStop(1,'#3d4845');
  round(ctx,x-8,y-8,w+16,h+18,2,'#182628','#b0b4a5');round(ctx,x,y,w,h,0,steel,'#e0dcc1');
  for(let i=1;i<w;i+=3)line(ctx,x+i,y+2,x+i,y+h-3,i%2?'#d9e1d40c':'#152e2711',.5);
  line(ctx,x+w/2,y,x+w/2,y+h,'#1b2f2d',2);line(ctx,x-5,y+h+6,x+w+6,y+h+6,'#c5b99b',3);
  round(ctx,x+15,y-34,w-30,23,2,'#0a1b20','#8c9c91');text(ctx,`${String(floor).padStart(2,'0')} · ${pose.floor===floor?'HERE':'↑ '+pose.floor}`,x+w/2,y-18,10,'#c8debb','center',650);
  round(ctx,x-20,y+155,11,33,2,'#59665f','#bec3ad');ctx.fillStyle='#c8dfac';ctx.beginPath();ctx.arc(x-14.5,y+166,2,0,Math.PI*2);ctx.fill();
  ctx.canvas.elevator={floor:pose.floor,y,animated:!reducedMotion};return;
 }
 const pose=elevatorPose(time,towerFloors,reducedMotion),x=building?164:1466,y=building?facadeTop:165,w=building?78:92,h=building?facadeHeight:355;
 const steel=ctx.createLinearGradient(x,0,x+w,0);steel.addColorStop(0,'#414b50');steel.addColorStop(.18,'#bdc7c7');steel.addColorStop(.35,'#57666a');steel.addColorStop(.75,'#253339');steel.addColorStop(1,'#a3b2b4');
 round(ctx,x,y,w,h,2,steel,'#b3c4c6');round(ctx,x+9,y+7,w-18,h-14,1,'#112833d9','#9ad2cd55');
 for(const dx of [18,w-18])line(ctx,x+dx,y+8,x+dx,y+h-8,'#b5c5c766',1.5);
 if(building)for(let i=0;i<towerFloors.length;i++){const r=facadeFloorRect(i);line(ctx,x+6,r.y+r.h,x+w-6,r.y+r.h,'#dae7e666',2);}
 const cabinY=building?y+pose.y-62:y+h-155;
 round(ctx,x+13,cabinY,w-26,120,3,'#5b77736b','#d7d3b0');round(ctx,x+18,cabinY+5,w-36,106,1,'#acc2bb19','#b3c6c977');
 line(ctx,x+w/2,cabinY+6,x+w/2,cabinY+110,'#e5e8da88',1);line(ctx,x+19,cabinY+91,x+w-19,cabinY+91,'#d1d5c5',2);
 round(ctx,x+9,y-28,w-18,23,2,'#091a20','#8ca8a8');text(ctx,`${building?'↑':String(floor).padStart(2,'0')} · ${String(pose.floor).padStart(2,'0')}`,x+w/2,y-12,11,'#c5eee0','center',650);
 ctx.canvas.elevator={floor:pose.floor,y:cabinY,animated:!reducedMotion};
}
function detailPod(ctx,s,look,frame,r){
 const cx=r.x+r.w/2;
 round(ctx,r.x+7,r.y+8,r.w,r.h,24,'#020d16aa');
 round(ctx,r.x,r.y,r.w,r.h,24,'#172d3b','#73929e');
 const glass=ctx.createLinearGradient(0,r.y,0,r.y+r.h);glass.addColorStop(0,'#416171');glass.addColorStop(1,'#142b3b');
 round(ctx,r.x+9,r.y+9,r.w-18,r.h-34,28,glass,'#a9c9d188');
 if(frame){
  const headH=Math.floor(frame.h*.57),scale=Math.min((r.h-42)/headH,(r.w-28)/frame.w),w=frame.w*scale,h=headH*scale;
  ctx.save();ctx.beginPath();ctx.roundRect(r.x+15,r.y+12,r.w-30,r.h-38,25);ctx.clip();ctx.drawImage(frame.image,frame.x,frame.y,frame.w,headH,cx-w/2,r.y+15+(r.h-42-h)/2,w,h);ctx.restore();
 }
 line(ctx,r.x+25,r.y+18,r.x+18,r.y+r.h-40,'#d5f3ff66',3);
 round(ctx,r.x+13,r.y+r.h-24,r.w-26,4,3,'#87bdc4');
 text(ctx,String(r.pod+1).padStart(2,'0'),r.x+17,r.y+r.h-7,10,'#91b2c1');text(ctx,'ZZZ',cx,r.y+r.h-7,11,'#c8e5e4','center');
 text(ctx,actorName(s,look).slice(0,26),cx,r.y+r.h+15,10,'#c7d7dd','center',550);
 s.hit={x:r.x,y:r.y,w:r.w,h:r.h};s.screen={x:r.x+r.w/2,y:r.y+r.h,top:r.y};
}
function photographicSleeper(ctx,s,look,frame,pod){
 // Occupy the two foreground photographic capsules rather than putting a
 // cartoon pod/badge in mid-air. Keep the original glass shell visible.
 const right=pod%2===1,cx=right?1411:207,cy=right?630:625,angle=right?.38:-.38;
 ctx.save();ctx.translate(cx,cy);ctx.rotate(angle);ctx.beginPath();ctx.ellipse(0,0,61,84,0,0,Math.PI*2);ctx.clip();
 if(frame){const crop=Math.floor(frame.h*.70);ctx.drawImage(frame.image,frame.x,frame.y,frame.w,crop,-46,-79,92,126);}
 else round(ctx,-30,-60,60,106,24,look.color);
 const glass=ctx.createLinearGradient(-65,-70,65,70);glass.addColorStop(0,'#d4f4ff2b');glass.addColorStop(.45,'#b2d8e600');glass.addColorStop(1,'#52788c26');ctx.fillStyle=glass;ctx.fillRect(-65,-90,130,180);line(ctx,-46,-66,-55,24,'#edffff99',2);ctx.restore();
 round(ctx,cx-85,cy+81,170,33,5,'#10252de8','#8aadb477');text(ctx,`${actorName(s,look).slice(0,22)} · ZZZ`,cx,cy+102,9,'#c9e5de','center',600);
 s.hit={x:cx-72,y:cy-90,w:144,h:205};s.screen={x:cx,y:cy+115,top:cy-90};
}
export function renderTower(ctx,{crew,characterFor,spriteFrames,filter='all',hoverGent=null,hoverFloor=null,time=0,preview=false,tour=false,reducedMotion=false,conferenceActive=false,phoneActive=false,telemetry={}}){
 const building=filter==='all',height=building?facadeTop+facadeHeight+105:1000;
 if(ctx.canvas.width!==1600||ctx.canvas.height!==height){ctx.canvas.width=1600;ctx.canvas.height=height;}
 const seats=reserveTowerSeats(crew,{conferenceActive,tour}),stats={active:0,resting:0,unassigned:0},thoughts=[];
 ctx.canvas.dataset&&(ctx.canvas.dataset.view=building?'building':'floor');
 ctx.canvas.floorHits=[];ctx.canvas.gentLayout=[];ctx.canvas.dashboardHits=[];
 for(const s of crew){s.hit=null;s.sleeping=!s.busy;const a=seats.get(crewKey(s));if(!a||!characterFor(s.id,s.role))stats.unassigned++;else if(s.busy)stats.active++;else stats.resting++;}
 if(building){ctx.canvas.interiorDesign=null;exterior(ctx,crew,seats,hoverFloor,time,phoneActive,tour);
  for(const [i,f]of towerFloors.entries()){const r=facadeFloorRect(i),scale=(r.w-6)/1470,sourceHeight=(r.h-10)/scale,top=Math.max(110,500-sourceHeight/2);ctx.canvas.dashboardHits.push(...paintScreens(ctx,f.id,telemetry,s=>({x:r.x+3+(s.x-45)*scale,y:r.y+4+(s.y-top)*scale,w:s.w*scale,h:s.h*scale})));}
  elevator(ctx,time,true,reducedMotion);if(tour)facadeWalkers(ctx,crew,seats,characterFor,spriteFrames,time,hoverGent,reducedMotion);return stats;}
 const f=towerFloors.find(f=>String(f.id)===String(filter));if(!f)return stats;
 background(ctx,{width:1600,height});
 renderInterior(ctx,f);
 floorSpecials(ctx,f,time,phoneActive,conferenceActive);
 ctx.canvas.dashboardHits=paintScreens(ctx,f.id,telemetry);elevator(ctx,time,false,reducedMotion,f.id);
 const floorCrew=crew.filter(s=>seats.get(crewKey(s))?.floor===f.id),clock=roomClock(f.id,time,reducedMotion||floorCrew.some(s=>hoverGent===crewKey(s)));
 const renderCrew=[...crew.entries()];let paintSequence=0;
 if(tour)renderCrew.sort(([,left],[,right])=>{
  const depth=s=>{const a=seats.get(crewKey(s));return a?.floor===f.id?(roomActivity(f.id,a.seat,clock,{occupants:floorCrew.length,reducedMotion})?.y||630):0;};return depth(left)-depth(right);
 });
 for(const [index,s]of renderCrew){
  const look=characterFor(s.id,s.role),a=seats.get(crewKey(s));if(!look||!a)continue;
  const showPreview=preview&&f.id>=3&&a.floor===f.id&&s.id.toLowerCase()!=='ceo';
  const resting=tour?a.state==='sleeping':!s.busy&&!conferenceActive&&s.id.toLowerCase()!=='ceo'&&!showPreview;
  if((resting?a.podFloor:a.floor)!==f.id)continue;
  const r=resting?interiorPodRect(a.pod):ctx.canvas.interiorDesign?.rendering==='photoreal-architectural'?photographicStationRect(a):detailStationRect(a),frame=spriteFrames.get(look.id);
  s.paintOrder=paintSequence++;s.towerFloor=f.id;s.reservedRect=r;s.characterId=look.id;s.sleeping=resting;s.preview=showPreview&&!s.busy;s.touring=tour;
  s.visualState=tour?a.state:null;s.officeActivity=tour?a.activity:null;
  if(resting){if(tour&&ctx.canvas.interiorDesign?.rendering==='photoreal-architectural')photographicSleeper(ctx,s,look,frame,a.pod);else detailPod(ctx,s,look,frame,r);}
  else if(tour&&ctx.canvas.interiorDesign?.rendering==='photoreal-architectural'){
   const pose=roomActivity(f.id,a.seat,clock,{occupants:floorCrew.length,reducedMotion});
   if(pose){roomActor(ctx,s,look,frame,pose,f.id,clock);s.visualState=pose.state;s.officeActivity=pose.activity;s.roomObject=pose.object;}
  }
  else{
   const delta=Math.min(100,Math.max(0,time-(s.lastVisualTime??time)));s.lastVisualTime=time;
   const paused=hoverGent===crewKey(s)||reducedMotion;
   s.walkClock=(s.walkClock??index*510)+(paused?0:delta);
   const pose=floorWalkPose(r,s.walkClock,index,reducedMotion||tour);if(f.id===6&&!reducedMotion)pose.y-=22+Math.sin(time/370+index)*18;walkingActor(ctx,s,look,frame,r,pose);
   const anchor=r.screenSpace?{x:r.x+r.w/2,y:r.y+r.h-20}:interiorPoint(r.x+r.w/2,r.y+208);
   thoughts.push([tour?{...s,activity:a.activity}:f.id===6?{...s,activity:`CODE · ${s.activity||'local model dreaming'}`} : s,{x:anchor.x-80,y:anchor.y-158,w:160},s.preview]);
  }
 }
 // Reserve the strip above each walking lane for a foreground public update.
 for(const [s,r,isPreview]of thoughts)thought(ctx,isPreview?{...s,activity:'Visual preview only'}:s,r);
 ctx.canvas.gentLayout=crew.filter(s=>s.hit).map(s=>({key:crewKey(s),character:s.characterId,floor:s.towerFloor,state:s.visualState,object:s.roomObject,resting:s.sleeping,touring:tour,preview:!!s.preview,hit:{...s.hit},reservation:s.reservedRect}));
 if(ctx.canvas.interiorDesign?.rendering!=='photoreal-architectural'){
  text(ctx,tour?`${crew.length} GENTS / HEADQUARTERS`:preview?'VISUAL PREVIEW ONLY / NO MODEL OR TASK EXECUTION':`${stats.active} ACTIVE / ${stats.resting} IN RECHARGE`,128,925,14,preview?'#e2c583':'#8bb0b3');
  text(ctx,'WALKING LANES NEVER CROSS / SELECT A GENT FOR ITS DOSSIER',1474,958,11,'#648594','right');
 }
 return stats;
}
