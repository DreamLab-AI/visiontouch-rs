#!/usr/bin/env python3
"""Exercise the actual preload library against ordinary files, no port required."""
import json
import os
from pathlib import Path
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[3]
LIBRARY = ROOT / "target/debug/libport_capture.so"
subprocess.run([str(ROOT / "scripts/build-recorder")], check=True)

with tempfile.TemporaryDirectory(prefix="haptics-recorder-") as directory:
    work = Path(directory)
    binary = work / "exercise"
    subprocess.run(["cc", "-Wall", "-Wextra", "-Werror", "-pthread",
                    str(Path(__file__).with_name("exercise.c")), "-o", str(binary)], check=True)
    # Baseline verifies fixture assumptions independent of the recorder.
    baseline = work / "baseline"
    baseline.touch()
    subprocess.run([str(binary), str(baseline)], check=True)
    target = work / "target"
    target.touch()
    env = dict(os.environ, LD_PRELOAD=str(LIBRARY),
               PORT_CAPTURE_TARGET=str(target), PORT_CAPTURE_PREFIX=str(work / "trace"))
    completed = subprocess.run([str(binary), str(target)], env=env, check=True,
                               capture_output=True, text=True)
    assert completed.stderr == "", completed.stderr
    assert baseline.read_bytes() == target.read_bytes()
    logs = list(work.glob("trace.*.jsonl"))
    assert len(logs) == 3, logs
    events = []
    for log in logs:
        thread = [json.loads(line) for line in log.read_text().splitlines()]
        assert [e["sequence"] for e in thread] == list(range(1, len(thread) + 1))
        assert all(e["schema"] == 1 and e["start_ns"] <= e["end_ns"] for e in thread)
        assert all(e["target"] == str(target) for e in thread)
        events.extend(thread)
    assert len(events) == 13, len(events)
    writes = [e for e in events if e["operation"] == "write"]
    initial = next(e for e in writes if e["result"] == 5)
    assert initial["before"]["hex"] == "00ff225c0a"
    assert initial["errno"] is None
    returned = next(e for e in events if e["operation"] == "read" and e["result"] == 5)
    assert returned["count"] == 16 and returned["after"]["hex"] == "00ff225c0a"
    assert next(e for e in events if e["operation"] == "read" and e["result"] == 0)["after"] is None
    modes = [e for e in events if e["request_name"] == "PPSETMODE"]
    assert modes[0]["before"]["hex"] == "40200000"
    assert modes[0]["result"] == -1 and modes[0]["errno"] == 25
    assert modes[1]["before"]["complete"] is False
    assert modes[1]["before"]["memory_errno"] == 14
    assert next(e for e in events if e["request_name"] == "PPFCONTROL")["before"]["hex"] == "0f04"
    assert next(e for e in events if e["request_name"] == "PPCLAIM")["before"] is None
    unknown = next(e for e in events if e["operation"] == "ioctl" and e["request_name"] is None)
    assert unknown["result"] == 0 and unknown["before"] is None and unknown["after"] is None
    invalid = next(e for e in writes if e["errno"] == 14)
    assert invalid["before"]["complete"] is False and invalid["before"]["copied"] == 0
    large = next(e for e in writes if e["count"] == 65537)
    assert large["result"] == 65537 and large["before"]["copied"] == 65536
    assert large["before"]["complete"] is False
    assert next(e for e in writes if e["errno"] == 9)["before"]["hex"] == "00ff225c0a"
    assert len([e for e in writes if e["before"]["hex"] == "7879"]) == 2
    assert len({e["session_ns"] for e in events}) == 1
    # Reusing a prefix starts a distinct session instead of merging trace files.
    target.write_bytes(b"")
    subprocess.run([str(binary), str(target)], env=env, check=True)
    assert len(list(work.glob("trace.*.jsonl"))) == 6
    # A log output error must be visible, and must not change the target behavior.
    target.write_bytes(b"")
    env["PORT_CAPTURE_PREFIX"] = str(work / "missing/trace")
    failure = subprocess.run([str(binary), str(target)], env=env, check=True,
                             capture_output=True, text=True)
    assert "capture incomplete" in failure.stderr
    assert target.read_bytes() == baseline.read_bytes()
print("Recorder integration checks passed: forwarding, errno, buffers, invalid pointers, truncation, threads, log failures.")
