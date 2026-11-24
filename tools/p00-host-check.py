"""Read-only hardware inventory plus an isolated workspace file-permission probe."""
import ctypes
from ctypes import wintypes
from datetime import datetime, timezone
import json
import os
from pathlib import Path
import shutil
import uuid
import winreg

ROOT = Path(__file__).resolve().parents[1]


class MemoryStatus(ctypes.Structure):
    _fields_ = [('length', wintypes.DWORD), ('load', wintypes.DWORD)] + [
        (name, ctypes.c_ulonglong) for name in ('physical_total', 'physical_available',
        'page_total', 'page_available', 'virtual_total', 'virtual_available', 'extended_available')]


stamp = datetime.now(timezone.utc).isoformat()
inventory, errors = {'logical_processors': os.cpu_count()}, []
try:
    with winreg.OpenKey(winreg.HKEY_LOCAL_MACHINE, r'HARDWARE\DESCRIPTION\System\CentralProcessor\0') as key:
        inventory['cpu_model'] = winreg.QueryValueEx(key, 'ProcessorNameString')[0].strip()
except OSError as error:
    errors.append('CPU registry inventory unavailable: '+str(error.winerror))
memory = MemoryStatus()
memory.length = ctypes.sizeof(memory)
kernel = ctypes.WinDLL('kernel32', use_last_error=True)
kernel.GlobalMemoryStatusEx.argtypes = [ctypes.POINTER(MemoryStatus)]
kernel.GlobalMemoryStatusEx.restype = wintypes.BOOL
if kernel.GlobalMemoryStatusEx(ctypes.byref(memory)):
    inventory['physical_memory_bytes'] = memory.physical_total
else:
    errors.append('Physical memory inventory unavailable')
disk = shutil.disk_usage(ROOT)
inventory.update(workspace_free_bytes=disk.free, workspace_volume_total_bytes=disk.total)
probe = ROOT / '.local' / ('p00-host-'+uuid.uuid4().hex)
assert probe.resolve().is_relative_to(ROOT.resolve())
try:
    probe.mkdir()
    path = probe/'probe.bin'
    with path.open('xb') as stream:
        stream.write(b'P00 isolated permission probe\n')
        stream.flush()
        os.fsync(stream.fileno())
    assert path.read_bytes() == b'P00 isolated permission probe\n'
    moved = probe/'renamed.bin'
    path.rename(moved)
    moved.unlink()
    probe.rmdir()
    inventory['workspace_create_write_sync_read_rename_delete'] = 'PASS'
except (OSError, AssertionError):
    errors.append('Workspace file permission probe failed; inspect isolated p00-host directory')
record = dict(name='host-hardware-and-workspace', status='PASS' if not errors else 'BLOCKED',
              timestamp=stamp, command='python -X utf8 tools/p00-host-check.py',
              exit_code=0 if not errors else 2, artifact_path='reports/host-check.json',
              inventory=inventory, issues=errors,
              scope='Hardware and file permissions only; no benchmark, display setting or production data change')
(ROOT/'reports/host-check.json').write_text(json.dumps(record,indent=2)+'\n',encoding='utf-8')
print(json.dumps(record,indent=2))
raise SystemExit(record['exit_code'])
