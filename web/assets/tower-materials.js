// Deterministic architectural materials. No model calls or fabricated telemetry.
const tiles=new Map();
const noise=(x,y)=>{const n=Math.sin(x*127.1+y*311.7)*43758.5453;return n-Math.floor(n);};
const rgb=color=>{if(color.startsWith('rgb'))return color.match(/[\d.]+/g).slice(0,3).map(Number);let hex=color.replace('#','').slice(0,6);if(hex.length===3)hex=[...hex].map(c=>c+c).join('');return [0,2,4].map(i=>parseInt(hex.slice(i,i+2),16));};
export function surfaceCanvas(width=1600,height=1000){
 if(typeof OffscreenCanvas!=='undefined')return new OffscreenCanvas(width,height);
 if(typeof document==='undefined')return null;
 const canvas=document.createElement('canvas');canvas.width=width;canvas.height=height;return canvas;
}
export function finish(color,delta=0,alpha=1){const c=rgb(color);if(color.length===9)alpha*=parseInt(color.slice(7,9),16)/255;return `rgba(${c.map(v=>Math.max(0,Math.min(255,Math.round(v+delta)))).join(',')},${alpha})`;}
export function shaded(ctx,from,to,color,light=16,dark=-22){
 const gradient=ctx.createLinearGradient(from.x,from.y,to.x,to.y);gradient.addColorStop(0,finish(color,light));gradient.addColorStop(.48,finish(color,0));gradient.addColorStop(1,finish(color,dark));return gradient;
}
export function material(ctx,kind,color){
 const key=`${kind}:${color}`;
 if(!tiles.has(key)){
  const tile=surfaceCanvas(256,256);if(!tile)return color;
  const pen=tile.getContext('2d'),pixels=pen.createImageData(256,256),base=rgb(color);
  for(let y=0;y<256;y++)for(let x=0;x<256;x++){
   const grain=noise(x,y),soft=noise(Math.floor(x/12),Math.floor(y/9));let shade;
   if(kind==='wood')shade=Math.sin(y*.76+Math.sin(x*.026)*2.4+Math.sin(y*.09)*1.8)*7+grain*9-4;
   else if(kind==='marble'){
    const vein=Math.sin(x*.031+y*.021+Math.sin(y*.04)*1.9+Math.sin(x*.015)*2.2);
    shade=(Math.abs(vein)>.96?-29:Math.sin(x*.016+y*.024)*6)+grain*7-3;
   }else if(kind==='carpet')shade=(x%2?1:-1)*3+(y%3?0:-3)+grain*13-6;
   else if(kind==='metal')shade=Math.sin(y*3.1)*5+grain*7+soft*3-5;
   else shade=grain*10+soft*5-7;
   const i=(y*256+x)*4;for(let c=0;c<3;c++)pixels.data[i+c]=Math.max(0,Math.min(255,base[c]+shade));pixels.data[i+3]=255;
  }
  pen.putImageData(pixels,0,0);tiles.set(key,tile);
 }
 return ctx.createPattern(tiles.get(key),'repeat')||color;
}
export function floorMaterial(style){
 if(['lobby','codex','ceo'].includes(style.id))return 'marble';
 if(['library','archive','cfo'].includes(style.id))return 'wood';
 if(['conference','operations','engineering'].includes(style.id))return 'carpet';
 return 'stone';
}
