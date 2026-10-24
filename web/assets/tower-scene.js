import {towerFloors,reserveTowerSeats,detailStationRect,interiorPodRect,interiorPoint,floorWalkPose,facadeFloorRect,crewKey} from './tower-layout.js';
import {renderInterior} from './tower-interiors.js';

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
function exterior(ctx,crew,seats,hoverFloor){
 const g={width:1600,height:1180};background(ctx,g);
 const x=425,y=191,w=700,h=804,depth=170,rise=82;
 // A continuous curtain-wall facade: roof, concrete core and reflective glazing.
 polygon(ctx,[[x-15,y-7],[x+depth-15,y-rise-20],[x+w+depth+15,y-rise-20],[x+w+15,y-7]],'#355362','#63828f');
 polygon(ctx,[[x+w,y],[x+w+depth,y-rise],[x+w+depth,y+h-rise],[x+w,y+h]],'#173b50','#638391');
 const facade=ctx.createLinearGradient(x,0,x+w,y+h);facade.addColorStop(0,'#204c63');facade.addColorStop(.45,'#31677d');facade.addColorStop(.6,'#193f56');facade.addColorStop(1,'#102c41');
 round(ctx,x,y,w,h,0,facade,'#6e9aab');
 for(let i=0;i<6;i++){
  const f=towerFloors[i],r=facadeFloorRect(i),selected=Number(hoverFloor)===f.id;
  const active=crew.filter(s=>s.busy&&seats.get(crewKey(s))?.floor===f.id).length;
  const residents=crew.filter(s=>{const a=seats.get(crewKey(s));return a&&(s.busy?a.floor:a.podFloor)===f.id;}).length;
  for(let col=0;col<7;col++){
   const px=x+col*100+5,py=r.y+8;
   const pane=ctx.createLinearGradient(px,py,px+91,py+116);pane.addColorStop(0,col%2?'#83b2c530':'#a4d6df25');pane.addColorStop(.55,'#102f4666');pane.addColorStop(1,active&&col%3===0?'#84d6be5c':'#123548aa');
   round(ctx,px,py,90,115,0,pane,'#456b7e');
   line(ctx,px+3,py+79,px+87,py+79,'#bad7df24');line(ctx,px+18,py+5,px+7,py+108,'#c3eaf41a',2);
  }
  for(let col=0;col<3;col++){
   const t=col/3,nt=(col+1)/3;
   polygon(ctx,[[x+w+depth*t, r.y-rise*t+6],[x+w+depth*nt,r.y-rise*nt+6],[x+w+depth*nt,r.y+128-rise*nt],[x+w+depth*t,r.y+128-rise*t]],col%2?'#82b4cc20':'#447a9740','#6b91a26b');
  }
  line(ctx,x,r.y+132,x+w,r.y+132,'#91b5c680',3);
  if(selected){round(ctx,r.x,r.y,r.w,r.h,0,'#b4eada16','#a0edda');}
  text(ctx,String(f.id).padStart(2,'0'),x-35,r.y+57,29,selected?'#c5fff0':'#a8c9d5','right',300);
  text(ctx,f.name,x-35,r.y+79,10,'#85a4b4','right');
  const lx=1340;line(ctx,x+w+depth+10,r.y+47-rise,lx-10,r.y+47-rise,selected?'#a0edda':'#476c7c');
  text(ctx,f.name,lx,r.y+47-rise,12,selected?'#ccfff1':'#c5dce3');
  text(ctx,active?`${active} active · select floor`:f.id<=2?`${residents} resting · select floor`:'Standby · select floor',lx,r.y+66-rise,10,active?'#a2dfc2':'#7e9eae');
 }
 // Broad sky reflections continue across panes, like a real glazed high-rise.
 polygon(ctx,[[x+20,y],[x+250,y],[x+690,y+h],[x+440,y+h]],'#d2f3fc0b');
 for(const px of [x,x+200,x+500,x+w])line(ctx,px,y,px,y+h,'#b4d6df55',2);
 polygon(ctx,[[x-20,y+h],[x+w+20,y+h],[x+w+depth+20,y+h-rise],[x+w+depth+20,y+h-rise+26],[x+w+20,y+h+26],[x-20,y+h+26]],'#263f4d','#647f8a');
 round(ctx,x+280,y+h-68,140,68,2,'#142b38','#6e8d97');line(ctx,x+350,y+h-68,x+350,y+h,'#93adba');
 line(ctx,x-100,y+h+34,x+w+depth+100,y+h+34,'#65879645',2);
 text(ctx,'G E N T S   /   H E A D Q U A R T E R S',x+w/2,y-30,15,'#d1e8eb','center',550);
 line(ctx,x+w/2+depth*.5,y-rise-20,x+w/2+depth*.5,y-rise-75,'#aac8d1',2);
 text(ctx,'SELECT A WINDOW BAND OR FLOOR BUTTON TO ENTER',800,1100,14,'#a4cdc9','center');
 ctx.canvas.floorHits=towerFloors.map((f,i)=>({id:f.id,...facadeFloorRect(i)}));
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
 const glass=ctx.createLinearGradient(0,r.y,0,r.y+135);glass.addColorStop(0,'#416171');glass.addColorStop(1,'#142b3b');
 round(ctx,r.x+9,r.y+9,r.w-18,130,39,glass,'#a9c9d188');
 if(frame){
  const headH=Math.floor(frame.h*.57),scale=Math.min(112/headH,145/frame.w),w=frame.w*scale,h=headH*scale;
  ctx.save();ctx.beginPath();ctx.roundRect(r.x+15,r.y+14,r.w-30,120,34);ctx.clip();ctx.drawImage(frame.image,frame.x,frame.y,frame.w,headH,cx-w/2,r.y+19+(108-h)/2,w,h);ctx.restore();
 }
 line(ctx,r.x+29,r.y+24,r.x+20,r.y+82,'#d5f3ff66',3);
 round(ctx,r.x+13,r.y+141,r.w-26,5,3,'#87bdc4');
 text(ctx,String(r.pod+1).padStart(2,'0'),r.x+19,r.y+168,12,'#91b2c1');text(ctx,'ZZZZ',cx,r.y+168,14,'#c8e5e4','center');
 text(ctx,actorName(s,look).slice(0,26),cx,r.y+r.h+19,12,'#c7d7dd','center',550);
 s.hit={x:r.x,y:r.y,w:r.w,h:r.h};s.screen={x:r.x+r.w/2,y:r.y+r.h,top:r.y};
}
export function renderTower(ctx,{crew,characterFor,spriteFrames,filter='all',hoverGent=null,hoverFloor=null,time=0,preview=false,reducedMotion=false}){
 const building=filter==='all',height=building?1180:1000;
 if(ctx.canvas.width!==1600||ctx.canvas.height!==height){ctx.canvas.width=1600;ctx.canvas.height=height;}
 const seats=reserveTowerSeats(crew),stats={active:0,resting:0,unassigned:0},thoughts=[];
 ctx.canvas.dataset&&(ctx.canvas.dataset.view=building?'building':'floor');
 ctx.canvas.floorHits=[];ctx.canvas.gentLayout=[];
 for(const s of crew){s.hit=null;s.sleeping=!s.busy;const a=seats.get(crewKey(s));if(!a||!characterFor(s.id,s.role))stats.unassigned++;else if(s.busy)stats.active++;else stats.resting++;}
 if(building){ctx.canvas.interiorDesign=null;exterior(ctx,crew,seats,hoverFloor);return stats;}
 const f=towerFloors.find(f=>String(f.id)===String(filter));if(!f)return stats;
 background(ctx,{width:1600,height});
 renderInterior(ctx,f);
 for(const [index,s]of crew.entries()){
  const look=characterFor(s.id,s.role),a=seats.get(crewKey(s));if(!look||!a)continue;
  const showPreview=preview&&f.id>=3&&a.floor===f.id;
  const resting=!s.busy&&!showPreview;
  if((resting?a.podFloor:a.floor)!==f.id)continue;
  const r=resting?interiorPodRect(a.pod):detailStationRect(a),frame=spriteFrames.get(look.id);
  s.paintOrder=index;s.towerFloor=f.id;s.reservedRect=r;s.characterId=look.id;s.sleeping=resting;s.preview=showPreview&&!s.busy;
  if(resting)detailPod(ctx,s,look,frame,r);
  else{
   const delta=Math.min(100,Math.max(0,time-(s.lastVisualTime??time)));s.lastVisualTime=time;
   const paused=hoverGent===crewKey(s)||reducedMotion;
   s.walkClock=(s.walkClock??index*510)+(paused?0:delta);
   const pose=floorWalkPose(r,s.walkClock,index,reducedMotion);walkingActor(ctx,s,look,frame,r,pose);
   const anchor=interiorPoint(r.x+r.w/2,r.y+208);
   thoughts.push([s,{x:anchor.x-80,y:anchor.y-158,w:160},s.preview]);
  }
 }
 // Reserve the strip above each walking lane for a foreground public update.
 for(const [s,r,isPreview]of thoughts)thought(ctx,isPreview?{...s,activity:'Visual preview only'}:s,r);
 ctx.canvas.gentLayout=crew.filter(s=>s.hit).map(s=>({key:crewKey(s),character:s.characterId,floor:s.towerFloor,resting:s.sleeping,preview:!!s.preview,hit:{...s.hit},reservation:s.reservedRect}));
 text(ctx,preview?'VISUAL PREVIEW ONLY / NO MODEL OR TASK EXECUTION':`${stats.active} ACTIVE / ${stats.resting} IN RECHARGE`,128,925,14,preview?'#e2c583':'#8bb0b3');
 text(ctx,'WALKING LANES NEVER CROSS / SELECT A GENT FOR ITS DOSSIER',1474,958,11,'#648594','right');
 return stats;
}
