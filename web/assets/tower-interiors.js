import {interiorPoint as point,detailStationRect,roomDesigns} from './tower-layout.js';

const polygon=(ctx,points,fill,stroke)=>{ctx.beginPath();points.forEach((p,i)=>i?ctx.lineTo(p.x,p.y):ctx.moveTo(p.x,p.y));ctx.closePath();if(fill){ctx.fillStyle=fill;ctx.fill();}if(stroke){ctx.strokeStyle=stroke;ctx.lineWidth=1;ctx.stroke();}};
const plane=(ctx,x,y,w,d,z,fill,stroke)=>polygon(ctx,[point(x,y,z),point(x+w,y,z),point(x+w,y+d,z),point(x,y+d,z)],fill,stroke);
const line=(ctx,a,b,color,width=1)=>{ctx.beginPath();ctx.moveTo(a.x,a.y);ctx.lineTo(b.x,b.y);ctx.strokeStyle=color;ctx.lineWidth=width;ctx.stroke();};
const label=(ctx,value,p,size,color,align='left')=>{ctx.font=`500 ${size}px ui-sans-serif,system-ui`;ctx.fillStyle=color;ctx.textAlign=align;ctx.fillText(value,p.x,p.y);ctx.textAlign='left';};
function roundedPlane(ctx,x,y,w,d,z,radius,fill,stroke){
 ctx.save();ctx.transform(.91,-.045,.16,.74,96-126*.91-180*.16,350+126*.045-180*.74-z);
 ctx.beginPath();ctx.roundRect(x,y,w,d,radius);ctx.fillStyle=fill;ctx.fill();if(stroke){ctx.strokeStyle=stroke;ctx.lineWidth=1;ctx.stroke();}ctx.restore();
}
function box(ctx,x,y,w,d,h,top,front,side,z=0){
 polygon(ctx,[point(x,y+d,z),point(x+w,y+d,z),point(x+w,y+d,z+h),point(x,y+d,z+h)],front);
 polygon(ctx,[point(x+w,y,z),point(x+w,y+d,z),point(x+w,y+d,z+h),point(x+w,y,z+h)],side);
 plane(ctx,x,y,w,d,z+h,top);
}
function wallPanel(ctx,x,w,z,h,fill,stroke){polygon(ctx,[point(x,180,z),point(x+w,180,z),point(x+w,180,z+h),point(x,180,z+h)],fill,stroke);}
function ring(ctx,x,y,z,rx,ry,color){const p=point(x,y,z);ctx.beginPath();ctx.ellipse(p.x,p.y,rx,ry,-.05,0,Math.PI*2);ctx.strokeStyle=color;ctx.lineWidth=2;ctx.stroke();}
function plant(ctx,x,y,accent){
 box(ctx,x-12,y-10,24,22,28,'#6c7770','#343f40','#253437');
 const p=point(x,y,48);for(let i=0;i<5;i++){ctx.beginPath();ctx.ellipse(p.x+(i-2)*7,p.y-Math.abs(i-2)*4,8,23,(i-2)*.4,0,Math.PI*2);ctx.fillStyle=i%2?'#648575':'#3f6b5c';ctx.fill();}
 ring(ctx,x,y,29,13,4,accent+'55');
}
function glass(ctx,x,y,w,d,height,accent){
 const a=point(x,y),b=point(x+w,y+d),c=point(x+w,y+d,height),e=point(x,y,height);
 polygon(ctx,[a,b,c,e],'#bde8f007',accent+'40');
 line(ctx,e,c,accent+'aa',2);line(ctx,a,e,accent+'55');line(ctx,b,c,accent+'55');
 polygon(ctx,[point(x+w*.16,y+d*.16,10),point(x+w*.25,y+d*.25,10),point(x+w*.38,y+d*.38,height-10),point(x+w*.29,y+d*.29,height-10)],'#dcf7ff08');
}
function surface(ctx,l,style){
 const x=l+10,y=180,w=640,d=674;
 plane(ctx,x,y,w,d,0,style.floor,'#93a5aa66');
 ctx.save();polygon(ctx,[point(x,y),point(x+w,y),point(x+w,y+d),point(x,y+d)]);ctx.clip();
 if(style.id==='executive'||style.id==='library'){
  for(let u=x;u<x+w;u+=38){plane(ctx,u,y,1,d,0,style.tile);for(let v=y+((u-x)%3)*73;v<y+d;v+=170)line(ctx,point(u,v),point(u+38,v),style.tile+'99');}
 }else{
  for(let v=y;v<y+d;v+=112)line(ctx,point(x,v),point(x+w,v),style.tile,1);
  for(let u=x;u<x+w;u+=126)line(ctx,point(u,y),point(u,y+d),style.tile,1);
  // Restrained mineral veins, deterministic rather than random per frame.
  for(let n=0;n<15;n++){const u=x+(n*97)%640,v=y+(n*131)%674;line(ctx,point(u,v),point(u+36,v+14),style.tile+'88');line(ctx,point(u+36,v+14),point(u+67,v+10),style.tile+'55');}
 }
 const sheen=ctx.createLinearGradient(0,200,0,880);sheen.addColorStop(0,'#d9edec0d');sheen.addColorStop(.6,'#ffffff00');sheen.addColorStop(1,'#00000024');
 plane(ctx,x,y,w,d,1,sheen);ctx.restore();
 // Soft window-light pools and a reflected mullion pattern on polished floors.
 for(let n=0;n<4;n++){
  const p=point(x+90+n*140,y+230),glow=ctx.createRadialGradient(p.x,p.y,4,p.x,p.y,155);
  glow.addColorStop(0,style.accent+'0d');glow.addColorStop(1,style.accent+'00');
  plane(ctx,x+16,y+170,606,435,2,glow);
 }
 // A deep slab and inlaid metallic perimeter read as architecture, not a grid.
 box(ctx,x,y+d,w,12,15,style.edge,'#162630','#101d25',-16);
 for(const inset of [15,20])plane(ctx,x+inset,y+inset,w-2*inset,d-2*inset,1,null,style.accent+(inset===15?'44':'18'));
}
function backWall(ctx,l,style){
 wallPanel(ctx,l+10,640,0,190,style.wall,'#80949e');
 for(let k=0;k<5;k++){
  const x=l+19+k*125;
  wallPanel(ctx,x,116,19,157,'#95c9dc13','#afcedc55');
  // Distant skyline through full-height glazing.
  for(let n=0;n<5;n++)wallPanel(ctx,x+4+n*21,15,22,20+(n*31+k*17)%63,'#0d293a77');
  polygon(ctx,[point(x+7,180,27),point(x+22,180,27),point(x+72,180,165),point(x+57,180,165)],'#def5ff13');
 }
 line(ctx,point(l+10,180,185),point(l+650,180,185),style.accent+'cc',3);
 line(ctx,point(l+10,180,8),point(l+650,180,8),style.accent+'88',2);
 // Recessed cove light and a narrow stone ceiling edge, with the roof cut away.
 plane(ctx,l+8,175,644,15,194,'#52616a',style.accent+'88');
 line(ctx,point(l+17,187,181),point(l+641,187,181),'#fff1ce66',2);
}
function roomFeature(ctx,l,style){
 const x=l+50;
 if(style.id==='executive'){
  for(let i=0;i<14;i++)wallPanel(ctx,l+19+i*10,2,12,120,'#96704b66');
  wallPanel(ctx,l+477,140,25,92,'#1c252a','#ab9361');
  for(let i=0;i<3;i++){const p=point(l+520+i*25,180,73);ring(ctx,l+520+i*25,180,73,17,30,'#d7b56c99');line(ctx,{x:p.x-6,y:p.y+8},{x:p.x+10,y:p.y-12},'#ecd7a3',2);}
  roundedPlane(ctx,x+15,292,505,475,2,20,'#353c3e88',style.accent+'33');
 }else if(style.id==='boardroom'){
  roundedPlane(ctx,x,286,535,484,2,28,'#4c414988',style.accent+'44');
  box(ctx,l+230,228,230,110,22,'#302d34','#1a1c24','#22232b',42);
  for(const side of [0,1])for(let i=0;i<4;i++)box(ctx,l+236+i*57,side?343:206,40,25,38,'#a57351','#77523d','#4f3b32');
  ring(ctx,l+345,280,186,120,25,'#e1bb86bb');ring(ctx,l+345,280,190,120,25,'#fff4d399');
 }else if(style.id==='library'){
  for(const offset of [20,490]){
   wallPanel(ctx,l+offset,130,10,130,'#172b25','#99835f');
   for(let row=0;row<3;row++)for(let book=0;book<10;book++)wallPanel(ctx,l+offset+7+book*12,8,18+row*38,22+(book%3)*3,['#bc9472','#8d9b89','#bbac87','#687a76'][book%4]);
   for(let row=0;row<4;row++)wallPanel(ctx,l+offset,130,12+row*38,3,'#a28c64');
  }
  roundedPlane(ctx,x,290,534,485,2,20,'#214d4166','#9a9f6e55');
 }else if(style.id==='training'){
  for(let n=0;n<4;n++){
   box(ctx,l+28+n*42,201,30,44,105,'#a9b0c4','#33425c','#293749');
   for(let k=0;k<7;k++)line(ctx,point(l+32+n*42,246,16+k*12),point(l+52+n*42,246,16+k*12),'#c3b5f4aa',2);
  }
  for(let n=0;n<3;n++)ring(ctx,l+472,240,55+n*27,64-n*10,16,style.accent+'99');
  plane(ctx,x,295,531,469,2,'#a9baff0b','#b0bcf055');
 }else if(style.id==='atelier'){
  wallPanel(ctx,l+27,208,32,100,'#122633','#7c9fbb');
  for(let i=0;i<5;i++){wallPanel(ctx,l+44+i*35,24,50,35+i%2*18,'#8ec1d41a','#8ec1d455');}
  box(ctx,l+438,210,175,78,15,'#274c61','#253645','#192d3a',42);
  plane(ctx,l+450,219,150,60,59,'#9cd6eb22','#94b6c388');
  for(let n=0;n<4;n++)line(ctx,point(l+462+n*30,227,60),point(l+477+n*30,270,60),'#9ad4e577');
 }else if(style.id==='quality'){
  wallPanel(ctx,l+29,185,35,91,'#42665e','#b0d9c3');
  for(let i=0;i<3;i++){ring(ctx,l+65+i*52,180,80,15,18,'#c4edce');const p=point(l+65+i*52,180,80);line(ctx,{x:p.x-7,y:p.y},{x:p.x-1,y:p.y+6},'#c4edce',2);line(ctx,{x:p.x-1,y:p.y+6},{x:p.x+9,y:p.y-7},'#c4edce',2);}
  plant(ctx,l+558,213,style.accent);plant(ctx,l+603,220,style.accent);
 }else if(style.id==='control'){
  for(let n=0;n<5;n++){
   wallPanel(ctx,l+25+n*119,110,31,103,'#0a2431','#629ca4');
   for(let k=0;k<5;k++)wallPanel(ctx,l+35+n*119,18+(k*37+n*7)%80,46+k*16,3,['#70bcbd','#b0bd87','#7e9ac2'][k%3]);
  }
  plane(ctx,x,293,535,480,2,'#112f3b88',style.accent+'55');
 }else if(style.id==='review'){
  for(let i=0;i<15;i++)wallPanel(ctx,l+18+i*10,2,11,127,'#c4a67944');
  wallPanel(ctx,l+460,149,27,100,'#241f2c','#b69a68');
  const p=point(l+534,180,76);line(ctx,{x:p.x,y:p.y-34},{x:p.x,y:p.y+30},'#ddc189',3);line(ctx,{x:p.x-42,y:p.y-20},{x:p.x+42,y:p.y-20},'#ddc189',2);
  for(const side of [-1,1]){line(ctx,{x:p.x+side*33,y:p.y-20},{x:p.x+side*33,y:p.y+10},'#ddc189');ring(ctx,l+534+side*36,180,63,16,6,'#ddc189');}
  plane(ctx,x,293,532,480,2,'#62364055',style.accent+'55');
 }
}
function desk(ctx,r,style){
 const x=r.x+32,y=r.y+96,w=211,d=71;
 // Suspended stone desktops, recessed bronze bases and leather task chairs.
 plane(ctx,x-7,y-5,w+24,d+30,1,'#0000001c');
 box(ctx,x+12,y+10,22,49,53,style.edge,'#26353b','#17282e');
 box(ctx,x+w-34,y+10,22,49,53,style.edge,'#26353b','#17282e');
 box(ctx,x,y,w,d,9,style.desk,style.edge,'#34434c',53);
 const p=point(x,y,63),finish=ctx.createLinearGradient(p.x,p.y,p.x+200,p.y+40);finish.addColorStop(0,style.desk);finish.addColorStop(.55,style.desk);finish.addColorStop(1,style.edge);
 roundedPlane(ctx,x-1,y-1,w+2,d+2,63,12,finish,style.accent+'77');
 roundedPlane(ctx,x+45,y+8,127,52,64,5,'#13212e50');
 if(['executive','quality','boardroom','review'].includes(style.id)){
  for(let k=0;k<3;k++)line(ctx,point(x+24+k*54,y+8,63),point(x+58+k*47,y+63,63),style.edge+'66');
 }
 const displayX=x+53,displayW=style.id==='control'?122:103;
 polygon(ctx,[point(displayX,y+22,65),point(displayX+displayW,y+22,65),point(displayX+displayW,y+22,112),point(displayX,y+22,112)],'#0b1b27',style.accent+'88');
 for(let i=0;i<3;i++)line(ctx,point(displayX+10,y+22,78+i*10),point(displayX+25+(i*23)%65,y+22,78+i*10),style.accent+(i?'66':'aa'),2);
 plane(ctx,x+70,y+42,71,18,64,'#242f3788',style.accent+'44');
 box(ctx,x+w-31,y+33,12,12,15,'#dad4bf','#a6a99d','#707d7a',64);
 // Upholstered swivel chairs: stitched backs, metallic arms and a pedestal base.
 const leather=['executive','boardroom','review'].includes(style.id)?'#9d7455':style.id==='quality'?'#809d91':'#36515c';
 box(ctx,x+105,y+101,5,14,23,style.edge,'#62737b','#293d46');
 roundedPlane(ctx,x+80,y+88,57,38,32,10,leather,style.accent+'55');
 const chair=point(x+108,y+117,79),seat=ctx.createLinearGradient(chair.x-25,chair.y,chair.x+25,chair.y+48);seat.addColorStop(0,leather);seat.addColorStop(.5,leather);seat.addColorStop(1,'#142d3b');
 ctx.beginPath();ctx.roundRect(chair.x-26,chair.y,52,48,13);ctx.fillStyle=seat;ctx.fill();ctx.strokeStyle=style.accent+'44';ctx.stroke();
 for(const side of [-1,1])line(ctx,{x:chair.x+side*18,y:chair.y+10},{x:chair.x+side*18,y:chair.y+36},style.accent+'33');
 // A brass task light and low-profile paper folio finish each workstation.
 const lamp=point(x+22,y+16,64);line(ctx,lamp,{x:lamp.x,y:lamp.y-37},style.accent,2);line(ctx,{x:lamp.x,y:lamp.y-37},{x:lamp.x+20,y:lamp.y-44},style.accent,2);line(ctx,{x:lamp.x+15,y:lamp.y-43},{x:lamp.x+31,y:lamp.y-43},'#fff0c9',3);
 plane(ctx,x+7,y+40,25,16,64,'#dddac6');
}
export function renderInterior(ctx,f){
 const sleep=f.id<=2;
 const designs=sleep?[{id:'recharge',material:'QUIET GLASS / IDENTICAL INDIVIDUAL PODS',floor:'#273e4a',tile:'#49616a',wall:'#1d303e',accent:'#9bbac9',edge:'#496273'}]:[roomDesigns[`${f.id}:0`],roomDesigns[`${f.id}:1`]];
 ctx.canvas.interiorDesign={camera:'oblique-35deg',floor:f.id,rooms:designs.map(d=>({id:d.id,material:d.material}))};
 label(ctx,`${String(f.id).padStart(2,'0')} / ${f.name}`,{x:128,y:101},25,'#e0eaed');
 label(ctx,'INTERIOR / ELEVATED ARCHITECTURAL VIEW',{x:1470,y:87},11,'#90a7b3','right');
 for(let room=0;room<2;room++){
  const l=126+room*660,style=designs[sleep?0:room];surface(ctx,l,style);backWall(ctx,l,style);
  if(!sleep)roomFeature(ctx,l,style);
  const p=point(l+30,180,146);
  label(ctx,sleep?'RECHARGE SUITE':f.rooms[room],p,18,'#edf3f1');
  label(ctx,style.material,{x:p.x,y:p.y+20},8,style.accent);
  if(!sleep)for(let n=0;n<4;n++)desk(ctx,detailStationRect({room,seat:n}),style);
  if(!sleep){plant(ctx,l+32,818,style.accent);plant(ctx,l+617,818,style.accent);}
 }
 // The front is cut away; transparent side and dividing walls retain room depth.
 glass(ctx,136,180,0,674,180,'#8ab4c2');glass(ctx,1456,180,0,674,180,'#8ab4c2');
 if(!sleep)glass(ctx,795,180,0,674,155,'#acc8ca');
 line(ctx,point(136,850,4),point(1456,850,4),'#d1ba8877',2);
}
