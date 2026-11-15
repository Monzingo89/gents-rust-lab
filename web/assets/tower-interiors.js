import {interiorPoint as point,roomDesigns} from './tower-layout.js';
import {surfaceCanvas,finish,shaded,material,floorMaterial} from './tower-materials.js';

const interiors=new Map();
const equipment=typeof Image==='undefined'?null:new Image();
if(equipment)equipment.src=new URL('./gents/equipment-realistic.png',import.meta.url).href;
const equipmentReady=()=>!!equipment?.complete&&equipment.naturalWidth>0;
const roomPhotos=new Map();
if(typeof Image!=='undefined')for(let id=0;id<=11;id++){
 const image=new Image();image.src=new URL(`./rooms/floor-${id}.png`,import.meta.url).href;roomPhotos.set(id,image);
}
const photoFor=f=>{const image=roomPhotos.get(f.id);return image?.complete&&image.naturalWidth>0?image:null;};

const polygon=(ctx,points,fill,stroke)=>{ctx.beginPath();points.forEach((p,i)=>i?ctx.lineTo(p.x,p.y):ctx.moveTo(p.x,p.y));ctx.closePath();if(fill){ctx.fillStyle=fill;ctx.fill();}if(stroke){ctx.strokeStyle=stroke;ctx.lineWidth=1;ctx.stroke();}};
const plane=(ctx,x,y,w,d,z,fill,stroke)=>polygon(ctx,[point(x,y,z),point(x+w,y,z),point(x+w,y+d,z),point(x,y+d,z)],fill,stroke);
const line=(ctx,a,b,color,width=1)=>{ctx.beginPath();ctx.moveTo(a.x,a.y);ctx.lineTo(b.x,b.y);ctx.strokeStyle=color;ctx.lineWidth=width;ctx.stroke();};
const label=(ctx,value,p,size,color,align='left',weight=500)=>{ctx.font=`${weight} ${size}px ui-sans-serif,system-ui`;ctx.fillStyle=color;ctx.textAlign=align;ctx.fillText(value,p.x,p.y);ctx.textAlign='left';};
function roundedPlane(ctx,x,y,w,d,z,radius,fill,stroke){
 ctx.save();ctx.transform(.96,-.04,.22,.72,72-126*.96-170*.22,350+126*.04-170*.72-z);ctx.beginPath();ctx.roundRect(x,y,w,d,radius);if(fill){ctx.fillStyle=fill;ctx.fill();}if(stroke){ctx.strokeStyle=stroke;ctx.stroke();}ctx.restore();
}
function box(ctx,x,y,w,d,h,top,front,side,z=0){
 if(z===0){ctx.save();ctx.shadowColor='#0008';ctx.shadowBlur=16;ctx.shadowOffsetY=8;plane(ctx,x+4,y+7,w+7,d+8,0,'#0003');ctx.restore();}
 const face=point(x,y+d,z+h),foot=point(x+w,y+d,z);
 polygon(ctx,[point(x,y+d,z),point(x+w,y+d,z),point(x+w,y+d,z+h),point(x,y+d,z+h)],shaded(ctx,face,foot,front,11,-16));
 polygon(ctx,[point(x+w,y,z),point(x+w,y+d,z),point(x+w,y+d,z+h),point(x+w,y,z+h)],shaded(ctx,point(x+w,y,z+h),foot,side,10,-13));
 plane(ctx,x,y,w,d,z+h,shaded(ctx,point(x,y,z+h),point(x+w,y+d,z+h),top,19,-7),'#ffffff18');
 line(ctx,point(x,y+d,z+h),point(x+w,y+d,z+h),'#ffffff25',.7);
}
function wall(ctx,x,w,z,h,fill,stroke){polygon(ctx,[point(x,170,z),point(x+w,170,z),point(x+w,170,z+h),point(x,170,z+h)],fill,stroke);}
function monitor(ctx,x,w,z,h,title,accent,lines=4){
 wall(ctx,x,w,z,h,'#07141e',accent+'99');label(ctx,title,point(x+w/2,170,z+h-18),10,accent,'center',700);
 for(let i=0;i<lines;i++){const y=z+18+i*(h-50)/Math.max(1,lines-1),len=22+((x+i*37)%Math.max(28,w-45));line(ctx,point(x+14,170,y),point(x+Math.min(w-14,len+20),170,y),i===1?'#e4ad67':accent+'aa',2);}
}
function plant(ctx,x,y,accent,scale=1){
 const base=point(x,y),p=point(x,y,34*scale);ctx.save();ctx.shadowColor='#0007';ctx.shadowBlur=17;ctx.shadowOffsetY=5;
 const pot=ctx.createLinearGradient(p.x-20*scale,0,p.x+20*scale,0);pot.addColorStop(0,'#343b37');pot.addColorStop(.38,'#b2a793');pot.addColorStop(1,'#4b5149');ctx.fillStyle=pot;ctx.beginPath();ctx.moveTo(p.x-20*scale,p.y);ctx.lineTo(p.x-15*scale,base.y);ctx.quadraticCurveTo(base.x,base.y+8*scale,base.x+15*scale,base.y);ctx.lineTo(p.x+20*scale,p.y);ctx.closePath();ctx.fill();ctx.restore();
 ctx.beginPath();ctx.ellipse(p.x,p.y,20*scale,6*scale,0,0,Math.PI*2);ctx.fillStyle='#20291b';ctx.fill();
 for(let i=0;i<19;i++){const angle=i*2.399,reach=(30+i%5*8)*scale,tip={x:p.x+Math.cos(angle)*reach,y:p.y-(36+i%7*9)*scale};
  line(ctx,p,tip,'#4b6542',1.1*scale);ctx.beginPath();ctx.moveTo(tip.x,tip.y);ctx.bezierCurveTo(tip.x-20*scale,tip.y-18*scale,p.x-12*scale,p.y-12*scale,p.x,p.y-7*scale);ctx.bezierCurveTo(p.x+13*scale,p.y-14*scale,tip.x+15*scale,tip.y+5*scale,tip.x,tip.y);ctx.fillStyle=i%3?'#36573a':'#6f8654';ctx.fill();line(ctx,{x:p.x,y:p.y-7*scale},tip,'#9aaa6750',.7);
 }
}
function chair(ctx,x,y,color='#59473d'){
 const feet=point(x+26,y+25,0);for(const dx of [-24,24])line(ctx,feet,{x:feet.x+dx,y:feet.y+5},'#62676a',3);
 box(ctx,x,y,52,48,14,color,'#252425','#1b2023',18);const p=point(x+26,y+14,82);ctx.save();ctx.shadowColor='#0006';ctx.shadowBlur=6;ctx.beginPath();ctx.roundRect(p.x-25,p.y,50,51,8);ctx.fillStyle=shaded(ctx,p,{x:p.x+25,y:p.y+50},color,24,-9);ctx.fill();ctx.restore();line(ctx,{x:p.x-19,y:p.y+7},{x:p.x+19,y:p.y+7},'#ffffff27',.8);for(let i=0;i<3;i++)line(ctx,{x:p.x-18,y:p.y+19+i*9},{x:p.x+18,y:p.y+19+i*9},'#00000026',.8);
}
function table(ctx,x,y,w,d,style,height=48){
 box(ctx,x+18,y+10,12,d-20,height-7,style.edge,'#28343a','#18232a');box(ctx,x+w-30,y+10,12,d-20,height-7,style.edge,'#28343a','#18232a');box(ctx,x,y,w,d,7,style.desk,style.edge,'#29343b',height);
 plane(ctx,x+2,y+2,w-4,d-4,height+7,material(ctx,['testing','engineering','operations'].includes(style.id)?'metal':'wood',style.desk));
}
function bookcase(ctx,x,w,style,rows=4){
 box(ctx,x,183,w,34,176,style.edge,'#1a241e','#302c24');
 for(let row=0;row<rows;row++){const base=15+row*32;wall(ctx,x+6,w-12,base,4,material(ctx,'wood',style.edge));let cursor=x+12;for(let book=0;cursor<x+w-14;book++){const bw=5+(book*7+row*3)%9,bh=20+(book*11+row)%10;wall(ctx,cursor,bw,base+5,bh,['#97714f','#6c7f65','#b0a17a','#465e62','#69514d','#352f2c'][book%6]);line(ctx,point(cursor+2,170,base+10),point(cursor+bw-1,170,base+10),'#d8bf824f',.6);cursor+=bw+2;}}
 wall(ctx,x,7,8,176,style.edge);wall(ctx,x+w-7,7,8,176,style.edge);
}
function baseArchitecture(ctx,style){
 const x=126,y=170,w=1320,d=710;
 const kind=floorMaterial(style);
 plane(ctx,x,y,w,d,0,material(ctx,kind,style.floor),'#a9a6a044');
 const step=kind==='wood'?42:kind==='marble'?220:130;
 if(kind!=='carpet')for(let v=y;v<y+d;v+=step){line(ctx,point(x,v),point(x+w,v),'#0c10101c',.8);for(let u=x+(Math.floor(v/step)%2)*110;u<x+w;u+=kind==='wood'?300:220)line(ctx,point(u,v),point(u,Math.min(y+d,v+step)),'#0c101022',.8);}
 const sheen=ctx.createLinearGradient(0,210,0,900);sheen.addColorStop(0,'#fff5da20');sheen.addColorStop(.5,'#ffffff04');sheen.addColorStop(1,'#00000032');plane(ctx,x,y,w,d,1,sheen);
 wall(ctx,x,w,0,195,shaded(ctx,{x:0,y:140},{x:0,y:350},style.wall,12,-15),'#b8b8aa44');
 wall(ctx,x,w,0,45,material(ctx,'wood',style.edge));wall(ctx,x,w,44,3,finish(style.accent,-30));
 for(let k=0;k<5;k++){
  const wx=x+34+k*253,glass=ctx.createLinearGradient(0,140,0,315);glass.addColorStop(0,'#192e3d');glass.addColorStop(.6,'#384b51');glass.addColorStop(1,'#c3ae8159');
  wall(ctx,wx,230,55,120,glass,'#ddd8c74d');
  for(let n=0;n<10;n++){const bx=wx+4+n*22,bh=16+(n*19+k*29)%58;wall(ctx,bx,15,55,bh,'#111e29aa');for(let level=0;level<bh-5;level+=13)wall(ctx,bx+3,2,58+level,3,'#d8c79c55');}
  wall(ctx,wx+111,3,55,120,'#79817a');line(ctx,point(wx+15,170,164),point(wx+92,170,66),'#e5f4f117',11);
 }
 // Fluted timber piers and concealed cove lighting instead of neon outlines.
 for(let u=x+9;u<x+w;u+=253)for(let flute=0;flute<3;flute++)wall(ctx,u+flute*5,2,48,139,finish(style.edge,flute%2?16:-12));
 wall(ctx,x,w,181,11,material(ctx,'metal',style.edge));
 line(ctx,point(x+4,170,181),point(x+w-4,170,181),'#f9dfb266',2);
 for(const lx of [310,770,1230]){const p=point(lx,245,0),light=ctx.createRadialGradient(p.x,p.y,5,p.x,p.y,180);light.addColorStop(0,'#ffedbd1c');light.addColorStop(1,'#ffedbd00');ctx.save();polygon(ctx,[point(x,y),point(x+w,y),point(x+w,y+d),point(x,y+d)],null);ctx.clip();ctx.fillStyle=light;ctx.fillRect(p.x-180,p.y-180,360,360);ctx.restore();}
 box(ctx,x,y+d,w,14,16,style.edge,'#14232c','#101b22',-16);
}
function panel(ctx,x,y,w,z,h,fill,stroke){polygon(ctx,[point(x,y,z),point(x+w,y,z),point(x+w,y,z+h),point(x,y,z+h)],fill,stroke);}
function computer(ctx,x,y,z=59,w=83){
 plane(ctx,x+w*.35,y-8,w*.3,26,z,'#737e83');box(ctx,x+w*.47,y,5,5,20,'#9ba4a8','#515e65','#29343a',z);
 panel(ctx,x,y,w,z+17,52,'#141b20','#77868b');panel(ctx,x+3,y,w-6,z+21,45,shaded(ctx,point(x,y,z+65),point(x+w,y,z+21),'#233b45',12,-8));
 line(ctx,point(x+8,y,z+58),point(x+w-9,y,z+58),'#a2b9b14d',1);line(ctx,point(x+8,y,z+51),point(x+w*.5,y,z+51),'#8ca6b54d',1);
 plane(ctx,x+5,y+30,w-10,22,z,'#3c474b','#8e979944');for(let k=0;k<5;k++)line(ctx,point(x+10+k*12,y+33,z+.5),point(x+10+k*12,y+48,z+.5),'#9ba4a14d',.6);
}
function cup(ctx,x,y,z=57){const p=point(x,y,z);ctx.fillStyle=shaded(ctx,{x:p.x-6,y:p.y-18},{x:p.x+6,y:p.y},'#d3cbb9',20,-30);ctx.fillRect(p.x-6,p.y-15,12,15);ctx.beginPath();ctx.ellipse(p.x,p.y-15,6,3,0,0,Math.PI*2);ctx.fillStyle='#3e2c21';ctx.fill();ctx.beginPath();ctx.arc(p.x+7,p.y-8,5,-1.3,1.3);ctx.strokeStyle='#d5cdbb';ctx.lineWidth=2;ctx.stroke();}
function sofa(ctx,x,y,w=215,color='#6f5748'){
 box(ctx,x+8,y+8,w-16,68,28,color,'#302b28','#242426',10);box(ctx,x,y,w,18,62,color,'#4c3d34','#2a2827',15);for(let i=0;i<3;i++){const cw=(w-36)/3;box(ctx,x+16+i*cw,y+23,cw-4,46,8,color,finish(color,-20),'#302d29',40);}for(const sx of [x,x+w-16])box(ctx,sx,y+17,16,58,40,color,finish(color,-19),'#302d29',18);
}
function rug(ctx,x,y,w,d,style){roundedPlane(ctx,x,y,w,d,1,6,material(ctx,'carpet',finishHex(style.floor,16)),'#eee1c126');for(let inset=8;inset<18;inset+=6)roundedPlane(ctx,x+inset,y+inset,w-2*inset,d-2*inset,2,4,null,'#c9b28d27');}
function finishHex(color,delta){return '#'+color.slice(1,7).match(/../g).map(v=>Math.max(0,Math.min(255,parseInt(v,16)+delta)).toString(16).padStart(2,'0')).join('');}
function deskDetails(ctx,x,y,w,style,height=56){computer(ctx,x+w*.25,y+22,height,Math.min(100,w*.35));cup(ctx,x+w-24,y+66,height);plane(ctx,x+15,y+65,43,30,height,'#d4cbb5','#af9c76');for(let i=0;i<4;i++)line(ctx,point(x+20,y+71+i*5,height+1),point(x+50,y+71+i*5,height+1),'#7a756344',.5);}
function readingLight(ctx,x,y,z){
 const base=point(x,y,z),top=point(x,y,z+37);plane(ctx,x-13,y-10,26,20,z,'#806c40');line(ctx,base,top,'#baaa76',3);ctx.save();ctx.shadowColor='#f5d89e99';ctx.shadowBlur=19;ctx.beginPath();ctx.ellipse(top.x,top.y,30,10,-.05,Math.PI,Math.PI*2);ctx.lineTo(top.x+30,top.y+5);ctx.lineTo(top.x-30,top.y+5);ctx.closePath();ctx.fillStyle=shaded(ctx,{x:top.x,y:top.y-13},{x:top.x,y:top.y+5},'#345a42',15,-18);ctx.fill();ctx.restore();line(ctx,{x:top.x-25,y:top.y+4},{x:top.x+25,y:top.y+4},'#ecdcae',2);
}
function microscope(ctx,x,y,z){
 plane(ctx,x-15,y-10,32,28,z,'#a4b5b5');const foot=point(x,y,z),arm=point(x+3,y,z+35);line(ctx,foot,arm,'#c3cecc',9);line(ctx,arm,{x:arm.x-18,y:arm.y-11},'#e7eeee',9);line(ctx,{x:arm.x-18,y:arm.y-11},{x:arm.x-22,y:arm.y-17},'#25343b',5);line(ctx,{x:foot.x-12,y:foot.y-13},{x:foot.x+12,y:foot.y-13},'#263940',4);
}
function snackMachines(ctx){
 // Recognizable equipment remains available offline while the photo loads.
 box(ctx,1020,350,130,90,92,'#9a3830','#662621','#39241e');box(ctx,1020,350,130,90,12,'#b99850','#775c2e','#47391f',200);
 panel(ctx,1024,440,122,95,103,'#d8e7db18','#d9c98b99');panel(ctx,1027,440,116,95,3,'#bd9b57');
 for(let i=0;i<90;i++){const p=point(1030+(i*17)%111,438,99+(i*7)%25);ctx.beginPath();ctx.arc(p.x,p.y,2+(i%3)*.6,0,Math.PI*2);ctx.fillStyle=['#ddc080','#f4dfaa','#b49353'][i%3];ctx.fill();}
 box(ctx,1061,383,48,32,27,'#d1cbc0','#817f73','#595b52',143);line(ctx,point(1083,393,170),point(1083,393,199),'#b5b9ae',3);
 box(ctx,1190,350,135,90,95,'#a7abad','#515a5b','#2b363b');box(ctx,1194,354,127,80,22,'#727b7a','#343d43','#19272d',95);
 for(let bowl=0;bowl<2;bowl++){const x=1200+bowl*62,p=point(x+25,400,180);ctx.beginPath();ctx.roundRect(p.x-22,p.y-43,44,64,12);ctx.fillStyle=shaded(ctx,{x:p.x-21,y:p.y-42},{x:p.x+21,y:p.y+20},bowl?'#408ba3':'#bc413e',25,-25);ctx.fill();ctx.strokeStyle='#e5f7f28c';ctx.lineWidth=2;ctx.stroke();line(ctx,{x:p.x-16,y:p.y-36},{x:p.x-16,y:p.y+6},'#ffffff59',3);line(ctx,{x:p.x,y:p.y+21},{x:p.x,y:p.y+39},'#111d24',5);line(ctx,{x:p.x-5,y:p.y+26},{x:p.x+5,y:p.y+26},'#30383b',4);}
 for(let slot=0;slot<11;slot++)line(ctx,point(1198+slot*10,439,99),point(1198+slot*10,422,99),'#c1cacb',1);
}
function tokenDesk(ctx,style,large=false){
 const w=large?470:330,x=large?545:620,y=510;rug(ctx,x-70,y-45,w+140,290,style);table(ctx,x,y,w,126,style,large?62:52);deskDetails(ctx,x,y,w,style,large?70:60);chair(ctx,x+w/2-26,y+152,large?'#815a3e':'#5d5048');
}
function codexSuite(ctx,style){
 for(let i=0;i<12;i++){const x=142+i*106;const p=point(x,170,152);ctx.beginPath();ctx.arc(p.x,p.y,7,0,Math.PI*2);ctx.fillStyle='#151515';ctx.fill();line(ctx,p,{x:p.x+((i%2)?9:-9),y:p.y+11},'#ffd868',2);}
 // Golden quantum computer suspended inside a glass containment cube.
 box(ctx,245,315,325,230,170,'#c79523aa','#60420f88','#33230d88');
 polygon(ctx,[point(245,315,0),point(570,545,0),point(570,545,205),point(245,315,205)],'#f9db6b0d','#ffe18b88');
 const q=point(405,430,116);for(let i=0;i<5;i++){ctx.beginPath();ctx.arc(q.x,q.y,18+i*14,0,Math.PI*2);ctx.strokeStyle=i%2?'#6fffd4':'#ffd76a';ctx.lineWidth=2;ctx.stroke();}
 label(ctx,'QUANTUM CORE / CONTAINED',point(405,315,188),11,'#ffe18b','center',700);
 // Bean bag and snack machines.
 const bean=point(760,625,30);ctx.save();ctx.shadowColor='#0009';ctx.shadowBlur=23;ctx.shadowOffsetY=13;ctx.beginPath();ctx.ellipse(bean.x,bean.y,92,58,-.05,0,Math.PI*2);ctx.fillStyle=shaded(ctx,{x:bean.x-50,y:bean.y-45},{x:bean.x+50,y:bean.y+50},'#88705a',25,-34);ctx.fill();ctx.restore();ctx.beginPath();ctx.ellipse(bean.x,bean.y-9,60,27,-.1,0,Math.PI);ctx.strokeStyle='#cbb39466';ctx.stroke();
 if(equipmentReady()){const p=point(1145,470);ctx.drawImage(equipment,p.x-196,p.y-235,392,260);}
 else snackMachines(ctx);
 sofa(ctx,610,340,270,'#877158');plant(ctx,174,650,style.accent,1.3);plant(ctx,1375,735,style.accent,1.2);
 // The hotline is always present; its glow is animated in tower-scene.
 box(ctx,884,545,74,54,23,'#b72622','#67120f','#410b0a',38);label(ctx,'EXECUTIVE HOTLINE',point(920,545,77),9,'#ff9b8f','center',750);
}
function ceoOffice(ctx,style){
 const metrics=['OBSERVABILITY','FAILURES','COMPLIANCE','BIAS','DRIFT','SECURITY'];metrics.forEach((name,i)=>monitor(ctx,145+i*210,190,35,126,name,style.accent,5));
 tokenDesk(ctx,style,true);plant(ctx,210,710,style.accent,1.4);plant(ctx,1370,710,style.accent,1.4);
 sofa(ctx,210,445,215,'#524b43');sofa(ctx,1090,490,210,'#524b43');table(ctx,1110,650,155,75,style,25);cup(ctx,1170,680,34);
 for(let i=0;i<5;i++)box(ctx,158+i*49,270,42,43,54,'#80715a','#4a4237','#302c28');
 wall(ctx,1260,130,20,145,'#6b4729','#d7b179');label(ctx,'ENTERPRISE AI / CEO',point(1325,170,102),13,'#f2ddbd','center',650);
}
function cfoOffice(ctx,style){
 monitor(ctx,520,520,42,112,'TOKENS SPENT / CLOUD ESCALATIONS',style.accent,6);tokenDesk(ctx,style,false);
 label(ctx,'ONE DESK · ONE LEDGER · EVERY ESCALATION EXPLAINED',point(780,170,176),11,'#d5bc92','center',650);plant(ctx,240,690,style.accent);wall(ctx,1160,180,25,130,'#594332','#a68359');
 for(let i=0;i<3;i++){box(ctx,240+i*100,330,88,66,63,'#796b58','#514b42','#363b3a');for(let row=0;row<3;row++){const p=point(260+i*100,397,18+row*17);line(ctx,p,{x:p.x+35,y:p.y-1},'#c5b89a',2);}}sofa(ctx,1135,570,180,'#61584e');
}
function library(ctx,style,archive=false){
 bookcase(ctx,145,270,style,4);bookcase(ctx,1120,290,style,4);bookcase(ctx,480,190,style,archive?5:4);bookcase(ctx,800,190,style,archive?5:4);
 rug(ctx,280,400,890,330,style);
 for(const [x,y]of [[310,520],[665,610],[1010,505]]){table(ctx,x,y,230,90,style,45);chair(ctx,x+88,y+112,'#49634f');readingLight(ctx,x+45,y+25,53);for(let n=0;n<3;n++)plane(ctx,x+110+n*22,y+40,37,34,53+n*.6,['#d0c5a7','#7a5941','#d1c5b1'][n]);cup(ctx,x+196,y+61,53);}
 plant(ctx,220,770,style.accent,1.2);plant(ctx,1360,760,style.accent,1.2);
 label(ctx,archive?'IMMUTABLE ARCHIVE / SOURCE MEMORY':'READING HALL / ACTIVE STUDY',point(785,170,177),12,style.accent,'center',700);
}
function engineering(ctx,style){
 for(let rack=0;rack<7;rack++){box(ctx,160+rack*178,250,120,95,170,'#171b2a','#090d16','#060910');for(let row=0;row<8;row++)line(ctx,point(172+rack*178,346,24+row*17),point(264+rack*178,346,24+row*17),row%3===0?'#a99fff':'#416274',2);}
 for(let i=0;i<12;i++)label(ctx,['fn','{}','=>','01','git','✓'][i%6],point(180+(i*113)%1180,490+(i%3)*85,80+(i%4)*23),16,i%3?'#856fda88':'#9ee6cc99','center',700);
 label(ctx,'ZERO-G CODE CHAMBER / ENGINEERS FLOAT WHILE MODELS RUN',point(785,170,179),11,style.accent,'center',700);
 for(let row=0;row<2;row++)for(let col=0;col<3;col++){const x=220+col*415,y=500+row*190;table(ctx,x,y,265,94,style,34);computer(ctx,x+22,y+18,42,95);computer(ctx,x+133,y+19,42,95);for(let cable=0;cable<3;cable++)line(ctx,point(x+35+cable*8,y+30),point(x+35+cable*8,y+104),'#191a20',2);}
}
function operations(ctx,style){
 wall(ctx,136,1300,118,48,'#071a22','#526b70');for(let i=0;i<8;i++)label(ctx,['TASKS','CONTEXT','TOOLS','QUEUES','RUNTIME','MEMORY','EVENTS','REVIEW'][i],point(220+i*160,170,146),9,'#a5bbb6','center',600);
 for(let row=0;row<3;row++)for(let col=0;col<4;col++){const x=175+col*310,y=320+row*155;table(ctx,x,y,238,88,style,38);computer(ctx,x+12,y+12,46,92);computer(ctx,x+123,y+12,46,92);cup(ctx,x+223,y+58,46);chair(ctx,x+89,y+100,'#39454a');}
 label(ctx,'LOCAL OPERATIONS EXCHANGE / LIVE ORDERS & QUEUES',point(785,170,184),11,style.accent,'center',700);
}
function conference(ctx,style){
 plane(ctx,170,220,1160,175,18,'#221b21',style.accent);monitor(ctx,320,930,43,118,'ALL-HANDS / VERIFIED WORK ONLY',style.accent,5);
 box(ctx,725,365,120,80,115,style.edge,'#51392e','#30231f',18);label(ctx,'PODIUM',point(785,445,95),10,'#ffe0bd','center',700);
 for(let row=0;row<3;row++)for(let col=0;col<6;col++)chair(ctx,245+col*175,520+row*105,row===0?'#765240':'#4d3c3d');
 for(const start of [140,1320])for(let i=0;i<6;i++)wall(ctx,start+i*16,7,15,172,i%2?'#332e31':'#665b52');for(let i=0;i<4;i++)line(ctx,point(1410,450+i*105),point(1410,450+i*105,9),'#f4db9a88',4);
}
function testing(ctx,style){
 for(let row=0;row<2;row++)for(let col=0;col<3;col++){const x=180+col*405,y=355+row*245;table(ctx,x,y,310,115,style,50);box(ctx,x+9,y+10,292,90,45,'#b2b6b5','#737c7c','#596568');plane(ctx,x,y,310,115,57,material(ctx,'stone',style.desk));computer(ctx,x+195,y+20,58,83);box(ctx,x+19,y+23,144,37,7,'#b6bfbb','#5d696b','#3d4c53',58);for(let tube=0;tube<6;tube++){const p=point(x+30+tube*21,y+40,90);ctx.beginPath();ctx.roundRect(p.x-3,p.y-23,6,25,3);ctx.fillStyle=tube%2?'#86c4bda1':'#aca1c49c';ctx.fill();ctx.strokeStyle='#ecf8f5a1';ctx.lineWidth=.7;ctx.stroke();line(ctx,{x:p.x-2,y:p.y-24},{x:p.x+2,y:p.y-24},'#d8e0de',2);}microscope(ctx,x+174,y+80,58);}
 // A visible transfer track makes test samples appear to pass bench-to-bench.
 line(ctx,point(195,505,67),point(1290,505,67),'#d8d7ee',4);for(let i=0;i<8;i++){const p=point(250+i*137,505,71);ctx.beginPath();ctx.arc(p.x,p.y,8,0,Math.PI*2);ctx.fillStyle=i%2?'#8ce3d0':'#c9b4ff';ctx.fill();}
 monitor(ctx,470,620,30,118,'EVALS / SAFETY / REGRESSION / RED TEAM',style.accent,6);
}
function lobby(ctx,style){
 // Sweeping walnut reception desk, marble inset, bell, art and hotel greenery.
 roundedPlane(ctx,480,490,610,185,58,42,style.desk,style.accent);box(ctx,505,520,560,130,70,style.desk,'#5f351d','#3b2518',0);
 const bell=point(980,545,82);ctx.beginPath();ctx.arc(bell.x,bell.y,12,Math.PI,0);ctx.fillStyle='#f6cf68';ctx.fill();line(ctx,{x:bell.x-15,y:bell.y},{x:bell.x+15,y:bell.y},'#f6cf68',3);
 wall(ctx,580,410,45,105,'#4c2c17','#d6a04c');label(ctx,'THE GENTS',point(785,170,111),24,'#f5d697','center',700);
 plant(ctx,200,620,style.accent,1.8);plant(ctx,1370,620,style.accent,1.8);plant(ctx,330,760,style.accent,1.2);plant(ctx,1230,760,style.accent,1.2);
 sofa(ctx,185,390,245,'#927754');sofa(ctx,1155,430,230,'#927754');table(ctx,210,545,165,95,style,28);table(ctx,1180,585,150,85,style,28);cup(ctx,260,570,37);cup(ctx,1220,610,37);
 for(const artX of [220,1150]){wall(ctx,artX,180,64,91,'#b9ac8e','#c29150');for(let n=0;n<12;n++)wall(ctx,artX+12+n*12,5,72,20+(n*17)%57,['#544f42','#84795b','#b5a885'][n%3]);}
}

function mcpBasement(ctx,style){
 for(let row=0;row<2;row++)for(let col=0;col<5;col++){
  const x=240+col*235,y=285+row*240;
  box(ctx,x,y,150,80,150,'#243d47','#152a33','#0a1a24');
  for(let slot=0;slot<7;slot++){
   panel(ctx,x+8,y+81,134,18+slot*17,12,'#0a1a22','#537979');
   const light=point(x+18,y+82,24+slot*17);ctx.fillStyle='#53605f';ctx.fillRect(light.x,light.y,4,3);
  }
 }
 label(ctx,'STDIO / STREAMABLE HTTP + SSE',point(785,175,115),21,style.accent,'center',700);
 label(ctx,'FUNCTION CATALOG BELOW · AUTHORITY CHECKED PER CALL',point(785,780,12),13,style.accent,'center',650);
}

function paintInterior(ctx,f){
 const style=roomDesigns[String(f.id)];baseArchitecture(ctx,style);
 ctx.canvas.interiorDesign={camera:'oblique-expanded',floor:f.id,rooms:[{id:style.id,material:style.material}],purpose:f.rooms[0]};
 label(ctx,`${f.id===0?'B1':String(f.id).padStart(2,'0')} / ${f.name}`,{x:92,y:92},25,'#edf2f3');
 label(ctx,f.rooms[0].toUpperCase(),{x:92,y:118},11,style.accent,'left',700);
 label(ctx,'OPEN-FRONT / DEEP FLOOR PLATE',{x:1490,y:86},11,'#90a7b3','right');
 ({mcp:mcpBasement,codex:codexSuite,ceo:ceoOffice,cfo:cfoOffice,archive:(c,s)=>library(c,s,true),library,engineering,operations,conference,testing,lobby}[style.id]||(()=>{}))(ctx,style);
 if(style.id==='recharge')label(ctx,'VISIBLE POD STATUS / SLEEPING GENTS REMAIN IDENTIFIABLE',point(785,170,179),11,style.accent,'center',700);
 // Transparent perimeter retains depth while keeping every room observable.
 polygon(ctx,[point(126,170,0),point(126,880,0),point(126,880,190),point(126,170,190)],'#bde8f006','#8ab4c244');
 polygon(ctx,[point(1446,170,0),point(1446,880,0),point(1446,880,190),point(1446,170,190)],'#bde8f006','#8ab4c244');
 line(ctx,point(126,880,4),point(1446,880,4),'#d1ba8877',2);
}

function cachedInterior(f){
 const photo=photoFor(f),key=`${f.id}:${equipmentReady()}:${!!photo}`;
 if(interiors.get(f.id)?.key!==key){
  const canvas=surfaceCanvas();if(!canvas)return null;
  const pen=canvas.getContext('2d');
  if(photo){
   pen.drawImage(photo,0,0,1600,1000);
   const top=pen.createLinearGradient(0,0,0,155);top.addColorStop(0,'#06121ef5');top.addColorStop(.6,'#06121eaa');top.addColorStop(1,'#06121e00');pen.fillStyle=top;pen.fillRect(0,0,1600,155);
   const bottom=pen.createLinearGradient(0,895,0,1000);bottom.addColorStop(0,'#06121e00');bottom.addColorStop(.4,'#06121e99');bottom.addColorStop(1,'#06121ed9');pen.fillStyle=bottom;pen.fillRect(0,895,1600,105);
   label(pen,`${f.id===0?'B1':String(f.id).padStart(2,'0')} / ${f.name}`,{x:92,y:78},25,'#edf2f3');label(pen,f.rooms[0].toUpperCase(),{x:92,y:105},11,roomDesigns[String(f.id)].accent,'left',650);
  }else paintInterior(pen,f);
  interiors.set(f.id,{key,canvas});
 }
 return interiors.get(f.id).canvas;
}
export function renderInterior(ctx,f){
 const image=cachedInterior(f);if(image)ctx.drawImage(image,0,0);else paintInterior(ctx,f);
 const style=roomDesigns[String(f.id)];
 ctx.canvas.interiorDesign={camera:'oblique-expanded',floor:f.id,rooms:[{id:style.id,material:style.material}],purpose:f.rooms[0],rendering:photoFor(f)?'photoreal-architectural':'textured-architectural',equipmentPhoto:equipmentReady()};
}
export function renderInteriorPreview(ctx,f,r){
 const image=cachedInterior(f);if(!image)return false;
 const width=r.w-6,height=r.h-10,sourceWidth=1470,sourceHeight=sourceWidth*height/width;
 ctx.save();ctx.beginPath();ctx.rect(r.x+3,r.y+4,width,height);ctx.clip();ctx.drawImage(image,45,Math.max(110,500-sourceHeight/2),sourceWidth,sourceHeight,r.x+3,r.y+4,width,height);ctx.restore();return true;
}
