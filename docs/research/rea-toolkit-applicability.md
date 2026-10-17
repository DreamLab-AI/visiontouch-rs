# REA toolkit: applicability to the clean-room haptics project

*Capability survey only, 2026-10-17. Nothing here was run against a vendor artefact; none exists in this container (`vendor-original/` is ignored and absent). Sources: `~/workspace/rea` README, AGENTS.md, `docs/` (installation, cli, signal-recordings, process-capture, frida-instrumentation, native-investigation, ghidra-provider, mcp-contracts). Read together with `docs/clean-room/POLICY.md`, `DECOMPILATION-LOG.md`, `PROVENANCE.md` and `docs/legal/POSITION.md`. Not legal advice.*

## 1. What REA is, how it is invoked, how its evidence maps to ours

REA ("Reverse Engineer Anything", npm `rea-agents`, MIT, Node 22+) is a local-only toolkit that exposes one set of analysis workflows through two adapters:

- **CLI**: `rea <command> ... --json` (default output is TOON, so pass `--json` when saving). Examples: `rea decode-signal-recording`, `rea capture-process`, `rea analyze|function|decompile|xrefs|trace --provider ghidra`, `rea doctor --provider ghidra --json`, `rea providers`, `rea capabilities`.
- **MCP server**: the same contracts as tools (`decode_signal_recording`, `inspect_signal_decoders`, `open_binary`, `list_procedures`, `analyze_function`, `start_frida_session` and so on), registered into an agent by `npx rea-agents setup`, which shows its planned changes and needs approval.

Its evidence model: every result is an **Evidence** record carrying the input path and SHA-256, the exact selections and arguments, the resolved tool path, digest and version banner, bounded raw output, warnings, and explicit unknowns. AGENTS.md insists on distinguishing observations, derivations, inferences and unknowns. Snapshots (`--snapshot`) retain Ghidra results for later queries, with owner-only permissions.

Mapping onto `PROVENANCE.md` source classes:

| REA workflow family | Our class | Notes |
|---|---|---|
| `decode-signal-recording` on a logic-analyser capture | OBSERVE | The recording is ours; REA only decodes it. |
| `capture-process` (PTY run of a command, with filesystem snapshots) | BLACKBOX | Records terminal behaviour and exit of a program we are entitled to run. |
| Frida attach/spawn with a script | BLACKBOX only if the script touches nothing but the I/O boundary; otherwise treat as DECOMPILE-adjacent and log it first (see 2.3). | Injects code into a vendor process. |
| Ghidra / Hopper / IDA providers (`analyze`, `decompile`, `xrefs`, `trace`) | DECOMPILE | Always gated, section 3. |
| Firmware (Binwalk/Unblob), `inspect-*-binary`, string and section inspection of vendor files | DECOMPILE-adjacent | Reading vendor bytes is not observation. Static metadata about a vendor binary still counts as examining it; log it. |

REA's Evidence IDs are a good fit for the `P-NNNN` rows: cite the Evidence ID and input digest in the "Source detail" column, never the content of a decompiler result.

## 2. Observation-class capabilities (freely usable)

### 2.1 Recorded signal analysis via sigrok

`decode_signal_recording` wraps caller-supplied **sigrok-cli 0.7.2** with libsigrokdecode on Linux (needs util-linux `prlimit`; set `REA_SIGROK_COMMAND=/abs/path/sigrok-cli`). It decodes an existing file, never acquires samples or touches hardware, works on a private read-only copy (32 MiB cap, 30 s deadline, 1 GiB address-space limit), and returns annotation text over sample ranges.

```sh
export REA_SIGROK_COMMAND=/abs/path/to/sigrok-cli
rea inspect-signal-decoders --query parallel          # discover what is installed
rea decode-signal-recording captures/epp-boot.sr \
  --decoders parallel:clk=STROBE:d0=D0:...:d7=D7 \
  --input_format srzip --format json
```

Points that matter here:

- The stock sigrok decoder catalogue has a generic `parallel` decoder (clock plus data lines) but, to our knowledge, no EPP-aware decoder. Confirm with `inspect-signal-decoders` once sigrok is installed. Expect to use it for byte extraction and then do the EPP address-versus-data cycle classification (nAStrobe versus nDStrobe, nWrite, nWait handshakes) in our own Rust tooling.
- REA does not parse display text into protocol facts. Annotation lines are sample-ranged text, not timestamps. Our own parser should work from the raw `.sr`/`.vcd` file, with REA used as an independent cross-check.
- Raw captures stay out of git (`.gitignore` already excludes `/captures/**/*.sr|vcd|pcapng`). Commit only our derived tables and the P-row.
- Acquisition itself (sigrok-cli with a logic analyser on the DB25) is outside REA. Route 3 in `capture-strategy.md` already covers it.
- Not installed here: `sigrok-cli` is absent, so a capture session needs a host or container with it.

### 2.2 Runtime tracing and black-box recording

- **ppdev strace** (`capture-strategy.md` Route 1) needs no REA at all and remains the primary register-level record. REA adds little; keep using `strace -f -tt -T -e trace=openat,ioctl,read,write`.
- `rea capture-process scenario.json --json > capture.json` runs one command in a PTY and records terminal output, scheduled interactions, exit, and selected filesystem state; `compare` workflows diff two runs. It is explicitly **not a sandbox** (current user's permissions, inherited environment). Useful for repeatable black-box scenarios such as "run the vendor Test utility under Wine/VM with this input script, hash its output files". Linux/macOS only; `ps` from procps must be on `PATH`.
- Frida (`start_frida_session`, `load_frida_script`, `instrument-with-frida`) runs caller-authored JavaScript inside the target. Hooking only `ioctl`/`read`/`write` at the syscall boundary of a program we are entitled to run is arguably OBSERVE, but it is in-process instrumentation of vendor code, which POSITION.md does not explicitly bless. Recommendation: prefer strace and the logic analyser, and raise any Frida use with the owner first.

## 3. Decompilation-class capabilities (gated)

- **Providers**: Ghidra (Linux x64/arm64, macOS, experimental Windows P0, read-only, ephemeral database, no executable bytes modified), Hopper, IDA (read-only), plus GNU objdump and Rizin for offline inspection. Ghidra and IDA are bring-your-own: REA never installs them.
- **What must be installed**: an extracted Ghidra 12.1.x (`GHIDRA_INSTALL_DIR`; bridge verified with 12.1.4), a full 64-bit JDK 21+ matching the host (`JAVA_HOME`), the Ghidra native decompiler for the host platform, Node 22.19+/24.11+, and a procps-compatible `ps`. Check with `rea doctor --provider ghidra --json`. The earlier D-0001 and D-0002 runs used REA 6.5.0 with Ghidra 12.1.2.
- **Current container**: node is present; `ghidraRun`, `analyzeHeadless` and `sigrok-cli` are not. A native session would first need a deliberate install, which is itself a visible step and not to be done speculatively.
- **Useful REA behaviour for scope control**: exact selectors (`--procedure=`, `--address=`), bounded `trace`, snapshot files (keep under ignored `vendor-original/`), Windows P0 has no mutation authority, and results report coverage and limitations. Large pseudocode queries can exceed the MCP transport budget (this happened in D-0002), so prefer small function-scoped queries.

### Operator checklist before any native or firmware use

1. Name one interoperability question, in a sentence, that a byte-level fact answers.
2. Show why OBSERVE and BLACKBOX failed or are unavailable (what was tried, with P-row IDs).
3. Append a new `D-000N` entry **before** opening the tool: date, file names, SHA-256 of every admitted binary, the question, the why-observation-failed paragraph, the analyst (spec role, never implementation), and the planned scope limit.
4. Confirm the work is in the spec role and the implementation team will see only the own-words conclusion.
5. Check out-of-container safety: no target execution, no hardware powered by the analysis.
6. After the session, record the P-row (class DECOMPILE) and close native sessions.
7. Confirm nothing from the decompiler is staged (section 5).
8. Remember POSITION.md trigger: any decompilation prompts a re-read of the legal position, and findings are excluded from the paper (CDPA s50B(3)).

## 4. Open questions: observation or a logged D-0003?

| Question | Observation-answerable? | Recommendation |
|---|---|---|
| (a) Reset/home pose and joint-angle offsets the vendor assumes | **Yes, BLACKBOX.** Put the arm in the physical reset pose, query encoder positions through our own driver, and compare with the vendor Configuration/Test utility's displayed values. Repeat across several pose cycles to bound the offsets. | No decompilation. Native analysis is not justified; a code-derived constant would also be unusable in the paper. |
| (b) Meaning of the 0x17 setup byte 0x02 and control bytes 0xc0/0xce/0xca/0xc5 (partially observed) | **Mostly yes.** Vary one input at a time under the logic analyser and ppdev trace, change the vendor utility setting that should affect it, and observe device responses (status, current draw, behaviour) with the arm safe. Partial observation is a reason to design more experiments, not to decompile. | No D-0003 now. Revisit only if a designed matrix of experiments leaves a specific bit with no observable effect and the answer is needed for interoperability. If so, log it per bit, not per family. |
| (c) Force-to-DAC gain | **Already answered** by black-box queries. | Nothing to do. Record the P-row if not already present. |
| (d) Hardware watchdog / amp timeout when packets stop | **Yes, OBSERVE/BLACKBOX.** Stop sending packets with a force gauge or a compliant mass attached, at low force, and record time-to-release on the logic analyser. Needs the hardware-safety plan in `overload-and-calibration.md`. | No decompilation. D-0001 already found no safe kernel watchdog statically, which reinforces that only a physical test can answer this. Perform at low force only. |
| (e) Thermal / overload trip conditions | **Covered by D-0001**, which found command clipping, thermal-history checks and disable paths but no hardware threshold. Driving the unit to its limit is deliberately avoided. | No new entry. Do not widen D-0001. A physical threshold, if ever needed, is a hardware experiment with an explicit safety case, not a decompile. |

None of these meets the "observation cannot answer it" bar today.

## 5. How a future REA session would be run here

1. **Offline sandbox.** Run REA with no network, preferably in a throwaway container or VM without the haptics device attached. Native providers perform no target execution, which is the pattern D-0001 and D-0002 followed.
2. **Ignored output directories.** All inputs, snapshots, pseudocode, symbol maps and Evidence files live under `vendor-original/` (already in `.gitignore`, as are `/disasm/`, `*.dll`, `*.sys`, `*.exe`, `*.bin`). Capture files stay under `/captures/` with the existing ignore rules.
3. **Evidence IDs.** Record the REA version, provider version, input SHA-256 and the Evidence IDs in the private report under `vendor-original/`. In the repo, cite only the P-row and the D-000N entry.
4. **Commit nothing from the decompiler.** What may be committed: the D-000N log entry, the PROVENANCE row (class DECOMPILE), and a short own-words finding in `docs/spec/` after the spec-before-code rule. What may not: pseudocode, instruction listings, identifiers, strings, tables, constants copied as code, screenshots, or Evidence JSON bodies.
5. **Spec team only.** The analyst writes the own-words conclusion; the implementation team implements from the spec, not from the session.
6. **Paper exclusion.** Anything traced to a DECOMPILE row stays out of the paper. Prefer designing an observation experiment that rederives the fact so that it can be published.
7. **Close down.** End native sessions, delete the working snapshots if no longer needed, and `git status` to confirm that no ignored-class file leaked into staging.

## Summary recommendation

REA is useful now as an independent decoder and cross-check for logic-analyser recordings and for repeatable black-box process runs. Its native providers are available as a last resort but nothing in the current open-question list justifies a D-0003. Recommended next steps are observation experiments (a), (b) and (d), with (c) and (e) closed.
