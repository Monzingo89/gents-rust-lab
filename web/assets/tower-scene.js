import {towerFloors,reserveTowerSeats,detailStationRect,interiorPodRect,interiorPoint,floorWalkPose,facadeFloorRect,facadeHeight,facadeTop,crewKey} from './tower-layout.js';
import {renderInterior} from './tower-interiors.js';

const specialImages=new Map();
if(typeof Image!=='undefined')for(const [id,src]of [['codex','./gents/codex-executive.png'],['receptionist','./gents/peacock-receptionist.png']]){const image=new Image();image.src=new URL(src,import.meta.url).href;specialImages.set(id,image);}

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
function exterior(ctx,crew,seats,hoverFloor,time,phoneActive){
 const g={width:1600,height:facadeTop+facadeHeight+105};background(ctx,g);
 const x=260,y=facadeTop,w=900,h=facadeHeight,depth=240,rise=92;
 // Open-front architectural cutaway: retain the frame and side elevation, but
 // omit the near curtain wall so live rooms and working silhouettes stay clear.
 polygon(ctx,[[x-15,y-7],[x+depth-15,y-rise-20],[x+w+depth+15,y-rise-20],[x+w+15,y-7]],'#355362','#63828f');
 polygon(ctx,[[x+w,y],[x+w+depth,y-rise],[x+w+depth,y+h-rise],[x+w,y+h]],'#173b5066','#638391');
 round(ctx,x,y,w,h,0,'#07151b35','#6e9aab');
 for(let i=0;i<towerFloors.length;i++){
  const f=towerFloors[i],r=facadeFloorRect(i),selected=Number(hoverFloor)===f.id;
  const activeCrew=crew.filter(s=>s.busy&&seats.get(crewKey(s))?.floor===f.id),active=activeCrew.length;
  const residents=crew.filter(s=>{const a=seats.get(crewKey(s));return a&&(s.busy?a.floor:a.podFloor)===f.id;}).length;
  round(ctx,x+3,r.y+4,w-6,r.h-10,0,active?`${f.theme}3d`:'#0919235c',active?f.theme:'#355767');
  for(let col=0;col<9;col++){
   const px=x+col*100+5,py=r.y+8;
   const occupied=windowInterior(ctx,px,py,col,activeCrew,time);
   // Structural mullions remain as floor-selection cues; there is no opaque
   // pane over the room. A faint edge glow marks occupied bays.
   round(ctx,px,py,90,r.h-19,0,null,occupied?f.theme:'#456b7e');
   line(ctx,px+3,py+r.h-48,px+87,py+r.h-48,'#bad7df30');
  }
  for(let col=0;col<3;col++){
   const t=col/3,nt=(col+1)/3;
   polygon(ctx,[[x+w+depth*t, r.y-rise*t+6],[x+w+depth*nt,r.y-rise*nt+6],[x+w+depth*nt,r.y+128-rise*nt],[x+w+depth*t,r.y+128-rise*t]],col%2?'#82b4cc20':'#447a9740','#6b91a26b');
  }
  line(ctx,x,r.y+r.h-2,x+w,r.y+r.h-2,'#91b5c680',3);
  if(selected){round(ctx,r.x,r.y,r.w,r.h,0,'#b4eada16','#a0edda');}
  if(active){
   const pulse=.45+Math.sin(time/500+i)*.12;
   round(ctx,x+738,r.y+15,142,24,12,`rgba(20,58,49,${.84+pulse*.1})`,f.theme);
   ctx.fillStyle=f.theme;ctx.beginPath();ctx.arc(x+753,r.y+27,5,0,Math.PI*2);ctx.fill();
   text(ctx,`WORKING NOW · ${active}`,x+765,r.y+31,9,'#effff8','left',700);
  }
  text(ctx,String(f.id).padStart(2,'0'),x-35,r.y+57,29,selected?'#c5fff0':'#a8c9d5','right',300);
  text(ctx,f.name,x-35,r.y+79,10,'#85a4b4','right');
  const lx=1420;line(ctx,x+w+depth+10,r.y+47-rise,lx-10,r.y+47-rise,selected?'#a0edda':'#476c7c');
  text(ctx,f.name,lx,r.y+47-rise,12,selected?'#ccfff1':'#c5dce3');
  text(ctx,active?`${active} active · select floor`:f.id===4?`${residents} resting · select floor`:'Standby · select floor',lx,r.y+66-rise,10,active?'#a2dfc2':'#7e9eae');
  if(f.id===11){const glow=phoneActive ? .92 : .42;round(ctx,x+28,r.y+25,48,36,7,`rgba(196,35,31,${glow})`,'#ff796c');text(ctx,'☎',52+x,r.y+51,22,'#fff4e9','center',750);}
 }
 // Columns make the missing front wall read as an intentional dollhouse cutaway.
 for(let px=x;px<=x+w;px+=100)line(ctx,px,y,px,y+h,'#9fc6ce52',2);
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

function specialActor(ctx,id,x,y,w,h,labelText){
 const image=specialImages.get(id),p=interiorPoint(x,y),left=p.x-w/2,top=p.y-h;
 if(image?.complete&&image.naturalWidth)ctx.drawImage(image,left,top,w,h);
 else{round(ctx,left+w*.2,top,w*.6,h,24,id==='codex'?'#c89c35':'#126f83',id==='codex'?'#ffe19a':'#56d6ce');}
 round(ctx,left-8,p.y+7,w+16,22,5,'#07141dcc','#67828f66');text(ctx,labelText,p.x,p.y+22,10,'#e7f0ef','center',650);
}
function seatedExecutive(ctx,x,y,labelText,{suit='#1a2732',accent='#d0ad70',skin='#9b6b4d'}={}){
 const p=interiorPoint(x,y),headY=p.y-112;
 ctx.fillStyle='#0006';ctx.beginPath();ctx.ellipse(p.x,p.y+2,43,10,0,0,Math.PI*2);ctx.fill();
 // This fixed NPC is independent of the transient worker roster, so leadership
 // remains visibly at its post even while every task model is recharging.
 round(ctx,p.x-35,p.y-116,70,113,24,'#4b3a31','#8e725b');
 round(ctx,p.x-31,p.y-68,26,71,10,suit);round(ctx,p.x+5,p.y-68,26,71,10,suit);
 polygon(ctx,[[p.x-45,p.y-91],[p.x,p.y-65],[p.x+45,p.y-91],[p.x+30,p.y-20],[p.x-30,p.y-20]],suit,accent);
 ctx.fillStyle=skin;ctx.beginPath();ctx.arc(p.x,headY,24,0,Math.PI*2);ctx.fill();
 polygon(ctx,[[p.x-25,headY-7],[p.x-12,headY-27],[p.x+18,headY-25],[p.x+26,headY-7]],'#2a211e');
 polygon(ctx,[[p.x-12,p.y-83],[p.x,p.y-67],[p.x+12,p.y-83],[p.x,p.y-42]],accent);
 round(ctx,p.x-82,p.y+12,164,22,5,'#07141dcc','#67828f66');text(ctx,labelText,p.x,p.y+27,10,'#f1e7d5','center',700);
}
function floorSpecials(ctx,f,time,phoneActive,conferenceActive){
 if(f.id===11){specialActor(ctx,'codex',1180,780,150,280,'LOCAL EXECUTIVE · 30B');const pulse=phoneActive ? .65+Math.sin(time/220)*.25 : .2;round(ctx,1035,636,58,38,7,`rgba(190,35,30,${pulse})`,'#ff6a5e');text(ctx,'☎',1064,664,24,'#fff4e6','center',750);}
 if(f.id===10&&!conferenceActive)seatedExecutive(ctx,780,545,'CEO · ENTERPRISE ASSURANCE',{suit:'#172c31',accent:'#83d2c1',skin:'#8f6048'});
 if(f.id===9)seatedExecutive(ctx,780,565,'CFO · TOKEN STEWARDSHIP',{suit:'#293039',accent:'#d0ad70',skin:'#81563e'});
 if(f.id===3&&conferenceActive)seatedExecutive(ctx,780,465,'CEO · ALL-HANDS',{suit:'#172c31',accent:'#83d2c1',skin:'#8f6048'});
 if(f.id===1)specialActor(ctx,'receptionist',785,612,165,245,'RECEPTION · NPC');
}
function walkingActor(ctx,s,look,frame,r,pose){
 const foot=interiorPoint(pose.x,pose.y),h=155,w=frame?Math.min(150,frame.w*h/frame.h):92,x=foot.x-w/2,y=foot.y-h;
 ctx.fillStyle='#0005';ctx.beginPath();ctx.ellipse(foot.x,foot.y+4,43,8,-.05,0,Math.PI*2);ctx.fill();
 if(frame){
  const split=Math.floor(frame.h*.68),topH=h*split/frame.h,legH=h-topH;
  // Rig the two leg regions around their hips; the untouched upper silhouette
  // keeps the exact supplied face and suit while the sprite genuinely travels.
  for(let side=0;side<2;side++){
   ctx.save();ctx.translate(x+w*(side?.75:.25),y+topH);ctx.rotate((side?-1:1)*pose.stride);
   ctx.drawImage(frame.image,frame.x+side*frame.w/2,frame.y+split,frame.w/2,frame.h-split,-w/4,0,w/2,legH);ctx.restore();
  }
  ctx.drawImage(frame.image,frame.x,frame.y,frame.w,split,x,y,w,topH);
 }else{round(ctx,x,y+50,w,120,5,'#284954');round(ctx,x+15,y,w-30,66,20,look.color);}
 s.hit={x:x-5,y:y-3,w:w+10,h:h+10};s.screen={x:foot.x,y:foot.y,top:y};
 const label=interiorPoint(r.x+r.w/2,r.y+208);
 round(ctx,label.x-126,label.y+10,252,23,5,'#0a1825bb','#73939a44');
 text(ctx,actorName(s,look).slice(0,30),label.x,label.y+26,12,'#dce7e6','center',550);
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
export function renderTower(ctx,{crew,characterFor,spriteFrames,filter='all',hoverGent=null,hoverFloor=null,time=0,preview=false,reducedMotion=false,conferenceActive=false,phoneActive=false}){
 const building=filter==='all',height=building?facadeTop+facadeHeight+105:1000;
 if(ctx.canvas.width!==1600||ctx.canvas.height!==height){ctx.canvas.width=1600;ctx.canvas.height=height;}
 const seats=reserveTowerSeats(crew,{conferenceActive}),stats={active:0,resting:0,unassigned:0},thoughts=[];
 ctx.canvas.dataset&&(ctx.canvas.dataset.view=building?'building':'floor');
 ctx.canvas.floorHits=[];ctx.canvas.gentLayout=[];
 for(const s of crew){s.hit=null;s.sleeping=!s.busy;const a=seats.get(crewKey(s));if(!a||!characterFor(s.id,s.role))stats.unassigned++;else if(s.busy)stats.active++;else stats.resting++;}
 if(building){ctx.canvas.interiorDesign=null;exterior(ctx,crew,seats,hoverFloor,time,phoneActive);return stats;}
 const f=towerFloors.find(f=>String(f.id)===String(filter));if(!f)return stats;
 background(ctx,{width:1600,height});
 renderInterior(ctx,f);
 floorSpecials(ctx,f,time,phoneActive,conferenceActive);
 for(const [index,s]of crew.entries()){
  const look=characterFor(s.id,s.role),a=seats.get(crewKey(s));if(!look||!a)continue;
  const showPreview=preview&&f.id>=3&&a.floor===f.id&&s.id.toLowerCase()!=='ceo';
  const resting=!s.busy&&!conferenceActive&&s.id.toLowerCase()!=='ceo'&&!showPreview;
  if((resting?a.podFloor:a.floor)!==f.id)continue;
  const r=resting?interiorPodRect(a.pod):detailStationRect(a),frame=spriteFrames.get(look.id);
  s.paintOrder=index;s.towerFloor=f.id;s.reservedRect=r;s.characterId=look.id;s.sleeping=resting;s.preview=showPreview&&!s.busy;
  if(resting)detailPod(ctx,s,look,frame,r);
  else{
   const delta=Math.min(100,Math.max(0,time-(s.lastVisualTime??time)));s.lastVisualTime=time;
   const paused=hoverGent===crewKey(s)||reducedMotion;
   s.walkClock=(s.walkClock??index*510)+(paused?0:delta);
   const pose=floorWalkPose(r,s.walkClock,index,reducedMotion);if(f.id===6&&!reducedMotion)pose.y-=22+Math.sin(time/370+index)*18;walkingActor(ctx,s,look,frame,r,pose);
   const anchor=interiorPoint(r.x+r.w/2,r.y+208);
   thoughts.push([f.id===6?{...s,activity:`CODE · ${s.activity||'local model dreaming'}`} : s,{x:anchor.x-80,y:anchor.y-158,w:160},s.preview]);
  }
 }
 // Reserve the strip above each walking lane for a foreground public update.
 for(const [s,r,isPreview]of thoughts)thought(ctx,isPreview?{...s,activity:'Visual preview only'}:s,r);
 ctx.canvas.gentLayout=crew.filter(s=>s.hit).map(s=>({key:crewKey(s),character:s.characterId,floor:s.towerFloor,resting:s.sleeping,preview:!!s.preview,hit:{...s.hit},reservation:s.reservedRect}));
 text(ctx,preview?'VISUAL PREVIEW ONLY / NO MODEL OR TASK EXECUTION':`${stats.active} ACTIVE / ${stats.resting} IN RECHARGE`,128,925,14,preview?'#e2c583':'#8bb0b3');
 text(ctx,'WALKING LANES NEVER CROSS / SELECT A GENT FOR ITS DOSSIER',1474,958,11,'#648594','right');
 return stats;
}
