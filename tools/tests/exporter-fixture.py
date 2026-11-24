import json,sys,xml.etree.ElementTree as E
from pathlib import Path
root=Path(__file__).resolve().parents[2]
def convert(e):
    kinds={'canvas':'PAGE','frame':'FRAME','symbol':'COMPONENT','text':'TEXT','instance':'INSTANCE','group':'GROUP'}
    kind=kinds.get(e.tag,e.tag.upper())
    if e.tag=='frame' and any(x.tag=='symbol' for x in e):kind='COMPONENT_SET'
    return {'id':e.get('id'),'name':e.get('name'),'type':kind,'visible':True,'children':[convert(x) for x in e]}
json.dump([convert(E.parse(f).getroot()) for f in sorted((root/'02-design/metadata').glob('*.xml'))],sys.stdout)
