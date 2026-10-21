# Local interface control

P-0067. Studio exposes newline-delimited JSON over one Unix socket per process in
a user-owned private runtime directory. No TCP port is opened. Socket permissions
are 0600 and the directory is 0700. Existing sockets are never overwritten.

A connection carries one bounded request and one response. Parsing, socket I/O
and client timeouts run outside the UI and servo threads. The UI applies a bounded
number of queued requests per frame. Requests expire before application if the UI
has not processed them within one second. The response distinguishes rejection
from dispatch to the worker: dispatch does not claim that registration, capture,
connection or persistence succeeded. Read status to observe the worker result.

Supported operations: status; Stop; connect/disconnect; switch guide/bench;
advance the guide only through its existing checks; save/replace a reference;
capture a return, ruler mark or effect centre; arm the input-only gravity check;
submit ruler, observation and measurement entries; coverage/export; start a
motors-off backdrive record; prepare existing held motor/effect tests. Preparing
requires idle output and valid existing prerequisites. It never enables output.

There is no hold command, raw demand, limit override, verdict setter, thermal
Start or thermal Confirm operation. A prepared powered test needs a fresh local
Space/button hold, and all existing worker and SDK interlocks remain active.
View changes and Stop cancel pending activation. Mutations are allowlisted and
unknown fields are rejected. Replacing a reference requires an explicit replace
flag. Evidence and liveness scoring use existing worker commands unchanged.

Status includes mode, PID, guide progress, connection, registration, counters,
model position/velocity, output state, pending/running test, last result, limits,
check results, worker status, evidence directory and snapshot age. Same-user
local clients have the same trust level as the local UI; this is not a boundary
against arbitrary code already running as the owner. Nothing starts hardware in
unit tests or headless smoke tests.
