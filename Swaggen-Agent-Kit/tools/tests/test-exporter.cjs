// Tests control flow against a mock; does not establish Figma runtime acceptance.
const fs=require('fs'),vm=require('vm'),assert=require('assert'),cp=require('child_process'),path=require('path');
const root=path.resolve(__dirname,'../..');
const pages=JSON.parse(cp.execFileSync('python',[path.join(__dirname,'exporter-fixture.py')],{encoding:'utf8',maxBuffer:32*1024*1024}));
const messages=[];
function prepare(node){
 node.loadAsync=async()=>{};
 node.exportAsync=async()=>Uint8Array.from([1,2,3]);
 if(node.type==='TEXT'){node.characters=node.name;delete node.children;}
 else if(node.children)node.children.forEach(prepare);
 if(node.type==='COMPONENT'||node.type==='COMPONENT_SET')node.componentPropertyDefinitions={};
}
pages.forEach(prepare);
const figma={root:{children:pages},showUI(){},ui:{postMessage(m){messages.push(m);}}};
const context=vm.createContext({figma,__html__:'mock UI',Uint8Array,Map,JSON,Error});
vm.runInContext(fs.readFileSync(path.join(root,'tools/figma-bulk-export/code.js'),'utf8'),context);
(async()=>{
 await figma.ui.onmessage({type:'start'});
 const report=messages.find(x=>x.type==='finished');assert(report,'No finished report');assert.equal(report.errors.length,0);assert.equal(report.cancelled,false);
 const inventory=JSON.parse(fs.readFileSync(path.join(root,'02-design/node-inventory.json')));
 const manifest=JSON.parse(messages.find(x=>x.type==='text'&&x.path==='02-design/native-export-results.json').text);
 const ids=new Set(manifest.results.map(x=>x.id));
 for(const node of inventory)assert(ids.has(node.id),'Missing target '+node.id);
 assert.equal(ids.size,inventory.length);
 assert.equal(messages.filter(x=>x.type==='page-complete').length,15);
 const summary={test:'mock-native-export-control-flow',targets:ids.size,pages:15,errors:0,native_figma_runtime_tested:false};
 console.log(JSON.stringify(summary,null,2));
})().catch(error=>{console.error(error);process.exitCode=1;});
