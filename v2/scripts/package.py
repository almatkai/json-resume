#!/usr/bin/env python3
"""Deterministic development ZIP; never publishes or installs."""
import hashlib,json,pathlib,stat,sys,zipfile,tempfile
ROOT=pathlib.Path(__file__).resolve().parents[1]
binary=pathlib.Path(sys.argv[1]).resolve();assert binary.is_relative_to(ROOT/'.qa')
sha=lambda b:hashlib.sha256(b).hexdigest()
files={'bin/json-resume-v2':(binary.read_bytes(),0o755)}
for name in ['LICENSE','NOTICE']:
 files[name]=((ROOT/name).read_bytes(),0o644)
for p in sorted((ROOT/'resources').rglob('*')):
 if p.is_file():assert not p.is_symlink();files[p.relative_to(ROOT).as_posix()]=(p.read_bytes(),0o644)
manifest=json.loads((ROOT/'extension.json').read_text());assert manifest['artifacts']=={}
manifest['files']={name:sha(data)for name,(data,mode)in sorted(files.items())}
files['extension.json']=((json.dumps(manifest,indent=2)+'\n').encode(),0o644)
def archive(path):
 with zipfile.ZipFile(path,'w',compression=zipfile.ZIP_DEFLATED,compresslevel=9)as z:
  for name,(data,mode)in sorted(files.items()):
   info=zipfile.ZipInfo(name,(2020,1,1,0,0,0));info.create_system=3;info.external_attr=(stat.S_IFREG|mode)<<16;info.compress_type=zipfile.ZIP_DEFLATED;z.writestr(info,data,compresslevel=9)
qa=ROOT/'.qa';a=qa/'json-resume-v2-aarch64-apple-darwin-dev.zip';b=qa/'json-resume-v2-repeat.zip';archive(a);archive(b);assert a.read_bytes()==b.read_bytes()
out=pathlib.Path(tempfile.mkdtemp(dir=qa,prefix='extracted-'))
with zipfile.ZipFile(a)as z:
 assert set(z.namelist())==set(files)
 for info in z.infolist():
  assert info.filename in files
  data=z.read(info);expected,mode=files[info.filename];assert data==expected;assert (info.external_attr>>16)&0o777==mode
  dst=out/info.filename;dst.parent.mkdir(parents=True,exist_ok=True);dst.write_bytes(data);dst.chmod(mode)
report={'archive':str(a.relative_to(ROOT)),'sha256':sha(a.read_bytes()),'binarySha256':sha(binary.read_bytes()),'extracted':str(out.relative_to(ROOT)),'files':[{'path':n,'sha256':sha(d),'mode':oct(m)}for n,(d,m)in sorted(files.items())]}
(qa/'package-inventory.json').write_text(json.dumps(report,indent=2)+'\n')
print(json.dumps({k:v for k,v in report.items()if k!='files'}|{'fileCount':len(files)}))
print(out/'bin/json-resume-v2')
