#!/usr/bin/env python3
"""Independent source/pin/candidate audit. Never writes the Git index."""
import hashlib,json,pathlib,subprocess
ROOT=pathlib.Path(__file__).resolve().parents[1];REPO=ROOT.parent
source='6cb8e33954fdc5da52af9993d92c6fa7b37e7c79';sha=lambda b:hashlib.sha256(b).hexdigest()
def git(*args):return subprocess.check_output(['git',*args],cwd=REPO)
assert git('rev-parse','--show-toplevel').decode().strip()==str(REPO)
assert git('rev-parse','HEAD').decode().strip()==source
assert git('branch','--show-current').decode().strip()=='feat/native-v2-rust'
assert git('diff','--cached','--name-only')==b''
original=git('ls-tree','-r','--name-only',source).decode().splitlines();audit=[]
for f in original:
 before=git('show',source+':'+f);after=(REPO/f).read_bytes()
 if f=='README.md':assert after==before+b'Standalone Rust V2: [implementation and verification](v2/README.md).\n'
 else:assert after==before,f
 audit.append({'path':f,'sourceSha256':sha(before),'unchanged':f!='README.md'})
pins=[]
for p in sorted((ROOT/'qa/pinned').iterdir()):
 if p.name.startswith('sdk-'):repo=REPO.parent/'tezbar-extension-sdk-rust';rev='e955911f0bf5b558f9d3eb5183df0adc95c22af2';f=p.name[4:].replace('__','/')
 elif p.name.startswith('host-'):repo=REPO.parents[1]/'Tezbar';rev='ea100cee3f69711de12485ba061950c5fc65258f';f=p.name[5:].replace('__','/')
 elif p.name=='HOST-LICENSE':repo=REPO.parents[1]/'Tezbar';rev='ea100cee3f69711de12485ba061950c5fc65258f';f='LICENSE'
 else:raise AssertionError(p)
 data=subprocess.check_output(['git','-C',str(repo),'show',rev+':'+f]);assert data==p.read_bytes();pins.append({'fixture':str(p.relative_to(REPO)),'commit':rev,'object':f,'sha256':sha(data)})
icon=(ROOT/'resources/extension-icon.png').read_bytes();assert icon==git('show',source+':assets/extension-icon.png');assert icon==(ROOT/'.qa/upstream-icon-final.png').read_bytes();assert sha(icon)=='038c492799df020c53b8bd0ffb6f84e664e8caae1de571496ba5591087b71d45'
assert (ROOT/'resources/resume-schema.json').read_bytes()==(ROOT/'.qa/v1/node_modules/@jsonresume/schema/schema.json').read_bytes()
assert (REPO/'package-lock.json').read_bytes()==(ROOT/'.qa/v1/package-lock.json').read_bytes()
assert (ROOT/'resources/licenses/r-efi-6.0.0/AUTHORS').read_bytes().find(b'Permission is hereby granted')>=0
# A Git-visible fixture inventory: ignored outputs/caches can never enter this candidate.
candidates=sorted(set(git('diff','--name-only').decode().splitlines()+git('ls-files','--others','--exclude-standard').decode().splitlines()))
assert all(f=='README.md'or f.startswith('v2/')for f in candidates)
assert all('/.qa/'not in f and '/node_modules/'not in f and '/target/'not in f and not f.endswith('.zip')for f in candidates)
for f in candidates:
 assert not (REPO/f).is_symlink();assert (REPO/f).is_file()
assert 'v2/qa/oracles.json'in candidates
inventory=[{'path':f,'sha256':sha((REPO/f).read_bytes()),'mode':oct((REPO/f).stat().st_mode&0o777)}for f in candidates if f!='v2/CANDIDATE-SHA256.json']
(ROOT/'CANDIDATE-SHA256.json').write_text(json.dumps(inventory,indent=2)+'\n')
report={'sourceHead':source,'branch':'feat/native-v2-rust','index':'empty','sourceFiles':audit,'pinnedObjects':pins,'candidateFilesExcludingInventory':len(inventory),'iconSha256':sha(icon)}
(ROOT/'.qa/audit.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps({k:v for k,v in report.items()if k not in ['sourceFiles','pinnedObjects']}|{'V1FilesAudited':len(audit),'pinnedObjectsVerified':len(pins)}))
