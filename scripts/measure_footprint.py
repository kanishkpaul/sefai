"""Measure sefai's idle and model footprint and print one JSON record.

Usage:
  python3 scripts/measure_footprint.py idle BINARY
  python3 scripts/measure_footprint.py bench BINARY -- BENCH_ARGUMENTS...

`idle` starts `chat --plain` in an empty temporary workspace, waits for the ready line,
lets it settle, samples the process and quits; no model is loaded.
`bench` runs `BINARY bench BENCH_ARGUMENTS...` and reports its peak memory and benchmark lines.

Memory comes from the operating system, never from sefai itself:
macOS reports physical footprint (which includes Metal and MLX unified-memory allocations)
and resident size; Linux reports resident size; Windows reports working set and private bytes.
Only the standard library is used.
"""

import json
import os
import platform
import re
import subprocess
import sys
import tempfile
import time

SETTLE_SECONDS = 2.0
READY_TIMEOUT_SECONDS = 60.0


def host():
    record = {"system": platform.system(), "machine": platform.machine(), "release": platform.release(), "python": platform.python_version()}
    if record["system"] == "Darwin":
        record["chip"] = subprocess.run(["sysctl", "-n", "machdep.cpu.brand_string"], capture_output=True, text=True).stdout.strip()
        record["memory_bytes"] = int(subprocess.run(["sysctl", "-n", "hw.memsize"], capture_output=True, text=True).stdout)
    elif record["system"] == "Linux":
        with open("/proc/cpuinfo") as cpuinfo:
            names = [line.split(":", 1)[1].strip() for line in cpuinfo if line.startswith("model name")]
        record["chip"] = names[0] if names else platform.processor()
    else:
        record["chip"] = platform.processor()
    record["cpu_count"] = os.cpu_count()
    return record


def sample(pid):
    """Current memory of one live process, in bytes."""
    system = platform.system()
    if system == "Darwin":
        resident = int(subprocess.run(["ps", "-o", "rss=", "-p", str(pid)], capture_output=True, text=True).stdout) * 1024
        footprint = subprocess.run(["footprint", "-p", str(pid)], capture_output=True, text=True).stdout
        match = re.search(r"phys_footprint:\s*([\d.]+)\s*([KMG]B)", footprint)
        scale = {"KB": 1024, "MB": 1024**2, "GB": 1024**3}
        physical = round(float(match.group(1)) * scale[match.group(2)]) if match else None
        return {"resident_bytes": resident, "physical_footprint_bytes": physical}
    if system == "Linux":
        fields = {}
        with open(f"/proc/{pid}/status") as status:
            for line in status:
                name, _, value = line.partition(":")
                if name in ("VmRSS", "VmHWM", "RssAnon"):
                    fields[name] = int(value.split()[0]) * 1024
        return {"resident_bytes": fields["VmRSS"], "peak_resident_bytes": fields["VmHWM"], "anonymous_bytes": fields.get("RssAnon")}
    return windows_memory(pid)


def windows_memory(pid):
    import ctypes
    from ctypes import wintypes

    class Counters(ctypes.Structure):
        _fields_ = [
            ("cb", wintypes.DWORD), ("PageFaultCount", wintypes.DWORD),
            ("PeakWorkingSetSize", ctypes.c_size_t), ("WorkingSetSize", ctypes.c_size_t),
            ("QuotaPeakPagedPoolUsage", ctypes.c_size_t), ("QuotaPagedPoolUsage", ctypes.c_size_t),
            ("QuotaPeakNonPagedPoolUsage", ctypes.c_size_t), ("QuotaNonPagedPoolUsage", ctypes.c_size_t),
            ("PagefileUsage", ctypes.c_size_t), ("PeakPagefileUsage", ctypes.c_size_t),
            ("PrivateUsage", ctypes.c_size_t),
        ]

    kernel = ctypes.WinDLL("kernel32", use_last_error=True)
    psapi = ctypes.WinDLL("psapi", use_last_error=True)
    handle = kernel.OpenProcess(0x1000 | 0x0010, False, pid)
    if not handle:
        raise OSError(ctypes.get_last_error(), "OpenProcess failed")
    try:
        counters = Counters()
        counters.cb = ctypes.sizeof(Counters)
        if not psapi.GetProcessMemoryInfo(handle, ctypes.byref(counters), counters.cb):
            raise OSError(ctypes.get_last_error(), "GetProcessMemoryInfo failed")
        return {
            "working_set_bytes": counters.WorkingSetSize,
            "peak_working_set_bytes": counters.PeakWorkingSetSize,
            "private_bytes": counters.PrivateUsage,
        }
    finally:
        kernel.CloseHandle(handle)


def idle(binary):
    with tempfile.TemporaryDirectory() as workspace:
        process = subprocess.Popen(
            [binary, "--workspace", workspace, "chat", "--plain"],
            stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
            env={**os.environ, "NO_COLOR": "1"}, text=True,
        )
        deadline = time.monotonic() + READY_TIMEOUT_SECONDS
        while time.monotonic() < deadline:
            line = process.stdout.readline()
            if line.startswith("[ready]") or not line:
                break
        else:
            process.kill()
            raise SystemExit("chat did not become ready")
        time.sleep(SETTLE_SECONDS)
        memory = sample(process.pid)
        process.communicate("/quit\n", timeout=READY_TIMEOUT_SECONDS)
        if process.returncode != 0:
            raise SystemExit(f"chat exited with {process.returncode}")
    return {"kind": "idle", "memory": memory}


def bench(binary, arguments):
    command = [binary, "bench", *arguments]
    system = platform.system()
    if system == "Darwin":
        # time -l reports the peak physical footprint, which includes GPU unified memory.
        result = subprocess.run(["/usr/bin/time", "-l", *command], capture_output=True, text=True)
        peak = re.search(r"(\d+)\s+peak memory footprint", result.stderr)
        resident = re.search(r"(\d+)\s+maximum resident set size", result.stderr)
        memory = {"peak_physical_footprint_bytes": int(peak.group(1)) if peak else None,
                  "peak_resident_bytes": int(resident.group(1)) if resident else None}
    elif system == "Linux":
        result = subprocess.run(command, capture_output=True, text=True)
        import resource
        memory = {"peak_resident_bytes": resource.getrusage(resource.RUSAGE_CHILDREN).ru_maxrss * 1024}
    else:
        process = subprocess.Popen(command, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
        stdout, stderr = "", ""
        memory = {}
        while process.poll() is None:
            try:
                memory = windows_memory(process.pid)
            except OSError:
                pass
            time.sleep(0.05)
        stdout, stderr = process.communicate()
        result = subprocess.CompletedProcess(command, process.returncode, stdout, stderr)
        memory = {"peak_working_set_bytes": memory.get("peak_working_set_bytes"), "sampled_private_bytes": memory.get("private_bytes")}
    if result.returncode != 0:
        raise SystemExit(f"bench failed: {result.stderr.strip()[-2000:]}")
    samples = []
    for line in result.stdout.splitlines():
        if line.startswith("sample="):
            samples.append(dict(field.split("=", 1) for field in line.split()))
    engine = [line for line in result.stderr.splitlines() if "memory" in line and not line.strip()[0].isdigit()]
    return {"kind": "bench", "arguments": arguments, "memory": memory, "samples": samples, "engine_memory": engine}


def main():
    if len(sys.argv) < 3 or sys.argv[1] not in ("idle", "bench"):
        raise SystemExit(__doc__)
    binary = os.path.abspath(sys.argv[2])
    if sys.argv[1] == "idle":
        record = idle(binary)
    else:
        if len(sys.argv) < 4 or sys.argv[3] != "--":
            raise SystemExit(__doc__)
        record = bench(binary, sys.argv[4:])
    record["host"] = host()
    record["binary_bytes"] = os.path.getsize(binary)
    print(json.dumps(record, indent=2))


if __name__ == "__main__":
    main()
