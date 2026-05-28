#!/usr/bin/env bash
# Seed `autotag_description` for the top-3 root parents and the post-sweep
# top-20 children. Reads the markdown reference doc and applies the
# descriptions via direct SQL UPDATE so the data is correct even before the
# server has been rebuilt with the lifted parent_id IS NULL restriction.
#
# Idempotent — re-running is safe (UPDATE replaces by tag id).

set -euo pipefail

DB="${ATOMIC_DB:-/Users/brandonkiefer/Library/Application Support/com.atomic.app/databases/default.db}"

apply() {
  local id="$1"
  # Escape single quotes (SQL standard: '' inside a quoted literal).
  local desc=${2//\'/\'\'}
  sqlite3 "$DB" "UPDATE tags SET autotag_description = '$desc' WHERE id = '$id'"
  printf 'updated %s\n' "$id"
}

# Top-level parents
apply b16dfa35-ff1c-4c4e-b311-0422ad2b2b97 'ABOUT atoms documenting how work gets done over time: procedures, workflows, recurring activities, and the practices the org follows. NOT for atoms about a specific system, service, or piece of infrastructure that work happens to use.'
apply 93c37531-ec1c-44c1-a7f4-eabb5a1468d1 'ABOUT atoms describing named services, applications, products, or systems the organization owns or integrates with as discrete units. NOT for atoms about generic processes or organizational practices that apply across services.'
apply 42f61330-eb4d-499a-a16a-018f91bf8262 'ABOUT atoms that ARE reference material: documentation, conceptual explanations, instructional content, durable knowledge artifacts. NOT for active work artifacts — incidents, meeting transcripts, transactions, or in-flight project status.'

# Top-20 children
apply d041e205-4215-4d12-a177-1a4639042127 'ABOUT atoms describing engineering process conventions: code-review norms, branch strategy, CI/CD policies, release procedures, the way the engineering team works. NOT for atoms about specific tools (Jenkins, GitHub) used IN the workflow, NOT for migrations described in passing, and NOT for atoms where engineering happened to be the actor but the subject is something else.'
apply a6857d27-f43c-4a50-9298-65032b271b91 'ABOUT atoms specific to the Arkansas state-systems migration: source/destination systems, cutover events, data movement decisions, program-specific runbooks. NOT for general migration topics or for atoms that mention Arkansas as a customer in passing.'
apply 395c453a-45b1-4fc6-94a5-a41446c5d664 'ABOUT atoms describing how deployments happen: release cadence, deploy gates, rollback procedures, deploy-time runbooks. NOT for individual deploy events (those go on the relevant Service tag) and NOT for incidents that occurred during a deploy.'
apply a35551f0-124b-48aa-83f0-e4686e20ebb0 'ABOUT atoms that DEFINE rules, conventions, or contracts the organization adopts: coding standards, security baselines, naming conventions, API conventions. NOT for atoms that merely REFERENCE a standard while doing other work.'
apply 19b7d224-5cb8-4556-b214-f905b670aa53 'ABOUT atoms documenting network architecture, topology, VPC/subnet design, connectivity patterns, firewall rules. NOT for atoms about a specific outage in the network (use Production Incidents) and NOT for service-level docs that incidentally describe their network deps.'
apply e43dc3f8-8a52-4ced-a8cf-a76c672c3929 'ABOUT specific outage events with date, affected system, blast radius, and root cause. NOT for runbooks, NOT for postmortems referenced in passing, NOT for atoms that mention any past incident, and NOT for general "we had an outage once" anecdotes. The atom must be the incident report itself.'
apply 32f7ef45-8c2a-4cec-95e2-bbc8b372ef00 'ABOUT atoms that enumerate or describe the catalog of services owned by the org as a whole, or that maintain the catalog itself. NOT for atoms about an individual service IN the catalog — those go on the specific service tag.'
apply 5b717a96-71ce-4ca7-92c6-c77885b78ff9 'ABOUT atoms specific to GitHub-as-a-system: repos, GitHub Actions configs, GitHub-hosted infra, integration points with GitHub APIs. NOT for atoms that merely link to a GitHub URL or where GitHub is the incidental delivery channel for some other topic.'
apply 0fee02a6-afa1-472b-935e-c6c3b12ba6c4 'ABOUT atoms documenting specific Jenkins job configurations, pipeline definitions, or job-level behavior. NOT for atoms about Jenkins-the-server (use Jenkins Infrastructure if it exists) and NOT for CI/CD as a concept (use Engineering Workflow).'
apply d2e70c23-01f1-4c52-8432-c1dccb6aa427 'ABOUT atoms describing AR collection procedures, dunning workflows, and payment-recovery operations. NOT for atoms about a single AR transaction and NOT for general billing-policy discussion.'
apply 42d56dff-203a-4fc8-a43d-afe255674bf0 'ABOUT atoms that ARE templates — skeletons users copy: prompt templates, document templates, meeting-agenda templates, runbook stubs to be filled in. NOT for atoms that USE a template to do something else.'
apply 46b9cb92-6a5e-469e-94dc-0b18102e454c 'ABOUT atoms describing how teams work together: collaboration norms, conflict patterns, role definitions, RACIs, communication protocols. NOT for atoms about specific team members (use People) and NOT for individual meeting notes (use Meetings).'
apply efeddeea-88f9-44fb-b218-d5d0c0bc1f5b 'ABOUT atoms specific to the Central Database system: schema, queries, operational concerns, integration points. NOT for atoms about generic SQL, NOT for services that store data in CDB but are not ABOUT CDB, and NOT for incidents where CDB happened to be involved.'
apply 90a99679-03b1-4453-9cab-b59f283d9ebc 'ABOUT atoms that ARE meeting artifacts: agendas, minutes, transcripts, recordings, post-meeting summaries. NOT for atoms that DECIDED something in a meeting (use the relevant decision tag, e.g. ADRs) and NOT for ongoing process docs that mention meetings.'
apply e8889f41-adbf-483d-9b0c-3fac57390e79 'ABOUT atoms documenting a specific architectural decision in ADR format: context, decision, consequences. NOT for atoms that REFERENCE an ADR and NOT for general design discussion that has not been ratified.'
apply 0502fe65-faf8-4265-9e93-5b594c9dbe64 'ABOUT atoms describing improvement initiatives, retrospective findings, kaizen items, or OKRs aimed at process change. NOT for atoms that incidentally mention "we should improve X."'
apply 0f46fd83-34db-4abd-b91f-52c62bf8ca62 'ABOUT atoms describing project planning, status reporting, scheduling, milestones, RAID logs, project-management process artifacts. NOT for the deliverables OF a project — those get their own service/process tag.'
apply 170e08cb-e696-4a89-8668-a558521cb8ef 'ABOUT atoms describing audit procedures for financial transactions: trail design, reconciliation processes, audit-readiness checks. NOT for the transactions themselves and NOT for individual incident investigations.'
apply 8cfb973a-1375-4e7f-85b9-81f24eaed045 'ABOUT atoms specific to the TVR Billing system: invoicing flows, billing data model, integration with downstream systems. NOT for atoms about billing in general and NOT for atoms about non-TVR billing systems.'
apply ae60650a-2983-4854-b578-d895a98f8bb6 'ABOUT atoms specific to the CCP Payment Gateway: API integration, transaction flow, error handling, settlement. NOT for atoms about other payment systems and NOT for general payments topics.'

echo
echo 'verification:'
sqlite3 -header -column "$DB" "SELECT name, length(autotag_description) chars FROM tags WHERE id IN ('b16dfa35-ff1c-4c4e-b311-0422ad2b2b97','93c37531-ec1c-44c1-a7f4-eabb5a1468d1','42f61330-eb4d-499a-a16a-018f91bf8262','d041e205-4215-4d12-a177-1a4639042127','a6857d27-f43c-4a50-9298-65032b271b91','395c453a-45b1-4fc6-94a5-a41446c5d664','a35551f0-124b-48aa-83f0-e4686e20ebb0','19b7d224-5cb8-4556-b214-f905b670aa53','e43dc3f8-8a52-4ced-a8cf-a76c672c3929','32f7ef45-8c2a-4cec-95e2-bbc8b372ef00','5b717a96-71ce-4ca7-92c6-c77885b78ff9','0fee02a6-afa1-472b-935e-c6c3b12ba6c4','d2e70c23-01f1-4c52-8432-c1dccb6aa427','42d56dff-203a-4fc8-a43d-afe255674bf0','46b9cb92-6a5e-469e-94dc-0b18102e454c','efeddeea-88f9-44fb-b218-d5d0c0bc1f5b','90a99679-03b1-4453-9cab-b59f283d9ebc','e8889f41-adbf-483d-9b0c-3fac57390e79','0502fe65-faf8-4265-9e93-5b594c9dbe64','0f46fd83-34db-4abd-b91f-52c62bf8ca62','170e08cb-e696-4a89-8668-a558521cb8ef','8cfb973a-1375-4e7f-85b9-81f24eaed045','ae60650a-2983-4854-b578-d895a98f8bb6') ORDER BY chars DESC"
