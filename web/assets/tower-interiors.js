import {interiorPoint as point,roomDesigns} from './tower-layout.js';

const polygon=(ctx,points,fill,stroke)=>{ctx.beginPath();points.forEach((p,i)=>i?ctx.lineTo(p.x,p.y):ctx.moveTo(p.x,p.y));ctx.closePath();if(fill){ctx.fillStyle=fill;ctx.fill();}if(stroke){ctx.strokeStyle=stroke;ctx.lineWidth=1;ctx.stroke();}};
const plane=(ctx,x,y,w,d,z,fill,stroke)=>polygon(ctx,[point(x,y,z),point(x+w,y,z),point(x+w,y+d,z),point(x,y+d,z)],fill,stroke);
const line=(ctx,a,b,color,width=1)=>{ctx.beginPath();ctx.moveTo(a.x,a.y);ctx.lineTo(b.x,b.y);ctx.strokeStyle=color;ctx.lineWidth=width;ctx.stroke();};
const label=(ctx,value,p,size,color,align='left',weight=500)=>{ctx.font=`${weight} ${size}px ui-sans-serif,system-ui`;ctx.fillStyle=color;ctx.textAlign=align;ctx.fillText(value,p.x,p.y);ctx.textAlign='left';};
function roundedPlane(ctx,x,y,w,d,z,radius,fill,stroke){
 ctx.save();ctx.transform(.96,-.04,.22,.72,72-126*.96-170*.22,350+126*.04-170*.72-z);ctx.beginPath();ctx.roundRect(x,y,w,d,radius);if(fill){ctx.fillStyle=fill;ctx.fill();}if(stroke){ctx.strokeStyle=stroke;ctx.stroke();}ctx.restore();
}
function box(ctx,x,y,w,d,h,top,front,side,z=0){
 polygon(ctx,[point(x,y+d,z),point(x+w,y+d,z),point(x+w,y+d,z+h),point(x,y+d,z+h)],front);
 polygon(ctx,[point(x+w,y,z),point(x+w,y+d,z),point(x+w,y+d,z+h),point(x+w,y,z+h)],side);
 plane(ctx,x,y,w,d,z+h,top);
}
function wall(ctx,x,w,z,h,fill,stroke){polygon(ctx,[point(x,170,z),point(x+w,170,z),point(x+w,170,z+h),point(x,170,z+h)],fill,stroke);}
function monitor(ctx,x,w,z,h,title,accent,lines=4){
 wall(ctx,x,w,z,h,'#07141e',accent+'99');label(ctx,title,point(x+w/2,170,z+h-18),10,accent,'center',700);
 for(let i=0;i<lines;i++){const y=z+18+i*(h-50)/Math.max(1,lines-1),len=22+((x+i*37)%Math.max(28,w-45));line(ctx,point(x+14,170,y),point(x+Math.min(w-14,len+20),170,y),i===1?'#e4ad67':accent+'aa',2);}
}
function plant(ctx,x,y,accent,scale=1){
 box(ctx,x-13*scale,y-10*scale,26*scale,22*scale,25*scale,'#a37949','#513f31','#2e3532');
 const p=point(x,y,42*scale);for(let i=0;i<7;i++){ctx.beginPath();ctx.ellipse(p.x+(i-3)*7*scale,p.y-Math.abs(i-3)*3,8*scale,25*scale,(i-3)*.36,0,Math.PI*2);ctx.fillStyle=i%2?'#5c8b70':'#376751';ctx.fill();}
 line(ctx,point(x-22,y,8),point(x+22,y,8),accent+'55',2);
}
function chair(ctx,x,y,color='#59473d'){
 box(ctx,x,y,52,48,20,color,'#2a2525','#1b2023');const p=point(x+26,y+14,65);ctx.beginPath();ctx.roundRect(p.x-23,p.y,46,48,10);ctx.fillStyle=color;ctx.fill();
}
function table(ctx,x,y,w,d,style,height=48){
 box(ctx,x,y,w,d,9,style.desk,style.edge,'#29343b',height);box(ctx,x+18,y+10,12,d-20,height-7,style.edge,'#28343a','#18232a');box(ctx,x+w-30,y+10,12,d-20,height-7,style.edge,'#28343a','#18232a');
}
function bookcase(ctx,x,w,style,rows=4){
 wall(ctx,x,w,8,157,'#16281f',style.edge);for(let row=0;row<rows;row++){const base=19+row*35;wall(ctx,x,w,base,3,style.edge);for(let book=0;book<Math.floor(w/15);book++)wall(ctx,x+7+book*14,9,base+4,20+(book%3)*4,['#bd9670','#849887','#b9aa7f','#627b76','#8d6c67'][book%5]);}
}
function baseArchitecture(ctx,style){
 const x=126,y=170,w=1320,d=710;
 plane(ctx,x,y,w,d,0,style.floor,'#80949e55');
 for(let v=y;v<y+d;v+=92)line(ctx,point(x,v),point(x+w,v),style.tile+'88');
 for(let u=x;u<x+w;u+=118)line(ctx,point(u,y),point(u,y+d),style.tile+'55');
 const sheen=ctx.createLinearGradient(0,180,0,900);sheen.addColorStop(0,style.accent+'14');sheen.addColorStop(1,'#00000028');plane(ctx,x,y,w,d,1,sheen);
 wall(ctx,x,w,0,195,style.wall,'#8499a2');
 for(let k=0;k<9;k++){const wx=x+10+k*145;wall(ctx,wx,132,18,160,'#7ab0c00c','#96bbc74b');for(let n=0;n<4;n++)wall(ctx,wx+7+n*29,20,22,22+(n*25+k*13)%65,'#0a263477');}
 line(ctx,point(x,170,190),point(x+w,170,190),style.accent+'cc',3);
 box(ctx,x,y+d,w,14,16,style.edge,'#14232c','#101b22',-16);
}
function tokenDesk(ctx,style,large=false){
 const w=large?470:330,x=large?545:620,y=510;table(ctx,x,y,w,126,style,large?62:52);chair(ctx,x+w/2-26,y+152,large?'#815a3e':'#5d5048');
}
function codexSuite(ctx,style){
 for(let i=0;i<12;i++){const x=142+i*106;const p=point(x,170,152);ctx.beginPath();ctx.arc(p.x,p.y,7,0,Math.PI*2);ctx.fillStyle='#151515';ctx.fill();line(ctx,p,{x:p.x+((i%2)?9:-9),y:p.y+11},'#ffd868',2);}
 // Golden quantum computer suspended inside a glass containment cube.
 box(ctx,245,315,325,230,170,'#c79523aa','#60420f88','#33230d88');
 polygon(ctx,[point(245,315,0),point(570,545,0),point(570,545,205),point(245,315,205)],'#f9db6b0d','#ffe18b88');
 const q=point(405,430,116);for(let i=0;i<5;i++){ctx.beginPath();ctx.arc(q.x,q.y,18+i*14,0,Math.PI*2);ctx.strokeStyle=i%2?'#6fffd4':'#ffd76a';ctx.lineWidth=2;ctx.stroke();}
 label(ctx,'QUANTUM CORE / CONTAINED',point(405,315,188),11,'#ffe18b','center',700);
 // Bean bag and snack machines.
 const bean=point(760,625,30);ctx.beginPath();ctx.ellipse(bean.x,bean.y,92,58,-.05,0,Math.PI*2);ctx.fillStyle='#8e5a22';ctx.fill();ctx.strokeStyle='#ffd868';ctx.stroke();
 box(ctx,1030,360,120,90,180,'#b97722','#452711','#2a1c10');monitor(ctx,1040,100,38,74,'POPCORN','#ffd868',3);
 box(ctx,1180,360,120,90,180,'#43a6c0','#17384a','#102535');monitor(ctx,1190,100,38,74,'SLURPIE','#8ae4ff',3);
 // The hotline is always present; its glow is animated in tower-scene.
 box(ctx,884,545,74,54,23,'#b72622','#67120f','#410b0a',38);label(ctx,'CODEX HOTLINE',point(920,545,77),9,'#ff9b8f','center',750);
}
function ceoOffice(ctx,style){
 const metrics=['OBSERVABILITY','FAILURES','COMPLIANCE','BIAS','DRIFT','SECURITY'];metrics.forEach((name,i)=>monitor(ctx,145+i*210,190,35,126,name,style.accent,5));
 tokenDesk(ctx,style,true);plant(ctx,210,710,style.accent,1.4);plant(ctx,1370,710,style.accent,1.4);
 wall(ctx,1260,130,20,145,'#6b4729','#d7b179');label(ctx,'ENTERPRISE AI / CEO',point(1325,170,102),13,'#f2ddbd','center',650);
}
function cfoOffice(ctx,style){
 monitor(ctx,520,520,42,112,'TOKENS SPENT / CODEX ESCALATIONS',style.accent,6);tokenDesk(ctx,style,false);
 label(ctx,'ONE DESK · ONE LEDGER · EVERY ESCALATION EXPLAINED',point(780,170,176),11,'#d5bc92','center',650);plant(ctx,240,690,style.accent);wall(ctx,1160,180,25,130,'#594332','#a68359');
}
function library(ctx,style,archive=false){
 bookcase(ctx,145,270,style,4);bookcase(ctx,1120,290,style,4);bookcase(ctx,480,190,style,archive?5:4);bookcase(ctx,800,190,style,archive?5:4);
 for(const [x,y]of [[310,520],[665,610],[1010,505]]){table(ctx,x,y,230,90,style,45);chair(ctx,x+88,y+112,'#49634f');}
 plant(ctx,220,770,style.accent,1.2);plant(ctx,1360,760,style.accent,1.2);
 label(ctx,archive?'IMMUTABLE ARCHIVE / SOURCE MEMORY':'READING HALL / ACTIVE STUDY',point(785,170,177),12,style.accent,'center',700);
}
function engineering(ctx,style){
 for(let rack=0;rack<7;rack++){box(ctx,160+rack*178,250,120,95,170,'#171b2a','#090d16','#060910');for(let row=0;row<8;row++)line(ctx,point(172+rack*178,346,24+row*17),point(264+rack*178,346,24+row*17),row%3===0?'#a99fff':'#416274',2);}
 for(let i=0;i<12;i++)label(ctx,['fn','{}','=>','01','git','✓'][i%6],point(180+(i*113)%1180,490+(i%3)*85,80+(i%4)*23),16,i%3?'#856fda88':'#9ee6cc99','center',700);
 label(ctx,'ZERO-G CODE CHAMBER / ENGINEERS FLOAT WHILE MODELS RUN',point(785,170,179),11,style.accent,'center',700);
}
function operations(ctx,style){
 wall(ctx,136,1300,118,48,'#071a22','#5fd4c7');for(let i=0;i<12;i++)label(ctx,`${['CTX','TOK','LAT','ERR'][i%4]} ${String(17+i*7).padStart(3,'0')}`,point(165+i*105,170,146),9,i%3?'#7cd5c1':'#e5b76e','center',700);
 for(let row=0;row<3;row++)for(let col=0;col<4;col++){const x=175+col*310,y=320+row*155;table(ctx,x,y,238,74,style,38);monitor(ctx,x+20,198,18,58,['RUNS','TOOLS','QUEUE','GPU'][col],style.accent,3);}
 label(ctx,'LOCAL OPERATIONS EXCHANGE / LIVE ORDERS & QUEUES',point(785,170,184),11,style.accent,'center',700);
}
function conference(ctx,style){
 plane(ctx,170,220,1160,175,18,'#221b21',style.accent);monitor(ctx,320,930,43,118,'ALL-HANDS / VERIFIED WORK ONLY',style.accent,5);
 box(ctx,725,365,120,80,115,style.edge,'#51392e','#30231f',18);label(ctx,'PODIUM',point(785,445,95),10,'#ffe0bd','center',700);
 for(let row=0;row<3;row++)for(let col=0;col<6;col++)chair(ctx,245+col*175,520+row*105,row===0?'#765240':'#4d3c3d');
}
function testing(ctx,style){
 for(let row=0;row<2;row++)for(let col=0;col<3;col++){const x=180+col*405,y=355+row*245;table(ctx,x,y,310,104,style,50);for(let tube=0;tube<6;tube++){const p=point(x+30+tube*42,y+40,92);line(ctx,{x:p.x,y:p.y-28},{x:p.x,y:p.y},tube%2?'#78ebd1':'#c1a5ff',5);ctx.beginPath();ctx.arc(p.x,p.y,5,0,Math.PI*2);ctx.fillStyle=tube%2?'#55cbb8':'#9b7be5';ctx.fill();}}
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
 for(let i=0;i<3;i++){wall(ctx,190+i*1010,160,35,110,'#6b472a','#c29150');label(ctx,['WELCOME','LOCAL MINDS','PRIVATE BY DESIGN'][i],point(270+i*505,170,91),10,'#ebc77f','center',650);}
}

export function renderInterior(ctx,f){
 const style=roomDesigns[String(f.id)];baseArchitecture(ctx,style);
 ctx.canvas.interiorDesign={camera:'oblique-expanded',floor:f.id,rooms:[{id:style.id,material:style.material}],purpose:f.rooms[0]};
 label(ctx,`${String(f.id).padStart(2,'0')} / ${f.name}`,{x:92,y:92},25,'#edf2f3');
 label(ctx,f.rooms[0].toUpperCase(),{x:92,y:118},11,style.accent,'left',700);
 label(ctx,'OPEN-FRONT / DEEP FLOOR PLATE',{x:1490,y:86},11,'#90a7b3','right');
 ({codex:codexSuite,ceo:ceoOffice,cfo:cfoOffice,archive:(c,s)=>library(c,s,true),library,engineering,operations,conference,testing,lobby}[style.id]||(()=>{}))(ctx,style);
 if(style.id==='recharge')label(ctx,'VISIBLE POD STATUS / SLEEPING GENTS REMAIN IDENTIFIABLE',point(785,170,179),11,style.accent,'center',700);
 // Transparent perimeter retains depth while keeping every room observable.
 polygon(ctx,[point(126,170,0),point(126,880,0),point(126,880,190),point(126,170,190)],'#bde8f006','#8ab4c244');
 polygon(ctx,[point(1446,170,0),point(1446,880,0),point(1446,880,190),point(1446,170,190)],'#bde8f006','#8ab4c244');
 line(ctx,point(126,880,4),point(1446,880,4),'#d1ba8877',2);
}
