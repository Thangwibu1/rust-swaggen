// Native development plugin: read/export only, no network, no canvas mutations.
figma.showUI(__html__, {width: 480, height: 430});
let running = false;
let cancelled = false;
const clean = value => value.toLowerCase().replace(/[^a-z0-9]+/g, '-').replace(/^-|-$/g, '').slice(0,100) || 'node';
const emit = value => figma.ui.postMessage(value);
function folder(page) {
  const match = page.name.match(/^(\d+)\s*[•·]?\s*(.*)/);
  return match ? match[1] + '-' + clean(match[2]) : clean(page.name);
}
function collect(page) {
  const nodes = new Map();
  const add = node => { if ('exportAsync' in node) nodes.set(node.id,node); };
  for (const child of page.children) {
    if (['FRAME','COMPONENT','COMPONENT_SET','GROUP'].includes(child.type)) add(child);
    if (child.type === 'SECTION') {
      const queue = [...child.children];
      while(queue.length) {
        const node = queue.shift();
        if (node.type==='SECTION') queue.push(...node.children);
        else if (['FRAME','COMPONENT','COMPONENT_SET','GROUP'].includes(node.type)) add(node);
      }
    }
  }
  // Component definitions and their variants, without exporting every UI instance.
  const stack = [...page.children];
  while(stack.length) {
    const node=stack.pop();
    if (node.type==='COMPONENT' || node.type==='COMPONENT_SET') add(node);
    if ('children' in node && node.type!=='INSTANCE') stack.push(...node.children);
  }
  return [...nodes.values()];
}
function details(root) {
  const layers=[];const stack=[root];
  while(stack.length) {
    const node=stack.pop();
    const row={id:node.id,name:node.name,type:node.type,visible:node.visible};
    for(const key of ['x','y','width','height','layoutMode','itemSpacing','paddingLeft','paddingRight','paddingTop','paddingBottom','cornerRadius','fills','strokes','effects','boundVariables']) {
      if(key in node) { try { const value=node[key]; if(typeof value!=='symbol')row[key]=value; }catch(_){} }
    }
    if(node.type==='TEXT') {
      row.characters=node.characters;
      for(const key of ['fontName','fontSize','lineHeight','letterSpacing'])if(typeof node[key]!=='symbol')row[key]=node[key];
    }
    if(node.type==='COMPONENT' || node.type==='COMPONENT_SET') {
      row.componentPropertyDefinitions=node.componentPropertyDefinitions;
      if(node.type==='COMPONENT')row.variantProperties=node.variantProperties;
    }
    if(node.type==='INSTANCE')row.componentProperties=node.componentProperties;
    if('children' in node){row.children=node.children.map(n=>n.id);stack.push(...[...node.children].reverse());}
    layers.push(row);
  }
  return layers;
}
figma.ui.onmessage=async msg=>{
  if(msg.type==='cancel'){cancelled=true;return;}
  if(msg.type!=='start' || running)return;
  running=true;cancelled=false;
  const results=[];const errors=[];let completed=0;
  try {
    for(const page of figma.root.children) {
      if(cancelled)break;
      await page.loadAsync();
      const slug=folder(page);const nodes=collect(page);
      emit({type:'page-start',page:page.name,total:nodes.length});
      const index=[];
      for(const node of nodes) {
        if(cancelled)break;
        const isComponent=node.type==='COMPONENT'||node.type==='COMPONENT_SET'||slug.startsWith('01-');
        const category=isComponent?'components':(slug.startsWith('00-')||slug.startsWith('07-')?'reference':'screens');
        const path=`02-design/${category}/${slug}/${clean(node.name)}__${node.id.replace(/:/g,'-')}`;
        const record={id:node.id,name:node.name,page:slug,path,files:[]};
        try {
          const png=await node.exportAsync({format:'PNG',constraint:{type:'SCALE',value:1}});
          emit({type:'file',path:path+'.png',bytes:png});record.files.push(path+'.png');
          if(isComponent) {
            const svg=await node.exportAsync({format:'SVG'});
            emit({type:'file',path:path+'.svg',bytes:svg});record.files.push(path+'.svg');
          }
          emit({type:'text',path:path+'.native.json',text:JSON.stringify({node:record,layers:details(node)},null,2)});
        }catch(error){errors.push({id:node.id,page:slug,error:String(error)});}
        results.push(record);index.push(record);completed++;
        emit({type:'progress',completed,name:node.name});
      }
      emit({type:'text',path:`02-design/native-manifests/${slug}.json`,text:JSON.stringify({page:page.name,id:page.id,nodes:index},null,2)});
      // One archive per page bounds memory. No external ZIP library/network.
      emit({type:'page-complete',page:slug});
    }
    emit({type:'text',path:'02-design/native-export-results.json',text:JSON.stringify({completed,cancelled,results,errors},null,2)});
    emit({type:'finished',completed,cancelled,errors});
  }catch(error){emit({type:'fatal',error:String(error)});}
  finally{running=false;}
};
