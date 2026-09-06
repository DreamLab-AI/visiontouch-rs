# Legal landscape: clean-room reimplementation of an abandoned driver stack

*Research compiled 2026-09-06 by a web-research agent. Not legal advice. Statutory text and cases were checked against primary or authoritative secondary sources where possible; where a source could not be verified this is stated. Applied to our specific facts in `POSITION.md`.*

---

## 1. The e-spincorp.com article

[Is Reverse Engineering Legal? What You Need to Know](https://www.e-spincorp.com/is-reverse-engineering-legal/) (E-SPIN Group, Malaysian IT reseller; first published 2020-12-18, updated 2025-06-18; returns HTTP 403 to non-browser user agents).

**What it says:** reverse engineering is used for interoperability, security research, right-to-repair, competitive analysis; EULAs are binding and commonly forbid RE; "no universal law prohibiting reverse engineering"; DMCA prohibits circumventing DRM but has an interoperability exception; fair use has protected RE for research and compatibility but "does not overrule the terms of a EULA"; recommends the two-team clean-room approach; lists five ways RE becomes illegal (DRM circumvention, EULA breach, copyright/patent infringement including substantial similarity, trade secret misappropriation, damaging the market).

**Reliability:** a generalist marketing post, not a legal source.

1. US-centric and thin: no cases, no statute sections, zero UK/EU content.
2. Its claim that EULA anti-RE clauses always trump fair use is an oversimplification for the US and **flatly wrong for the UK/EU**: CDPA 1988 s296A and Software Directive Art 8 make such terms **void** for the acts protected by s50B/50BA and Arts 5(3)/6.
3. Conflates DMCA anti-circumvention with ordinary RE of a driver that has no protection measure at all.
4. No mention of the trade-secret "proper means" doctrine (DTSA; Trade Secrets Directive Art 3(1)(b)), the most protective doctrine for us.
5. No mention of patent term/expiry or firmware distribution.

Treat as a one-paragraph lay primer. Everything below supersedes it.

---

## 2. United States

### 2.1 *Sega Enterprises v. Accolade*, 977 F.2d 1510 (9th Cir. 1992)
Disassembly of Genesis code solely to extract the interface needed for compatibility was **fair use**: intermediate copying was "the only way to gain access to the ideas and functional elements" and Accolade had a legitimate reason. [Justia](https://law.justia.com/cases/federal/appellate-courts/F2/977/1510/305345/) · [Copyright Office summary](https://www.copyright.gov/fair-use/summaries/segaenters-accolade-9thcir1992.pdf)

### 2.2 *Sony v. Connectix*, 203 F.3d 596 (9th Cir. 2000)
Repeated copying of the PlayStation BIOS during disassembly to build an emulator was fair use; the product shipped **no Sony code** and required the user to supply their own BIOS. [Copyright Office summary](https://www.copyright.gov/fair-use/summaries/sony-connectix-9thcir2000.pdf) · [Justia](https://law.justia.com/cases/federal/appellate-courts/F3/203/596/474793/)

### 2.3 *Atari v. Nintendo*, 975 F.2d 832 (Fed. Cir. 1992)
RE "untainted by the purloined copy" is fair use, but Atari lost because it obtained Nintendo's source from the Copyright Office under false pretences. **How you obtained the thing you analyse matters.** [Justia](https://law.justia.com/cases/federal/appellate-courts/F2/975/832/163650/)

### 2.4 17 U.S.C. § 1201(f)
Permits circumventing an access control on a lawfully obtained program solely to identify elements necessary for interoperability of an independently created program, developing the means to do so, and sharing the information solely for interoperability. Only engaged if there is a technological protection measure at all; a bare EPP protocol almost certainly has none. [Cornell LII](https://www.law.cornell.edu/uscode/text/17/1201)

### 2.5 *Google v. Oracle*, 593 U.S. 1 (2021)
Copying Java API declaring code to build Android was fair use; reimplementing an interface so programmers can reuse their skills weighs toward fair use. Decided *assuming* copyrightability, so not a blanket "interfaces are free" holding. [Cornell LII](https://www.law.cornell.edu/supremecourt/text/18-956) · [CRS](https://www.congress.gov/crs-product/LSB10597)

### 2.6 DTSA, 18 U.S.C. § 1839(6)
"Improper means" expressly **excludes** "reverse engineering, independent derivation, or any other lawful means of acquisition." [Cornell LII](https://www.law.cornell.edu/uscode/text/18/1839)

### 2.7 *Bowers v. Baystate*, 320 F.3d 1317 (Fed. Cir. 2003)
A shrink-wrap anti-RE clause was enforceable and not preempted by fair use. Federal Circuit applying First Circuit law; not uniformly followed. Matters much less if you never accepted a EULA (boxed late-90s software; or pure wire observation never running the vendor software). [Justia](https://law.justia.com/cases/federal/appellate-courts/F3/320/1317/615628/)

---

## 3. European Union

### 3.1 Software Directive 2009/24/EC
[EUR-Lex](https://eur-lex.europa.eu/eli/dir/2009/24/oj/eng)
- **Art 5(3)**: a person entitled to use a copy may, without authorisation, "observe, study or test the functioning of the program in order to determine the ideas and principles which underlie any element" while loading, running, etc.
- **Art 6**: decompilation without authorisation where **indispensable** to obtain information necessary for interoperability of an independently created program, provided (a) done by a licensee/authorised person, (b) the information was not already readily available, (c) confined to the necessary parts. Art 6(2): information may not be used for other goals, given to others except as necessary, or used to develop a substantially similar program.
- **Art 8**: contractual provisions contrary to Art 6 or Art 5(2)–(3) are **null and void**.

### 3.2 *SAS Institute v World Programming*, CJEU C-406/10 (2 May 2012)
Grand Chamber: "Neither the functionality of a computer program nor the programming language and the format of data files used in a computer program … constitute a form of expression of that program … and, as such, are not protected by copyright." [EUR-Lex](https://eur-lex.europa.eu/legal-content/EN/TXT/?uri=celex:62010CJ0406) · [WIPO Lex](https://www.wipo.int/wipolex/en/judgments/details/1728)

**The single most important precedent for this project**: a wire protocol, command set and data format are not copyright subject matter; only the literal code is.

### 3.3 *Top System v Belgian State*, CJEU C-13/20 (6 Oct 2021)
A lawful acquirer may decompile to correct errors under the general Art 5(1) provision, not only under Art 6. Reinforces that the CJEU reads decompilation exceptions generously. [Lexology](https://www.lexology.com/library/detail.aspx?g=e8faecf8-961b-4d6a-af88-c4f6363122c5)

### 3.4 Trade Secrets Directive (EU) 2016/943, Art 3(1)(b)
Acquisition is **lawful** by "observation, study, disassembly or testing of a product or object that has been made available to the public or that is lawfully in the possession of the acquirer" free of any valid duty to limit acquisition. Recital 16 names reverse engineering as lawful "except when otherwise contractually agreed". [EUR-Lex](https://eur-lex.europa.eu/legal-content/EN/TXT/PDF/?uri=CELEX%3A32016L0943)

---

## 4. United Kingdom

### 4.1 CDPA 1988 ss 50B, 50BA, 296A
Inserted by the Copyright (Computer Programs) Regulations 1992 (SI 1992/3233); s50BA added by the 2014 Research, Education and Libraries Regulations.

- **[s50B Decompilation](https://www.legislation.gov.uk/ukpga/1988/48/section/50B)**: not infringement for a lawful user to convert low-level code to a higher-level form, provided (a) necessary to obtain information needed to create an independent program that can operate with it ("the permitted objective") and (b) the information is not used for any other purpose. s50B(3): conditions **not met** if (i) the information was readily available, (ii) decompilation went beyond what was necessary, (iii) the information was supplied to persons not needing it for the permitted objective, or (iv) used to create a substantially similar program.
- **[s50BA Observing, studying and testing](https://www.legislation.gov.uk/ukpga/1988/48/section/50BA)**: not infringement for a lawful user to observe, study or test the functioning of a program to determine underlying ideas and principles while loading, running, etc. **This is the section that authorises black-box behavioural testing of the vendor driver.**
- **[s296A](https://www.legislation.gov.uk/ukpga/1988/48/section/296A)**: any agreement term purporting to prohibit acts permitted by s50B or s50BA is **void**.

### 4.2 *SAS v WPL* in the UK courts
Court of Appeal, [2013] EWCA Civ 1482: WPL's clean-room replication of SAS's language and functionality did **not** infringe; WPL was liable only for **literal copying of parts of SAS's manuals**. Binding in England & Wales. [CaseMine](https://www.casemine.com/judgement/uk/5b46f2142c94e0775e7f1ff6) · [Linklaters](https://www.linklaters.com/en/insights/publications/intellectual-property-news/sas-v-world-programming--court-of-appeal-considers-copyright-in-software)

**Takeaway**: never quote the vendor's manuals, strings, or comments verbatim; that is exactly where WPL lost.

### 4.3 Trade Secrets (Enforcement, etc.) Regulations 2018, SI 2018/597
UK domestic implementation of the Directive; observation/study/disassembly/testing of a lawfully possessed product is lawful acquisition. [legislation.gov.uk](https://www.legislation.gov.uk/uksi/2018/597/contents/made)

### 4.4 Post-Brexit status
1. **Statutory text**: ss50B/50BA/296A are in the CDPA (a UK Act) and SI 2018/597 is UK secondary legislation. Neither was on the REUL Act 2023 Schedule 1 sunset list as far as could be found; legislation.gov.uk shows them as current. Re-check immediately before relying on them. [REUL Act 2023](https://www.legislation.gov.uk/ukpga/2023/28/enacted)
2. **CJEU case law**: the REUL Act s6 change to the departure test was **paused** by the 2024 Commencement No.2 Revocation Regulations, so the conservative test still applies and *SAS* remains effectively binding, in addition to the domestic Court of Appeal ruling. [Osborne Clarke](https://www.osborneclarke.com/insights/uk-government-pauses-introduction-courts-expanded-eu-case-law-departure-powers)

### 4.5 Additional UK provisions relevant to a private research project
- **CDPA s29** fair dealing for non-commercial research and private study.
- **Patents Act 1977 s60(5)(a),(b)**: acts done privately and for non-commercial purposes, and acts done for experimental purposes relating to the subject-matter of the invention, are not infringement.

---

## 5. Patents
20-year term from filing (Patents Act 1977 s25; EPC Art 63; 35 USC 154). A 1993–2003 device family's patents mostly expire 2013–2023, but continuations and later improvement patents can be live, and RE is **not** a defence to patent infringement. Check each patent's status individually (Google Patents, Espacenet, UK IPO). See `docs/research/device-phantom-parallel.md` §6 for the specific family.

## 6. Trademarks
Do not use the vendor's product name as the project, crate or binary name. Referential use ("works with X") is permitted (UK TMA 1994 s11(2); US nominative fair use); add a non-affiliation disclaimer. A mark can be cancelled for 5 years' non-use (TMA 1994 s46) but check the register rather than assume.

## 7. "Abandonware" is not a legal category
Copyright does not lapse on abandonment. Software is a literary work (CDPA s3(1)(b)); for a late-1990s corporate work the term runs for decades yet. **Orphan works** are a separate category requiring a diligent search and, in the UK, an IPO licensing scheme aimed at cultural institutions; not a practical route for us. On acquisition or insolvency the IP transfers to the successor. Find the current owner via Companies House / company registers and the trademark and patent registers, which show the owner of record. An unlocatable owner reduces practical enforcement risk but is not a defence.

## 8. Clean-room methodology
**Precedent**: Phoenix Technologies' IBM PC BIOS (1984): engineers who had read IBM's code wrote a functional spec; a second team who had never seen it implemented from the spec. IBM never sued. [History of Information](https://www.historyofinformation.com/detail.php?id=3391) · [Wikipedia](https://en.wikipedia.org/wiki/Clean-room_design)

**Method**: (1) two-team separation; (2) a spec that describes behaviour and interfaces in prose, tables and pseudocode, never verbatim code, names, comments, or strings; (3) provenance records on both sides; (4) no verbatim reuse of constants, tables, or strings.

**When it is overkill**: the two-team wall is a litigation defence. For a solo, private, non-commercial project on a 25-year-old device from a company that has passed through two acquisitions, it is more process than the risk warrants. A single developer captures most of the value with "clean-room lite": write the behavioural spec **before** the implementation, never paste decompiler output into the codebase, and keep dated commits showing observed → documented → implemented.

## 9. Risk gradient across techniques

| Technique | Legal basis / risk |
|---|---|
| **(a) Passive wire observation** with a logic analyser or port trace while the original software drives the device | **Very low.** No code is reproduced; arguably not a copyright act at all. s50BA / Art 5(3) if a program is involved; *SAS* says the protocol is not protected; TSD Art 3(1)(b) says observation of a lawfully possessed product is lawful acquisition. |
| **(b) Black-box testing** of the vendor driver | **Low.** Directly authorised by s50BA / Art 5(3); EULA restrictions void under s296A; *Sega*/*Connectix* in the US. |
| **(c) Decompilation** | **Permitted but conditional** under s50B / Art 6: necessary, not readily available elsewhere, confined to the necessary parts, results not shared beyond need, never used to produce a substantially similar program. Feed results into a written spec only. Lawful acquisition of the binary is a precondition (*Atari*). |
| **(d) Copying code, tables or constants verbatim** | **Not OK.** s50B(3)(iv); *SAS v WPL* (UK CA) on literal copying. Values dictated purely by hardware may be protected by merger, but document why and prefer independent re-derivation. |
| **(e) Redistributing vendor binaries or firmware** | **Not OK.** No interoperability exception covers redistributing the original work. |

## 10. Is a protocol copyrightable?
Generally no. US 17 USC 102(b) excludes procedures, processes, systems and methods of operation; *SAS* excludes functionality, language and data formats in the EU/UK. The **merger doctrine** protects reproduction of a byte value that is the only way to make the silicon do a thing. What *is* protected is the vendor's literal source, its expressive choices, and its documentation text. So: document the protocol as facts about behaviour, and implement from that document.

## 11. Firmware uploaded at start-up
If the device has no non-volatile firmware and the host driver uploads a blob at start, that blob is the vendor's copyrighted work and cannot be bundled. Options in descending cleanliness: (1) the user extracts the blob from their own media with a local tool and the new driver loads it at runtime (the *Connectix* / emulator-BIOS pattern); (2) reimplement the blob's *function* from the spec if it is simple; (3) never host or accept community dumps. Whether the parallel PHANTOM does this at all is an open question for the OBSERVE phase.

## 12. Provenance checklist
- Acquisition record: dated proof of lawful possession of hardware and software.
- Observation log: dated captures with exact hardware/software versions.
- Decompilation log (if any): binary, hash, tool, question, why not otherwise available.
- Specification document: own words, version-controlled, the sole input to implementation.
- No-copy attestation and spec-first commit ordering.
- Reviewer-separation note marking which files are spec work vs clean implementation.
- Trademark disclaimer.
- Firmware handling note.

---

## Sources
- E-SPIN article: https://www.e-spincorp.com/is-reverse-engineering-legal/
- *Sega v Accolade*: https://law.justia.com/cases/federal/appellate-courts/F2/977/1510/305345/
- *Sony v Connectix*: https://law.justia.com/cases/federal/appellate-courts/F3/203/596/474793/
- *Atari v Nintendo*: https://law.justia.com/cases/federal/appellate-courts/F2/975/832/163650/
- *Google v Oracle*: https://www.law.cornell.edu/supremecourt/text/18-956
- *Bowers v Baystate*: https://law.justia.com/cases/federal/appellate-courts/F3/320/1317/615628/
- 17 USC 1201: https://www.law.cornell.edu/uscode/text/17/1201
- 18 USC 1839: https://www.law.cornell.edu/uscode/text/18/1839
- Directive 2009/24/EC: https://eur-lex.europa.eu/eli/dir/2009/24/oj/eng
- *SAS v WPL* CJEU C-406/10: https://eur-lex.europa.eu/legal-content/EN/TXT/?uri=celex:62010CJ0406
- *SAS v WPL* [2013] EWCA Civ 1482: https://www.casemine.com/judgement/uk/5b46f2142c94e0775e7f1ff6
- *Top System* C-13/20: https://www.lexology.com/library/detail.aspx?g=e8faecf8-961b-4d6a-af88-c4f6363122c5
- Trade Secrets Directive 2016/943: https://eur-lex.europa.eu/legal-content/EN/TXT/PDF/?uri=CELEX%3A32016L0943
- CDPA s50B: https://www.legislation.gov.uk/ukpga/1988/48/section/50B
- CDPA s50BA: https://www.legislation.gov.uk/ukpga/1988/48/section/50BA
- CDPA s296A: https://www.legislation.gov.uk/ukpga/1988/48/section/296A
- SI 2018/597: https://www.legislation.gov.uk/uksi/2018/597/contents/made
- REUL Act 2023: https://www.legislation.gov.uk/ukpga/2023/28/enacted
- REUL s6 pause: https://www.osborneclarke.com/insights/uk-government-pauses-introduction-courts-expanded-eu-case-law-departure-powers
- Phoenix BIOS: https://www.historyofinformation.com/detail.php?id=3391
- EFF RE FAQ: https://www.eff.org/issues/coders/reverse-engineering-faq
- UK IPO orphan works: https://assets.publishing.service.gov.uk/media/5a7510d8e5274a3cb286961a/ipresearch-orphan-201307.pdf
