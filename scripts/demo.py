#!/usr/bin/env python3
"""Exercise the actual CLI on disposable demo data, never personal files."""
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]
BINARY = ROOT / 'target' / 'release' / ('smart-backup.exe' if os.name == 'nt' else 'smart-backup')
if not BINARY.is_file():
    raise SystemExit('First run: cargo build --release')
ENGINE = os.environ.get('SMART_BACKUP_7ZZ') or shutil.which('7zz') or shutil.which('7z')
if not ENGINE:
    raise SystemExit('Install 7-Zip or set SMART_BACKUP_7ZZ to its executable path')
base = ROOT / '.demo'
base.mkdir(exist_ok=True)
base = Path(tempfile.mkdtemp(prefix='run-', dir=base))
source = base / 'source'
output = base / 'backups'
source.mkdir()
output.mkdir()
(source / 'hello.txt').write_text('Smart Backup: restored bytes match source.\n', encoding='utf-8')
(source / '项目说明.txt').write_text('中文文件名与内容验证 🐾\n', encoding='utf-8')
(source / 'desktop.ini').write_text('ignored system file', encoding='utf-8')
(source / 'empty').mkdir()

def run(*args, expected=0):
    proc = subprocess.run([str(BINARY), '--home', str(base / 'state'), '--json', *args], capture_output=True, text=True, encoding='utf-8')
    if proc.returncode != expected:
        raise RuntimeError(proc.stdout + proc.stderr)
    result = json.loads(proc.stdout)
    print(f"{' '.join(args[:2])}: {result['status']} / {result['reason_code']}")
    return result['data']

run('engine', 'configure', '--path', ENGINE)
run('job', 'create', 'Demo', '--source', f'Documents={source}', '--output', str(output))
preview = run('run', 'Demo', '--dry-run')
first = run('run', 'Demo')
run('verify', first['artifact'])
run('archive', 'list', first['artifact'])
run('run', 'Demo', expected=2)
run('restore', first['artifact'], '--output', str(base / 'restored'))
run('history', 'list')
assert (source / 'hello.txt').read_bytes() == (base / 'restored' / 'Documents' / 'hello.txt').read_bytes()
assert not (base / 'restored' / 'Documents' / 'desktop.ini').exists()
print(f'\nVerified demo: {base}')
print(f'Archive: {first["artifact"]}')
