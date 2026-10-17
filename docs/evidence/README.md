# Evidence

Committed evidence is what a provenance row cites. This directory holds owner
measurements, photographs and captured sessions. It never holds vendor artefacts
(AGENTS.md rule 2; [clean-room policy](../clean-room/POLICY.md)).

## Naming

| Kind | Form |
|---|---|
| Session or capture directory | `<topic>-<yyyymmdd>/`, for example `input-only-sweep-20261016/` |
| Force verification session | `force-verification-<yyyymmdd>/`, a copy of one `captures/calibration/<utc>-<session_id>/` directory ([spec 14](../spec/14-force-calibration.md) section 9) |
| Single file | `<topic>-<yyyymmdd>.<ext>`, for example `owner-photos-20261016.zip` |

Dates are UTC, in the form of the session start. Every directory carries a `README.md`
stating who collected it, the question it answers, what it does not show, and the
SHA-256 of each large or lossless file.

## Promotion from `archive/`

`archive/` is the gitignored local holding area for raw captures and scratch material.
Material becomes evidence only by promotion:

1. Confirm the material answers a question a provenance row asks and contains no vendor
   artefact, serial number or personal data.
2. Copy (never move) it into `docs/evidence/<name>/`. Compress large CSV files
   losslessly; record the uncompressed digest.
3. Hash every promoted file with `sha256sum` and write the digests in the directory
   `README.md`. Do not edit a file after hashing; supersede it with a new directory.
4. Add the provenance row (class OWNER or OBSERVE) citing the path and digest.
5. Commit the directory and the row together.

## Hashing

SHA-256 only, lowercase hexadecimal, via `sha256sum`. Attempt files in a verification
session record the CSV digest in the JSON (`csv.sha256`); the directory `README.md`
records the digest of the JSON files and any archive.

## Referencing

Cite evidence from `PROVENANCE.md` with the repository-relative path and the digest, as
rows P-0043 and P-0044 do. Specifications cite the provenance row, not the file. A
delivered-force statement may cite only an OWNER or OBSERVE row that points at a
verification session here; a command value never counts (AGENTS.md rule 4).

## Current contents

| Path | Contents | Row |
|---|---|---|
| `input-only-sweep-20261016/` | Input-only workspace sweep, 31,271 samples, never armed | P-0044 |
| `liveness-cadence-20261017/` | Three attended liveness attempts that stopped on the speed interlock; the cadence correction | P-0065 |
| `owner-photos-20261016.zip` | Three owner photographs of the unit | P-0043 |

## Owner photographs

`owner-photos-20261016.zip` holds three photographs of the owner's unit taken on
2026-10-16 (P-0043). They show the base name plate and a motor
part-number label. Those are referential identification of lawfully owned
hardware, the same facts `docs/hardware/identification.md` records, and are not
vendor software artefacts under `docs/clean-room/POLICY.md` rule 5. Reviewed
2026-10-17; kept.
