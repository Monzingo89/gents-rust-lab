# Website security and SOC 2 readiness register

**Status: readiness work in progress; no SOC 2 report or compliance certification.**
Last reviewed for source preparation: November 14, 2026.

SOC 2 is an examination by an independent licensed CPA of controls for a defined
system and period/scope. See the [AICPA SOC overview](https://www.aicpa-cima.com/resources/landing/system-and-organization-controls-soc-suite-of-services).
This register is an operator checklist and evidence map, not a CPA opinion or a
claim to satisfy every Trust Services Criterion. The 2026 proposed criteria are
not treated as an issued examination or proof of compliance.

## System scope and data

The hosted website provides public project information, a local-browser task-brief
builder, hardware/time/ownership scenarios and a support form. It hosts no model
inference, MCP execution, local task records, financial documents or patient data.
The repository contains source, synthetic fixtures and reviewed documentation;
operators supply models and private runtime storage separately.

Vercel serves the site and support endpoint. Resend accepts submitted support mail
for the fixed owner-controlled Gmail recipient. Website use also produces hosting
access data. The privacy page explains this boundary. Provider assurances must be
reviewed for their actual contracted services; they do not attest to this project.
The project maintainer is the responsible operator; contact details are published
in SECURITY.md and `.well-known/security.txt`.

| Control area | Implemented evidence | Outstanding operator evidence |
| --- | --- | --- |
| Source and change management | Public Git history; CI format/lint/tests; separate deploy artifact whitelist; exact source revision in build metadata | Review/branch-protection policy, emergency-change procedure and periodic access reviews |
| Secret handling | Ignored local env files; exact-value and generic secret scans; GitHub push protection; website-only Vercel stdin sync with Secret storage | Account MFA/least-privilege review, credential inventory, rotation/revocation exercise and access audit |
| Vulnerability management | Scheduled Rust/npm/Python-pin audits; reviewed GTK source backport and hash gate; security reporting contact | Triage ownership/SLA, complete production dependency inventory, assessment and closure evidence |
| Website access and abuse | HTTPS, security headers, origin/body/deadline checks, bounded support concurrency, signed challenges and replay controls | Provider edge quotas, sustained distributed-abuse testing and availability objectives |
| Privacy/confidentiality | Fixed support recipient, no form attachments, no task/model upload API, privacy disclosure, no support-body/raw-IP application logging | Lawful-basis/vendor review, mailbox retention/deletion records, data-subject request workflow and backup review |
| Incident response | Private GitHub vulnerability reporting, owner email, security.txt and source rollback capability | Approved response plan, escalation roster, exercised communications, incident register and recovery drill |
| Availability/recovery | Versioned source, isolated website package, staged deployment and byte/browser checks | Recovery objectives, monitoring/alerting ownership, tested restore/failover procedure and retained exercise records |
| Local agent installation | Owner/actor/tool/memory boundaries and recorded acceptance checks; local runtime excluded from hosted site | Per-installation model admission, host hardening, retention, regulated-data authorization and independent evaluation |

## Evidence handling and review

Keep private account reviews, incident records, access logs, vendor agreements and
credential inventories outside the public repository. Do not publish credentials,
real support messages, raw IPs, personal records or fabricated compliance evidence.
Record the control owner, review date, scope, result and next action for each real
review. Automated checks indicate only what was tested; failed and uncertain results
remain visible. Review advisory exceptions whenever dependencies or source change.

Before requesting an independent examination, the operator must define the service
scope, choose applicable criteria with the examiner, complete the organizational
controls above, retain operating evidence and remediate findings. No paid audit,
legal service or vendor upgrade has been purchased by this work. Independent
assurance cannot be completed through an unsupported website badge.

## Discovery claims

Search visibility is measured through real impressions/indexing/crawl data. SOC 2
readiness is not an asserted Google ranking factor. Follow [Google's search and AI
feature guidance](https://developers.google.com/search/docs/appearance/ai-features):
helpful, accurate, accessible content and technical crawl/index eligibility.
No guaranteed ranking, AI recommendation placement or invented score is promised.
