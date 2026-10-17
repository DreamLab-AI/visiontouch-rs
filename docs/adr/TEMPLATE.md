---
id: ADR-NNNN
title: Short imperative title
date: YYYY-MM-DD
decision_status: proposed        # proposed | accepted | rejected | superseded
implementation_status: none      # none | partial | complete
activation_status: inactive      # inactive | staged | live
supersedes: []                   # e.g. [ADR-0002]
superseded_by: []                # e.g. [ADR-0007]
verified_commit:                 # full 40-char git sha at which implementation_status was verified
verified_paths: []               # optional: repo-relative paths; any change after verified_commit fails the index gate
owner:                           # accountable person/handle
review_trigger:                  # the event/date that forces a re-review of this ADR
repo: visiontouch-rs               # visiontouch-rs
---

# ADR-NNNN: Short imperative title

## Context
<!-- Max 10 lines. The forces at play: the problem, constraints, prior state.
     State facts, not narrative. If it needs more than 10 lines, it is two ADRs. -->

## Decision
<!-- What we are doing, stated in the present tense as policy.
     Specific enough that a reader can tell whether a given change complies. -->

## Consequences
<!-- What becomes true, easier, harder, or forbidden as a result.
     Include costs and follow-on work, not just the upside. -->

## Verification
<!-- How implementation_status was established: the command, test, grep or
     artefact inspected, at verified_commit. Evidence, not intent. Physical
     claims about the device need an attended-session reference (P-NNNN). -->
