// Calibrated to the 1600x1000 room photographs. All positions are feet on
// the floor, not portrait centers. A single journey per room prevents agents
// from walking through one another; furnishings own their interaction anchors.
const station=(x,y,h,target,path,clipY=null)=>({chair:{x,y,h,clipY},target,path});
export const roomStations={
 10:[station(701,482,114,{id:'overview-wall',label:'Reviewing company overview',x:1100,y:474},[[1010,485],[1100,474]],482)],
 9:[station(833,569,120,{id:'finance-books',label:'Checking the budget ledger',x:1260,y:700},[[1215,680],[1260,700]],569)],
 8:[station(787,672,105,{id:'archive-catalog',label:'Retrieving an indexed context record',x:1130,y:640},[[1125,700],[1130,640]],672)],
 7:[station(542,545,100,{id:'reference-stacks',label:'Reading shared procedures',x:665,y:434},[[590,575],[665,434]],525)],
 6:[
  station(426,735,115,{id:'tool-cart',label:'Inspecting the engineering toolkit',x:620,y:810},[[440,835],[620,810]]),
  station(498,535,94,{id:'server-rack',label:'Checking local runtime diagnostics',x:739,y:456},[[720,590],[739,456]]),
  station(1181,563,104,{id:'engineering-display',label:'Reviewing implementation feedback',x:967,y:605},[[1150,650],[967,605]]),
  station(1301,738,123,{id:'review-table',label:'Comparing the next bounded experiment',x:1030,y:832},[[1340,840],[1030,832]]),
 ],
 5:[
  station(311,792,118,{id:'routing-terminal',label:'Reviewing agent routing',x:731,y:813},[[475,884],[731,813]]),
  station(521,817,122,{id:'context-terminal',label:'Inspecting governed context transfers',x:731,y:735},[[665,900],[731,735]]),
  station(1158,823,120,{id:'operations-video-wall',label:'Inspecting live failure telemetry',x:969,y:750},[[1040,900],[969,750]]),
  station(1393,783,116,{id:'capacity-terminal',label:'Checking measured GPU capacity',x:1042,y:832},[[1390,895],[1042,832]]),
 ],
 3:[station(588,739,93,{id:'conference-podium',label:'Preparing the company review',x:793,y:424},[[486,755],[487,476],[793,424]])],
 2:[
  station(486,587,99,{id:'test-instruments',label:'Inspecting a local evaluation result',x:779,y:563},[[735,655],[779,563]]),
  station(1098,590,99,{id:'regression-console',label:'Checking the regression instrument',x:790,y:703},[[807,648],[790,703]]),
 ],
};
const interpolate=(a,b,t)=>({x:a.x+(b.x-a.x)*t,y:a.y+(b.y-a.y)*t});
export function traverse(points,progress){
 const lengths=points.slice(1).map((p,i)=>Math.hypot(p.x-points[i].x,p.y-points[i].y)),total=lengths.reduce((a,b)=>a+b,0);
 let remaining=Math.max(0,Math.min(1,progress))*total;
 for(let i=0;i<lengths.length;i++){if(remaining<=lengths[i])return interpolate(points[i],points[i+1],lengths[i]?remaining/lengths[i]:0);remaining-=lengths[i];}
 return points.at(-1);
}
export function roomActivity(floor,seat,time,{occupants=1,reducedMotion=false}={}){
 const stations=roomStations[floor];if(!stations?.length)return null;
 const config=stations[seat%stations.length],count=Math.max(1,occupants),clock=Math.max(0,time)/1000,owner=Math.floor(clock/40)%count,phase=clock%40;
 const resting=reducedMotion||owner!==seat||phase>=25;
 if(resting)return {...config.chair,state:'seated',object:'chair',activity:'Seated at assigned workstation',stride:0};
 // Chair anchors are seated hips, not standing feet. Preserve head/face scale
 // when rising instead of shrinking the whole character and floating it.
 const standingHeight=config.chair.h/.73,standing={x:config.chair.x,y:config.chair.y+standingHeight*(1-.73)};
 const points=[standing,...config.path.map(([x,y])=>({x,y}))],returning=phase>=20;
 const moving=phase<6||returning,progress=returning?1-(phase-20)/5:phase/6;
 const position=moving?traverse(points,progress):config.target;
 const h=Math.max(100,Math.min(195,standingHeight+(position.y-standing.y)*.1));
 const clipY=[9,10].includes(floor)&&position.x<(floor===9?1210:1005)?config.chair.clipY:null;
 return {...position,h,clipY,state:moving?'walking':'interacting',object:moving?'walkway':config.target.id,activity:moving?(returning?'Returning to assigned chair':`Walking to ${config.target.id.replaceAll('-',' ')}`):config.target.label,stride:moving?Math.sin(clock*9)*.13:0};
}

// Use exactly the same image-space projection in the exterior cutaway and
// detailed room, so occupants no longer jump to arbitrary window positions.
export function projectRoomPoint(rect,p){
 const scale=(rect.w-6)/1470,sourceHeight=(rect.h-10)/scale,top=Math.max(110,500-sourceHeight/2);
 return {x:rect.x+3+(p.x-45)*scale,y:rect.y+4+(p.y-top)*scale,h:p.h*scale};
}
