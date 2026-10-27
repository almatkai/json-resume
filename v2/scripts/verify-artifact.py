#!/usr/bin/env python3
"""Read-only audit of the latest owned development ZIP and HTTPS SDK checkout."""
import hashlib, json, pathlib, stat, subprocess, tomllib, zipfile

ROOT = pathlib.Path(__file__).resolve().parents[1]
QA = ROOT / '.qa'
sha = lambda b: hashlib.sha256(b).hexdigest()
report = json.loads((QA / 'package-inventory.json').read_text())
archive = ROOT / report['archive']
extracted = ROOT / report['extracted']
assert archive.is_relative_to(QA) and extracted.is_relative_to(QA)
binary = extracted / 'bin/json-resume-v2'
assert sha(archive.read_bytes()) == report['sha256']
assert archive.read_bytes() == (QA / 'json-resume-v2-repeat.zip').read_bytes()
assert sha(binary.read_bytes()) == report['binarySha256']
expected = {'bin/json-resume-v2', 'extension.json', 'LICENSE', 'NOTICE'}
expected |= {p.relative_to(ROOT).as_posix() for p in (ROOT / 'resources').rglob('*') if p.is_file()}
rows = {row['path']: row for row in report['files']}
assert set(rows) == expected
with zipfile.ZipFile(archive) as z:
    assert set(z.namelist()) == expected and len(z.infolist()) == len(expected)
    for info in z.infolist():
        name = info.filename
        data = z.read(info)
        mode = 0o755 if name == 'bin/json-resume-v2' else 0o644
        assert info.date_time == (2020, 1, 1, 0, 0, 0)
        assert stat.S_IFMT(info.external_attr >> 16) == stat.S_IFREG
        assert (info.external_attr >> 16) & 0o777 == mode
        assert int(rows[name]['mode'], 8) == mode and sha(data) == rows[name]['sha256']
        target = extracted / name
        assert not target.is_symlink() and target.read_bytes() == data
        assert target.stat().st_mode & 0o777 == mode
        if name not in ['bin/json-resume-v2', 'extension.json']:
            assert data == (ROOT / name).read_bytes()
manifest = json.loads((extracted / 'extension.json').read_text())
assert manifest['files'] == {name: row['sha256'] for name, row in rows.items() if name != 'extension.json'}
assert {k: v for k, v in manifest.items() if k != 'files'} == json.loads((ROOT / 'extension.json').read_text())
assert manifest['artifacts'] == {}
assert all(v['state'] == 'not-run' for v in manifest['verification'])
assert not any(part in {'qa', '.qa', 'node_modules', 'target', 'scripts'} for name in expected for part in pathlib.PurePosixPath(name).parts)
for needle in [str(ROOT.parent).encode(), b'node_modules', b'JSON_RESUME_TEST_', b'/same-url-aba', b'/aba-process-', b'/cancel-observe']:
    assert needle not in binary.read_bytes(), needle
info = json.loads(subprocess.check_output([str(binary), '--protocol-info'], cwd=extracted))
assert info == {'commands': ['open-resume', 'search-resumes'], 'id': 'com.tezbar.native.json-resume', 'protocol': 1, 'sdkVersion': '0.1.1', 'version': '0.1.0'}
architecture = subprocess.check_output(['file', str(binary)], text=True).strip()
assert architecture.endswith('Mach-O 64-bit executable arm64')
dylibs = [line.strip().split(' (')[0] for line in subprocess.check_output(['otool', '-L', str(binary)], text=True).splitlines()[1:]]
assert set(dylibs) == {'/System/Library/Frameworks/CoreFoundation.framework/Versions/A/CoreFoundation', '/usr/lib/libiconv.2.dylib', '/usr/lib/libSystem.B.dylib'}
home, target = map(pathlib.Path, (QA / 'fresh-build-paths.txt').read_text().splitlines())
assert home != target and home.is_relative_to(QA) and target.is_relative_to(QA)
assert binary.read_bytes() == (target / 'aarch64-apple-darwin/release/json-resume-v2').read_bytes()
checkouts = list((home / 'git/checkouts').glob('tezbar-extension-sdk-rust-*/e955911'))
assert len(checkouts) == 1
sdk = checkouts[0]
assert subprocess.check_output(['git', '-C', str(sdk), 'rev-parse', 'HEAD'], text=True).strip() == 'e955911f0bf5b558f9d3eb5183df0adc95c22af2'
for snapshot in (ROOT / 'qa/pinned').glob('sdk-*'):
    assert snapshot.read_bytes() == (sdk / snapshot.name[4:].replace('__', '/')).read_bytes()
locked = tomllib.loads((ROOT / 'Cargo.lock').read_text())
checksums = {p['name'] + '-' + p['version'] + '.crate': p['checksum'] for p in locked['package'] if p.get('source', '').startswith('registry+')}
fresh_archives = list((home / 'registry/cache').glob('*/*.crate'))
assert fresh_archives
for archive_file in fresh_archives:
    assert sha(archive_file.read_bytes()) == checksums[archive_file.name]
icon = extracted / 'resources/extension-icon.png'
assert icon.read_bytes() == (QA / 'upstream-icon-final.png').read_bytes()
assert sha(icon.read_bytes()) == '038c492799df020c53b8bd0ffb6f84e664e8caae1de571496ba5591087b71d45'
result = {'archiveSha256': report['sha256'], 'binarySha256': report['binarySha256'], 'members': len(expected), 'iconSha256': sha(icon.read_bytes()), 'protocolInfo': info, 'systemDylibs': dylibs, 'freshSDK': 'exact pinned checkout bytes', 'freshCargoArchivesVerified': len(fresh_archives), 'binaryFixtureAndCloneStrings': 'absent', 'deterministicClosedPackage': True}
(QA / 'round2-artifact-audit.json').write_text(json.dumps(result, indent=2) + '\n')
print(json.dumps(result))
