"""Check actual receiver bytes against the deterministic document fixture."""
import hashlib
import os
import json
from pathlib import Path
import runpy
import subprocess
import sys
adb = sys.argv[1] if len(sys.argv) > 1 else os.environ.get('ADB', 'adb')
serial = sys.argv[2] if len(sys.argv) > 2 else os.environ.get('ANDROID_SERIAL')
assert serial, 'Pass adb and device serial, or set ANDROID_SERIAL.'
raw = subprocess.check_output([adb, '-s', serial, 'shell', 'run-as', 'com.lingxia.test.receiver', 'cat', 'files/received.jsonl'], text=True)
record = json.loads(raw.strip().splitlines()[-1])
expected = runpy.run_path(str(Path(__file__).with_name('file-fixture.py')))['BODY']
assert record['action'] == 'android.intent.action.SEND'
assert record['mime'] == 'application/pdf', record
assert len(record['files']) == 1
received = record['files'][0]
assert received['size'] == len(expected), received
assert received['sha256'] == hashlib.sha256(expected).hexdigest(), received
assert received['uri'].startswith('content://com.lingxia.example.gpu.dev.lingxia.documents/')
Path('target/ui-share-file-payload.json').write_text(json.dumps(record, indent=2), encoding='utf-8')
print('PASS selected PDF bytes, MIME type, content URI grant and SHA-256 match fixture')
