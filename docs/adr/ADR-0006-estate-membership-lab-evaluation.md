---
id: ADR-0006
title: Hold this repository in the VisionFlow research estate as a lab evaluation with no economic use
date: 2026-10-17
decision_status: accepted
implementation_status: complete
activation_status: live
supersedes: []
superseded_by: []
verified_commit: 990beb0cf8acfb48013fe0f5c04e688f45248c9e
verified_paths: [AGENTS.md, README.md, docs/legal/POSITION.md]
owner: jjohare
review_trigger: any intent to publish the repository, distribute software, demonstrate outside the lab, or begin the VisionClaw integration
repo: visiontouch-rs
---

# ADR-0006: Hold this repository in the VisionFlow research estate as a lab evaluation with no economic use

## Context

The owner runs one of very few surviving units of this device and wants it usable
as an input for the lab's VisionFlow installation: a pose-and-button source for
selecting and dragging nodes in the VisionClaw graph. The legal position
(`docs/legal/POSITION.md`) rests on private study of lawfully owned hardware and
software, with an academic paper as the only output; every trigger for re-review
is a form of distribution or commercial use. The VisionFlow canon records
supporting repositories and grades them by evidence (visionflow:ADR-2006,
ADR-2017).

## Decision

The repository is formally part of the research estate and is recorded in the
VisionFlow canon (repository map, roster, website card, visionflow:ADR-2017) as a
private study and proof of principle, a lab evaluation with no
economic use. The repository stays private. The only planned estate integration is
a pose stream and stylus button entering VisionClaw through the same path as its
other input devices (web client controller producing node drag messages; XR client
selection resolver); it is not built here, and the force return path is out of
scope. No canon text, website text or README may describe the work as a product,
a service, a release or a driver for others' hardware. Product names appear only
referentially with the non-affiliation statement.

## Consequences

Estate documents can cite this repository, its diagrams live in the canon under
area `VT`, and the estate health roster lists it (as a private repository it reads
amber for CI visibility, which is accepted). Opening the repository, publishing
binaries, or demonstrating to third parties each re-open ADR-0001's legal review
before anything else happens. The VisionClaw integration, when started, is its own
ADR in both ledgers.

## Verification

`AGENTS.md` and `README.md` state the posture. The VisionFlow canon carries
ADR-2017, the roster row, the repository-map row, the website card and the
diagrams area `docs/diagrams/visiontouch-rs` (VT-01 to VT-08) at VisionFlow
commit 1e3bdd0 (re-stamped at fee91eb).
