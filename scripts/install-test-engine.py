#!/usr/bin/env python3
"""Install a checksum-pinned official 7-Zip into ignored CI storage."""
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tarfile
import urllib.request

root = Path(__file__).resolve().parents[1]
asset = json.loads((root / 'scripts/test-engines.lock.json').read_text())['assets'][sys.platform]
destination = root / '.ci' / 'engine'
destination.mkdir(parents=True, exist_ok=True)
archive = destination / asset['url'].rsplit('/', 1)[-1]
with urllib.request.urlopen(asset['url'], timeout=60) as response:
    payload = response.read()
if hashlib.sha256(payload).hexdigest() != asset['sha256']:
    raise SystemExit('Official test engine download checksum mismatch')
archive.write_bytes(payload)
if sys.platform == 'win32':
    extractor = shutil.which('7z') or r'C:\Program Files\7-Zip\7z.exe'
    subprocess.run([extractor, 'x', '-y', str(archive), f'-o{destination}'], check=True)
    binary = destination / 'x64' / '7za.exe'
else:
    with tarfile.open(archive) as bundle:
        bundle.extractall(destination, filter='data')
    binary = destination / '7zz'
    binary.chmod(0o755)
print(binary)
if os.environ.get('GITHUB_ENV'):
    with open(os.environ['GITHUB_ENV'], 'a', encoding='utf-8') as env:
        env.write(f'SMART_BACKUP_7ZZ={binary}\n')
