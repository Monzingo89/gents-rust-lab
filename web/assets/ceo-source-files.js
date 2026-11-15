// Reads only files explicitly selected by the owner. Server validation remains authoritative.
export async function prepareSourceFiles(files) {
 const selected = Array.from(files);
 if (selected.length > 8 || selected.reduce((n,f)=>n+f.size,0)>32768) throw Error('Select at most eight JSON files, 32768 bytes total');
 const paths=new Set(), uploads=[];
 for (const file of selected) {
  if (!/^[A-Za-z0-9][A-Za-z0-9_.-]*\.json$/.test(file.name)||file.name.length>120||file.size>8192||paths.has(file.name)) throw Error('Use unique plain .json filenames, at most 8192 bytes each');
  const id=`attached-${uploads.length+1}-${file.name.replace(/[^A-Za-z0-9_-]/g,'-')}`.slice(0,40);
  if(uploads.some(f=>f.id===id))throw Error('Attached input IDs collide; rename the files');
  const text=new TextDecoder('utf-8',{fatal:true,ignoreBOM:true}).decode(await file.arrayBuffer());
  // Preserve exact UTF-8 bytes, including BOM (the server rejects unsupported JSON).
  paths.add(file.name);uploads.push({id,path:file.name,text});
 }
 return uploads;
}
