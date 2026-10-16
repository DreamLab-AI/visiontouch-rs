# Reference-call capture format

This specifies instrumentation, not the device protocol. Source P-0014: public
Linux ppdev ioctl definitions and dynamic-loader interposition interfaces.

Capture dynamically linked calls to `read`, `write`, and `ioctl` for one explicitly
selected descriptor target. Resolve descriptor paths through `/proc/self/fd` and
compare against the canonical target path. Forward the original call once,
preserve its result and errno, and never invent a successful hardware operation.
Use a small C/assembly ABI shim for the variadic ioctl entry point; on x86-64 it
forwards the third argument register unchanged, avoiding a variadic read when the
caller supplies no argument. Rust owns capture data and JSON serialization.
Support Linux x86-64 only initially.

Write JSON lines to separate files per session/process/thread. A session includes
the monotonic initialization timestamp to distinguish repeated PID namespaces.
Each event has schema version, session timestamp, PID, TID, sequence, monotonic start/end nanoseconds, descriptor, resolved
target, operation, requested count or ioctl request, result, errno on failure,
and before/after snapshots. Bytes are lowercase hex in memory order. Snapshot
metadata includes requested length, actual bytes copied, and whether it is complete.
Bound each snapshot to 65536 bytes and explicitly mark larger/inaccessible buffers
incomplete. Read memory via process_vm_readv on self so invalid caller pointers
are recorded as inaccessible rather than dereferenced by the recorder.

- write: snapshot the requested bytes before the call; result determines which
  prefix, if any, was transferred. A failed write is not a bus write.
- read: snapshot only the positive returned byte count after the call.
- ppdev ioctl: use the public generic ioctl direction and size encoding. Capture
  input before a write-direction request and output after a successful
  read-direction request. Unknown requests retain the raw request without
  claiming understood argument semantics. No-argument ppdev calls have no snapshot.
- Record attempted calls even when the underlying kernel rejects them.
- Include errno only for failures; successful calls may leave errno unchanged.

This is a function-call trace. LD_PRELOAD cannot see direct syscalls, inlined port
I/O, hidden/internal libc calls, statically linked programs, secure-execution
programs, or calls in a separate linker namespace. Interposition and logging alter
timing. Nested calls while recording (including signal reentrancy) are not recorded.
Only completed calls produce events; a crash, cancellation, or killed process can
leave an unrecorded in-flight call. Require a clean run and compare against the
syscall trace, including unfinished calls, before asserting completeness.
Cross-check with strace before asserting coverage; syscall counts, results, byte
counts and paths must agree. Do not replay concurrent calls as a total ordering:
start/end intervals can overlap. Logging failures invalidate a capture.
Initialization must complete before forking from a multithreaded process;
concurrent modification of application buffers is outside the recorder's control.

No peripheral register meaning or mode state is inferred by this layer. Decode
only successful operations later, retaining failed attempts and incomplete data
as explicit evidence. Sandbox/mock-file captures never prove unit connectivity.
