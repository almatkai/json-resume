#!/usr/bin/env python3
"""Verify locked registry archive SHA256s and copy authentic upstream notices."""
import hashlib,json,pathlib,subprocess,tarfile,tomllib,re,shutil
ROOT=pathlib.Path(__file__).resolve().parents[1];cargo=ROOT/'.qa/cargo-home'
lock=tomllib.loads((ROOT/'Cargo.lock').read_text());out=ROOT/'resources/licenses';out.mkdir(exist_ok=True)
rows=[];gaps=[]
for p in lock['package']:
 if not p.get('source','').startswith('registry+'):continue
 name=p['name']+'-'+p['version'];archives=list((cargo/'registry/cache').glob('*/'+name+'.crate'))
 if len(archives)!=1:raise RuntimeError('Missing locked archive '+name)
 archive=archives[0];digest=hashlib.sha256(archive.read_bytes()).hexdigest();assert digest==p['checksum'],name
 with tarfile.open(archive)as t:
  meta=tomllib.loads(t.extractfile(name+'/Cargo.toml').read().decode());package=meta['package'];found=[]
  for member in t.getmembers():
   if not member.isfile():continue
   path=pathlib.PurePosixPath(member.name);rel=pathlib.PurePosixPath(*path.parts[1:]);base=path.name.lower()
   if any(part in ('..','')for part in rel.parts):raise RuntimeError('Bad archive path')
   if re.match(r'^(licen[sc]e|copying|notice|authors|copyright)([.\-_].*)?$',base):
    dst=out/name/rel;dst.parent.mkdir(parents=True,exist_ok=True);data=t.extractfile(member).read();dst.write_bytes(data);found.append({'file':str(rel),'sha256':hashlib.sha256(data).hexdigest()})
  row={'name':p['name'],'version':p['version'],'archiveSha256':digest,'license':package.get('license'),'rustVersion':package.get('rust-version'),'notices':found};rows.append(row)
  if not found:gaps.append(name+' declares '+str(package.get('license'))+' but archive has no standalone license/notice grant file')
# Exact npm schema grants and validator regexp attribution, plus pinned SDK grant.
v1=ROOT/'.qa/v1/node_modules'
for package,filename,label in [('@jsonresume/schema','LICENSE.md','jsonresume-schema-1.2.1'),('jsonschema','LICENSE','node-jsonschema-1.5.0')]:
 src=v1/package/filename;dst=out/label/filename;dst.parent.mkdir(exist_ok=True);dst.write_bytes(src.read_bytes())
repo='/Users/almat/Desktop/code/Tezbar APP/rust-extension-ports/tezbar-extension-sdk-rust';rev='e955911f0bf5b558f9d3eb5183df0adc95c22af2'
for name in ['LICENSE','NOTICE']:
 dst=out/'tezbar-extension-sdk-0.1.1'/name;dst.parent.mkdir(exist_ok=True);dst.write_bytes(subprocess.check_output(['git','-C',repo,'show',rev+':'+name]))
# Host code is retained solely as immutable QA fixtures, under its exact grant.
repo='/Users/almat/Desktop/code/Tezbar APP/Tezbar';rev='ea100cee3f69711de12485ba061950c5fc65258f'
(ROOT/'qa/pinned/HOST-LICENSE').write_bytes(subprocess.check_output(['git','-C',repo,'show',rev+':LICENSE']))
(ROOT/'resources/dependency-notices.json').write_text(json.dumps({'registry':rows,'evidenceGaps':gaps},indent=2)+'\n')
print(json.dumps({'lockedRegistryArchivesVerified':len(rows),'noticeFiles':sum(len(r['notices'])for r in rows),'evidenceGaps':gaps,'rEfiAuthors':[r['notices']for r in rows if r['name']=='r-efi']}))
