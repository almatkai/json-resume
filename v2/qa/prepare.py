#!/usr/bin/env python3
"""Prepare only clone-owned immutable fixtures. Requires adjacent read-only Git objects."""
import pathlib,subprocess,json
ROOT=pathlib.Path(__file__).resolve().parents[1];REPO=ROOT.parent
assert subprocess.check_output(['git','rev-parse','--show-toplevel'],cwd=REPO,text=True).strip()==str(REPO)
source='6cb8e33954fdc5da52af9993d92c6fa7b37e7c79'
v1=ROOT/'.qa/v1';v1.mkdir(parents=True,exist_ok=True)
for f in subprocess.check_output(['git','ls-tree','-r','--name-only',source],cwd=REPO,text=True).splitlines():
 if f.startswith('metadata/'):continue
 p=v1/f;p.parent.mkdir(parents=True,exist_ok=True);p.write_bytes(subprocess.check_output(['git','show',source+':'+f],cwd=REPO))
# Every snapshot is reproduced from an immutable object, never a working file.
for p in (ROOT/'qa/pinned').iterdir():
 if p.name.startswith('sdk-'):repo=REPO.parent/'tezbar-extension-sdk-rust';rev='e955911f0bf5b558f9d3eb5183df0adc95c22af2';f=p.name[4:].replace('__','/')
 elif p.name.startswith('host-'):repo=REPO.parents[1]/'Tezbar';rev='ea100cee3f69711de12485ba061950c5fc65258f';f=p.name[5:].replace('__','/')
 elif p.name=='HOST-LICENSE':repo=REPO.parents[1]/'Tezbar';rev='ea100cee3f69711de12485ba061950c5fc65258f';f='LICENSE'
 else:continue
 assert p.read_bytes()==subprocess.check_output(['git','-C',str(repo),'show',rev+':'+f]),f
for f in ['src/renderer/ExtensionRuntimeView.tsx','src/shared/extensionRuntime.ts','src/renderer/src/raycast-api/index.tsx']:
 dest=ROOT/'.qa/patched'/f;dest.parent.mkdir(parents=True,exist_ok=True);dest.write_bytes((ROOT/'qa/pinned'/('host-'+f.replace('/','__'))).read_bytes())
subprocess.run(['git','apply','--directory=v2/.qa/patched','v2/qa/host-prerequisites.patch'],cwd=REPO,check=True)
host=ROOT/'.qa/host-ui';host.mkdir(exist_ok=True)
(host/'package.json').write_bytes((ROOT/'qa/host-ui-package.json').read_bytes());(host/'package-lock.json').write_bytes((ROOT/'qa/host-ui-package-lock.json').read_bytes())
print(json.dumps({'fixtureGitObjects':'verified','V1':'disposable exact-source copy','hostPatch':'applied only to owned fixtures'}))
