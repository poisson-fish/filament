# Filament Legal Exposure Analysis (U.S.-Focused, 2026-02-27)

This document is a practical legal-risk analysis for operating Filament, especially with optional E2EE.

This is not legal advice. Use this as engineering/compliance planning input and review with qualified counsel.

## Scope and Assumptions
- Jurisdiction focus: United States federal baseline.
- Filament model:
  - self-hosted communication service
  - user-generated messages/files
  - optional end-to-end encryption (E2EE)
  - LiveKit-based realtime media
- Legal risk varies by deployment, operator location, and user location.

## Executive Summary
- E2EE can reduce operator access to content, but does not eliminate legal exposure.
- Exposure shifts from content-review capability to policy/process execution risk.
- Core U.S. federal risk areas:
  - CSAM reporting and preservation obligations
  - criminal law carve-outs from Section 230
  - copyright takedown/safe-harbor process requirements
  - disclosure/preservation handling under stored-communications law

## Core Exposure Areas
## 1) CSAM and Child-Exploitation Reporting
Relevant baseline:
- `18 U.S.C. § 2258A` imposes a duty to report certain apparent violations after obtaining actual knowledge.
- Penalties for knowing/willful failure to report were increased in 2024 amendments.
- The statute also states it does not require general monitoring.

What this means for Filament operators:
- If the service obtains actual knowledge, reporting workflow must exist and be executed quickly.
- E2EE does not remove duty to report known material.
- Incident evidence retention and chain-of-custody controls are operationally critical.

## 2) Section 230 Boundaries
Relevant baseline:
- `47 U.S.C. § 230(c)` provides broad protections for third-party content.
- `47 U.S.C. § 230(e)` expressly does not impair:
  - federal criminal law
  - intellectual property law
  - sex-trafficking related claims/prosecutions carved out by statute

What this means for Filament operators:
- Section 230 is helpful but not a blanket shield.
- You still need robust criminal-response and abuse-escalation processes.

## 3) Copyright (DMCA Safe Harbor)
Relevant baseline:
- `17 U.S.C. § 512` safe harbor is conditional, not automatic.
- Conditions include:
  - designated agent registration and publication
  - repeat-infringer policy
  - notice-and-takedown and counter-notice process execution

What this means for Filament operators:
- If hosting user files/messages, missing process controls can forfeit safe-harbor protections.

## 4) Disclosure and Preservation (Stored Communications)
Relevant baseline:
- `18 U.S.C. § 2702` limits voluntary disclosure with specific exceptions (including NCMEC-related and emergency pathways).
- `18 U.S.C. § 2703` defines compelled-disclosure pathways.
- `18 U.S.C. § 2703(f)` requires preservation on government request.

What this means for Filament operators:
- Disclosure decisions must follow documented legal-process handling.
- Preservation requests need a repeatable runbook and audit trail.

## How E2EE Changes Exposure
With E2EE enabled for a context (DM/group/guild message/call):
- Operator generally cannot access plaintext content.
- Operator still sees metadata (accounts, timing, sizes, routing/context identifiers).
- Moderation/search based on server-side content inspection is reduced or unavailable.

Legal/compliance implications:
- You still need:
  - abuse reporting pathways
  - emergency escalation pathways
  - account and metadata-based enforcement controls
- Lawful process responses may be limited to metadata/ciphertext when keys are client-held.
- Product disclosures should clearly state what is and is not readable by operators.

## Risk Matrix (High-Level)
1. CSAM known-content report failure
- Likelihood: medium
- Impact: high
- Primary controls:
  - documented CyberTipline process
  - on-call escalation
  - evidence preservation workflow

2. Improper legal-process disclosure
- Likelihood: medium
- Impact: high
- Primary controls:
  - request validation SOP
  - role-limited disclosure approvals
  - immutable audit log for disclosures

3. DMCA safe-harbor process gaps
- Likelihood: medium
- Impact: medium/high
- Primary controls:
  - designated agent registration
  - published policy
  - ticketed SLA for notices/counter-notices

4. E2EE misuse for abuse coordination
- Likelihood: medium
- Impact: high
- Primary controls:
  - account controls and friendship/message-request friction
  - metadata/rate abuse detection
  - user-reporting with optional plaintext reporter disclosure

## Minimum Compliance Controls for Filament Operators
## Policy and Terms
- Publish Terms of Service and Acceptable Use Policy with explicit prohibited-content clauses.
- Publish privacy/disclosure statement explaining metadata retention and E2EE limits.
- Publish law-enforcement request policy.

## Reporting and Enforcement
- Provide in-product abuse reporting for message/file/call contexts.
- Provide high-priority reporting path for child safety emergencies.
- Define internal severity levels and response SLAs.
- Keep enforcement actions auditable (who acted, when, why).

## Legal Operations
- Maintain contact method for legal process.
- Implement request triage:
  - subpoena vs court order vs warrant vs emergency disclosure
- Implement preservation request handling and retention timers.
- Maintain counsel-reviewed templates for response/rejection/escalation.

## Copyright Operations
- Register and maintain DMCA designated agent.
- Implement takedown and counter-notice workflows.
- Implement repeat-infringer policy with documented enforcement.

## Security and Evidence Handling
- Retain security/event logs with access controls and integrity guarantees.
- Separate operations access from moderation/legal access.
- Avoid storing sensitive secrets in logs/telemetry.

## Filament-Specific Engineering Requirements
These should be treated as product requirements for hosted deployments:
- `AUP/ToS` acceptance capture per account.
- Structured audit log categories:
  - `abuse.report`
  - `legal.request.received`
  - `legal.request.response`
  - `preservation.request`
  - `enforcement.action`
- Configurable retention policies for:
  - audit logs
  - legal hold/preservation snapshots
  - abuse-report artifacts
- E2EE policy flags in server config and UI disclosures.
- Clear UI indicators for encrypted contexts and reporting limits.

## Operational Runbook (Recommended)
1. Intake
- Receive abuse/legal report and classify severity.
2. Preserve
- Apply immediate preservation hold for relevant metadata/logs/artifacts.
3. Validate
- Validate legal basis (if government request) and scope.
4. Decide
- Approve, partially comply, or reject based on legal basis + data availability.
5. Execute
- Produce data according to least-privilege and chain-of-custody controls.
6. Record
- Write immutable audit entries and evidence manifest.
7. Review
- Post-incident review for policy/control gaps.

## Open Legal Questions to Resolve with Counsel
- Which jurisdictions/operators are in scope for launch and how conflicts of law are handled.
- Required retention periods by category (operational, legal hold, abuse report artifacts).
- Emergency disclosure standard and internal approval chain.
- Content/reporting obligations for encrypted contexts across target jurisdictions.
- Terms/privacy language for optional E2EE in mixed plaintext/encrypted environments.

## Source References (Primary / Official)
- U.S. Code (House): `47 U.S.C. § 230`
  - https://uscode.house.gov/view.xhtml?req=granuleid:USC-prelim-title47-section230&num=0&edition=prelim
- U.S. Code (House): `18 U.S.C. § 2258A`
  - https://uscode.house.gov/view.xhtml?req=granuleid:USC-prelim-title18-section2258A&num=0&edition=prelim
- U.S. Code (House): `17 U.S.C. § 512`
  - https://uscode.house.gov/view.xhtml?req=granuleid:USC-prelim-title17-section512&num=0&edition=prelim
- U.S. Code (House): `18 U.S.C. § 2702`
  - https://uscode.house.gov/view.xhtml?req=granuleid:USC-prelim-title18-section2702&num=0&edition=prelim
- U.S. Code (House): `18 U.S.C. § 2703`
  - https://uscode.house.gov/view.xhtml?req=granuleid:USC-prelim-title18-section2703&num=0&edition=prelim
- NCMEC CyberTipline:
  - https://www.ncmec.org/gethelpnow/cybertipline
- U.S. Copyright Office `§512` resources:
  - https://www.copyright.gov/512/
