"""Download Figma export responses; input JSON is private and never shipped."""
import json, sys, hashlib, time, subprocess, concurrent.futures
from pathlib import Path
ROOT = Path(__file__).resolve().parents[1]
def fetch(job):
    url, path = job
    path.parent.mkdir(parents=True, exist_ok=True)
    if path.exists() and path.stat().st_size: return str(path.relative_to(ROOT))
    for attempt in range(3):
        try:
            result = subprocess.run(['curl','--silent','--show-error','--fail','--location','--max-time','25',url],capture_output=True,timeout=30)
            result.check_returncode()
            data=result.stdout
            if not data: raise ValueError('Empty download')
            path.write_bytes(data)
            return str(path.relative_to(ROOT))
        except Exception:
            if attempt==2: raise
            time.sleep(attempt+1)
def main():
    batch=json.loads(Path(sys.argv[1]).read_text())
    jobs=[]; entries=[]
    for item in batch:
        info=item['assets']; meta=item['node']; export=info.get('export',{})
        paths=[]
        if export.get('url'):
            target=ROOT/(meta['path']+'.'+export.get('format','png'));jobs.append((export['url'],target));paths.append(str(target.relative_to(ROOT)))
        for kind in ['rawImages','svgAssets']:
            if kind=='svgAssets' and '/components/' not in '/'+meta['path']: continue
            for index,asset in enumerate(info.get(kind,[])):
                ext=asset.get('format','bin'); ext='bin' if ext=='original' else ext
                target=ROOT/(meta['path']+f'__{kind}-{index:02d}.'+ext)
                jobs.append((asset['url'],target));paths.append(str(target.relative_to(ROOT)))
        entries.append({**meta,'files':paths,'svg_truncated':info.get('svgAssetsTruncated',False),'raw_images_truncated':info.get('rawImagesTruncated',False)})
    failures=[]
    with concurrent.futures.ThreadPoolExecutor(max_workers=20) as pool:
        future_map={pool.submit(fetch,j):j for j in jobs}
        for future in concurrent.futures.as_completed(future_map):
            try: future.result()
            except Exception as e: failures.append({'path':str(future_map[future][1].relative_to(ROOT)),'error':str(e)})
    index_path=ROOT/'02-design/export-results.json'
    previous=json.loads(index_path.read_text()) if index_path.exists() else []
    by_id={x['id']:x for x in previous}
    by_id.update({x['id']:x for x in entries})
    index_path.write_text(json.dumps(list(by_id.values()),ensure_ascii=False,indent=2))
    print(json.dumps({'nodes':len(entries),'files':len(jobs),'failures':failures},ensure_ascii=False))
    if failures: sys.exit(1)
if __name__=='__main__':main()
