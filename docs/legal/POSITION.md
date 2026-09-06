# Applied legal position for this project

*Not legal advice. This applies `landscape.md` to our specific facts. Re-check the cited sections on legislation.gov.uk before relying on them.*

## Facts

| Fact | Consequence |
|---|---|
| Device: SensAble PHANTOM, parallel/EPP generation (Desktop or Premium 1.0), lawfully owned | We are a "lawful user" of the hardware and of any driver copy we hold, which is the precondition for CDPA s50B/50BA and for the trade-secret "lawful acquisition" safe harbour. |
| Rights holder: 3D Systems Corporation (via Geomagic 2012, SensAble) | An active, well-resourced company. The IP is not orphaned. But the product and its Win98/NT driver are genuinely unsupported and unsold. |
| Developer is UK-based | UK CDPA 1988 ss50B, 50BA, 296A and s29 govern; *SAS v WPL* [2013] EWCA Civ 1482 is binding authority. |
| **Private project, no software release, academic paper as output** | Copyright infringement requires a restricted act. Private study on a lawfully owned copy is expressly permitted (s50BA), and there is no distribution. Exposure is confined to what is published in the paper. |
| No DRM / technological protection measure on an EPP link | DMCA 1201 and the UK equivalent (CDPA s296ZA) are not engaged. |

## Position by technique

1. **Observation of the wire and of the vendor software's I/O behaviour** (logic analyser, ppdev trace, VM I/O trace): permitted, s50BA, and not a copyright act at all as regards the wire. Publishable in the paper without restriction. *SAS* says the protocol itself is not protected expression.
2. **Black-box use of the vendor utilities** (Configuration, Test, PDD): permitted, s50BA; any licence term forbidding it is void, s296A.
3. **Decompilation of vendor binaries**: permitted under s50B only where observation could not yield the fact, only for the parts needed, and **information so obtained must not be communicated beyond what interoperability needs (s50B(3)(c))**. For a project whose output is a paper, that restriction is the one that bites. Policy: decompilation is a last resort, must be logged, and nothing traced to it goes in the paper.
4. **Copying code, tables, strings**: not permitted. Also the one thing WPL was actually held liable for.
5. **Distributing vendor binaries or firmware**: not permitted and not planned.

## Patents

US 5,587,937 (the foundational MIT patent) expired 2013. Later family members may be live, but UK Patents Act 1977 s60(5)(a) (private, non-commercial) and s60(5)(b) (experimental purposes relating to the invention) cover this project regardless. Only revisit if the posture changes to distribution or commercial use.

## Trademarks

"PHANTOM", "SensAble", "GHOST", "OpenHaptics" are used referentially in documentation only, with a non-affiliation disclaimer in the README. No crate, binary, or project name uses them.

## The paper

- Everything in the paper must trace via `docs/clean-room/PROVENANCE.md` to PUBLIC, OWNER, OBSERVE, or BLACKBOX rows.
- Do not quote vendor manuals or on-screen strings beyond short referential fragments.
- Describe the protocol in our own words, tables, and timing diagrams derived from our own captures.
- State the methodology (observation-first, clean-room lite) explicitly. It is part of the contribution.

## Optional: ask the rights holder

An email to 3D Systems asking whether they would release or license the legacy parallel-interface documentation for a non-commercial hardware-archaeology paper costs nothing. A "no" or silence does not change any of the above. A "yes" removes most remaining caution and would be worth a footnote of thanks.

## Trigger points to revisit this document

- Any intent to release the software, even as source.
- Any intent to sell services or hardware based on it.
- Any decompilation of a vendor binary.
- Any contact from the rights holder.
