"""Collect locally installed dependency notices for the packaged Windows build."""
from pathlib import Path
import json
import shutil
import subprocess

root = Path(__file__).resolve().parents[1]
out = root / 'assets' / 'licenses'
out.mkdir(parents=True, exist_ok=True)
metadata = json.loads(subprocess.check_output([
    'cargo', 'metadata', '--format-version', '1', '--locked',
    '--filter-platform', 'x86_64-pc-windows-msvc'
], cwd=root))
rows = ['# Third-party dependencies', '', 'Generated from Cargo.lock and package-lock.json.', '',
        '| Package | License declaration | Repository |', '|---|---|---|']
def collect(name, directory, license_label='', repository=''):
    rows.append(f'| {name} | {license_label or "See notices"} | {repository or ""} |')
    destination = out / name.replace('/', '_').replace('@', '')
    for file in directory.iterdir():
        if file.is_file() and file.name.upper().startswith(('LICENSE', 'LICENCE', 'COPYING', 'NOTICE', 'COPYRIGHT')):
            destination.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(file, destination / file.name)
for package in sorted(metadata['packages'], key=lambda p:(p['name'],p['version'])):
    if package['source']:
        collect(package['name']+'-'+package['version'],Path(package['manifest_path']).parent,
                package.get('license'),package.get('repository'))
lock = json.loads((root/'package-lock.json').read_text())
for path, info in lock['packages'].items():
    if path and (root/path).is_dir():
        collect(path.split('node_modules/')[-1]+'-'+info['version'],root/path,info.get('license'))
(out/'THIRD_PARTY.md').write_text('\n'.join(rows)+'\n',encoding='utf-8')
shutil.copyfile(root/'LICENSE',out/'PDF-Toolkit-LICENSE')
print(f'Collected notices for {len(rows)-5} packages in {out}')
