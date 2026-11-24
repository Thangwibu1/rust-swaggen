"""Validate local handoff completeness. Metadata success never implies image completion."""
import argparse, json, re, sys, hashlib, xml.etree.ElementTree as E
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
def main():
    parser=argparse.ArgumentParser();parser.add_argument('--require-all-images',action='store_true');args=parser.parse_args()
    errors=[]
    req=json.loads((ROOT/'01-prd/requirements.json').read_text(encoding='utf-8'))
    trace=json.loads((ROOT/'04-phases/traceability.json').read_text(encoding='utf-8'))
    inventory=json.loads((ROOT/'02-design/node-inventory.json').read_text(encoding='utf-8'))
    if len(req)!=160 or len({r['id'] for r in req})!=160:errors.append('Expected 160 unique requirements')
    if {r['id'] for r in req}!={r['id'] for r in trace}:errors.append('Traceability requirement mismatch')
    for r in trace:
        if not r.get('phase') or not r.get('screen_ids'):errors.append('Missing phase/screen map: '+r['id'])
        if not r.get('description'):errors.append('Missing requirement behavior: '+r['id'])
    for item in inventory:
        for extension in ['.json','.md']:
            if not (ROOT/(item['path']+extension)).exists():errors.append('Missing metadata: '+item['id']+extension)
    images=list((ROOT/'02-design').rglob('*.png'))
    image_ids={}
    for image in images:
        match=re.search(r'__(\d+-\d+)\.png$',image.name)
        if match:image_ids.setdefault(match.group(1).replace('-',':'),[]).append(str(image.relative_to(ROOT)))
        if image.read_bytes()[:8]!=b'\x89PNG\r\n\x1a\n':errors.append('Invalid PNG signature: '+str(image))
    for svg in (ROOT/'02-design').rglob('*.svg'):
        try:E.parse(svg)
        except E.ParseError:errors.append('Invalid SVG XML: '+str(svg))
    missing=[{'id':x['id'],'name':x['name'],'page':x['page'],'expected_path':x['path']+'.png'} for x in inventory if x['id'] not in image_ids]
    (ROOT/'02-design/missing-images.json').write_text(json.dumps(missing,ensure_ascii=False,indent=2),encoding='utf-8')
    native=ROOT/'02-design/native-export-results.json'
    if native.exists():
        report=json.loads(native.read_text(encoding='utf-8'))
        if report.get('errors') or report.get('cancelled'):errors.append('Native export report is partial/failed')
    if args.require_all_images and missing:errors.append(f'Missing {len(missing)} rendered targets')
    summary={'requirements':len(req),'mapped_requirements':sum(bool(r['screen_ids']) for r in trace),'phases':len(list((ROOT/'04-phases').glob('P*.md'))),'pages':len(list((ROOT/'02-design/metadata').glob('*.xml'))),'export_targets':len(inventory),'rendered_targets':sum(x['id'] in image_ids for x in inventory),'missing_images':len(missing),'png_files':len(images),'svg_files':len(list((ROOT/'02-design').rglob('*.svg'))),'documentation_valid':not any('Missing metadata' in e or 'requirement' in e.lower() for e in errors),'all_images_complete':not missing,'errors':errors}
    (ROOT/'06-quality/kit-validation.json').write_text(json.dumps(summary,ensure_ascii=False,indent=2),encoding='utf-8')
    print(json.dumps(summary,ensure_ascii=False,indent=2))
    return 1 if errors else 0
if __name__=='__main__':sys.exit(main())
