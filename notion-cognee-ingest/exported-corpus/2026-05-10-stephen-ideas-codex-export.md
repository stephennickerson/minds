# Stephen's Ideas Captured by Agents - Notion Corpus Export

Exported for Cognee ingestion on 2026-05-10 from the Notion root page:
https://www.notion.so/3594b2507c2080e69d3fc5ca1d8264c1

This document preserves the reachable page tree, standing context, linked decision pages, database schemas, and database rows returned by the Notion connector.

## Root Page

Title: Stephen's Ideas Captured by Agents

Purpose: This is the single knowledge tree for Stephen Nickerson's digital clone. Every conversation, client, system, prompt, decision, framework, and skill lives here. The intended end state is to export this page and its children as a single corpus, convert it to a knowledge graph, and use it to train an AI system that thinks, decides, and operates the way Stephen does.

The tree has two kinds of children: standing context and collections.

Standing context:
- Background & Identity: who Stephen is, where he came from, what he has built, and the core identity of building recursively self-improving systems now applied to generating capital.
- Architecture Principles: design principles governing every system Stephen builds.
- Clients & Engagements: client relationships, current and past.
- Top of Mind: current priorities, active threads, and near-term targets.

Collections:
- Projects: active and recent builds, systems, products, and deliverables.
- Frameworks & Protocols: reusable thinking shapes, protocols, patterns, methodologies, and principles.
- Skills Library: SKILL.md catalog, capability primitives agents load on demand.
- Decisions & Analysis: resolved decision trees, capital grill reports, opportunity memos, and strategic evaluations.

Routing rules:
- A new build, product, or deliverable goes in Projects.
- A reusable pattern, protocol, or methodology goes in Frameworks & Protocols.
- A new SKILL.md or capability primitive goes in Skills Library.
- A resolved decision, grill report, or strategic memo goes under Decisions & Analysis.
- A change to identity/background updates Background & Identity.
- A change to design principles updates Architecture Principles.
- A new or updated client relationship updates Clients & Engagements.
- A shift in current priorities updates Top of Mind.
- Anything that does not fit requires asking before creating a new top-level category.

Operating constraints:
- Helene / FFC and Mandie / E4B are different people.
- No Docker. Claude Code agent fleet runs natively on Linux with JS plugins.
- Mike is one JS plugin, not a separate daemon.
- Avoid absolute statements.
- Thinking buddies, not coding buddies. Architecture and mental models first; code only when explicitly asked.
- Capital generation lens: How does this generate capital today? Monetize what already exists before building what is next.
- Single tree, no structure links outside the tree.

## Background & Identity

Long-term context: the 25+ year arc that produced the current work.

Stephen is the solo founder of RadicalSimplicity.AI. He lives in Costa Rica with partner Mandie. He has an early-morning mountain hike and meditation practice. He no longer trades crypto.

Core entrepreneurial identity: building recursively self-improving systems, now deliberately directed at capital generation.

Career arc:
- 25+ years as a systems architect and organizational fixer across Financial Services, Oil & Gas, Insurance, Automotive, and Technology.
- Clients included Yale, Toyota, Johnson & Johnson.
- Roles spanning developer, CTO, CEO, CMO, and founder.
- Previously ran RapidMind Solutions Inc.
- Experienced a significant business failure that seeded the current work, including the Justice Swarm concept, a legal accountability platform.

Costa Rica period: roughly 7 years in intentional simplification before deliberately redirecting toward capital generation.

Past work that still echoes:
- Trading systems: PVSRA, ribbon strategies, multi-agent trading analysis.
- CrypTones: pen name for trading psychology content.
- Co-authored book projects with Mandie.
- Generator Suite pattern: coined about 7 years ago, now manifesting as the agent fleet.
- Persona Wizard methodology: 5 masters to distilled traits to named persona, origin of FRANK.

Tooling reality:
- Claude Max 20x plan.
- Free models through AntiGravity or other routes for most execution work.
- Claude Code on Linux is the Claude environment, migrated from OpenClaw fork after the April 4, 2026 policy change.
- AntiGravity for AI workflow orchestration, chaining personas via YAML context files and outputting to GitHub.
- No ZeroClaw for Claude. ZeroClaw is GPT-only OAuth.
- Codex via codex-mcp-server was briefly explored during a usage cooldown while continuing to sell Anthropic-based services.

Operating rules:
- Avoid absolute statements. Use qualifiers such as "much less than," "unclear how much," or "rarely."
- When in doubt, invite the conversation instead of declaring a verdict.
- Thinking buddies, not coding buddies. Default to architecture, shape, and mental models. Code only when explicitly asked.

## Architecture Principles

Database-first, fully normalized:
No JSON or Markdown as storage. Cloudflare D1/SQLite is the source of truth. Workers validate intake against dynamic schemas. The Director of Coherent Infrastructure role enforces this.

Cloudflare as nervous system:
D1, KV, R2, Workers. The substrate everything runs on.

Closed-System Principle / True Zero Error Handling:
In a system fully controlled end-to-end, defensive code is dead weight. The test is whether both ends are owned. If yes, validation is dead weight inside the boundary. Validation lives only at the boundary where untrusted input enters.

Problem-Earned Code:
Stricter than YAGNI. Do not write a line until reality has handed the problem to this codebase, with a real use case. No solutions without problems. No abstraction without lived demand. No pattern because tutorials use it.

Solve Once, Configure Forever:
When code is written, solve the real problem as a configurable instance of a general pattern so future variations are configuration changes, not new code. This is the third path between speculative abstraction and pure YAGNI. DRY as bug prevention by structure.

Narrowest Scope:
Place configuration at the narrowest scope where it is actually used. Method-level for single-method constants, class-level for shared constants, file-level for component-shared constants, global only for truly app-wide constants. Zero magic numbers in business logic.

Pipeline Architecture:
Clear, sequential, modular pipelines. Linear data flow. Easy to debug. Compose complex flows from simple building blocks.

Composition Over Inheritance:
Clear boundaries. Simple interfaces. Compose behavior; do not inherit it.

Locality of Behavior:
Code that changes together lives together. Cleverness via indirection costs more than it saves.

Dual-Purpose Functions:
Configurability means functions absorb variation through parameters, not copy-paste. Readability means 4-10 line functions encapsulate library interactions behind intention-revealing names. A reusable function with hardcoded variations is repeated code in a wrapper. A configurable function absorbs the variation itself.

Naming as Storytelling:
ALL_CAPS for reusable constants at broader scope. normalCasing for method-level variables describing contextual meaning. _prefixedFunctions for private/internal. Direct literals are acceptable in self-describing functions such as setLogLevel("debug") when the function name clarifies the parameter purpose.

No Docker, No Containers:
Claude Code agent fleet runs natively on Linux with JS add-ons/plugins. Architecture discussions should not reference Docker, containers, images, or containerization.

Agent Restart, Never Resume:
Context-limit restart loop at 180k tokens. Stop hook to handover document to orchestrator restart. Agents never resume. They restart clean.

Model-Agnostic Dispatcher:
Switching models is a single database row change. All Claude Code CLI flags are stored in DB tables. A watchtower agent can optimize them dynamically.

Observed attachment references on this page:
- CODING_STANDARDS.md
- SKILL.md
The connector exposed these as file references. The canonical local copies were added to the same Cognee dataset as durable memory and saved under `exported-corpus/notion-attachments/`.

## Fleet Router - LiteLLM Gateway

Fleet Router is the central LLM gateway for RadicalSimplicity.AI agent infrastructure. It is an OpenAI-compatible drop-in replacement routing through 6 pooled Gemini accounts.

Connection:
- URL: https://fleet.radicalsimplicity.ai/v1/chat/completions
- Protocol: OpenAI-compatible
- Auth: Bearer token via SWARM_FLEET_KEY
- Default model: gemini-3.0-flash
- Server-side concurrency: 6

Available models:
- gemini-3.0-flash: swarm daemon default.
- gemini-3.1-pro-preview: higher capability.
- gemini-3-flash: alternate flash.

Verified Cognee configuration:

LLM:
```text
LLM_PROVIDER=custom
LLM_MODEL=openai/gemini-3-flash
LLM_ENDPOINT=https://fleet.radicalsimplicity.ai/v1
LLM_API_KEY=$SWARM_FLEET_KEY
LLM_ARGS={"extra_body":{"stream":false}}
```

Embeddings:
```text
EMBEDDING_PROVIDER=fastembed
EMBEDDING_MODEL=BAAI/bge-small-en-v1.5
```

Storage:
```text
VECTOR_DB=lancedb
GRAPH_DB=ladybug
```

Key gotcha: LLM_ARGS with stream false is required so Cognee/LiteLLM can parse the Fleet Router response correctly.

Cognee API is expected at http://127.0.0.1:8000 and health returns ready, healthy, 1.0.9-local.

## Clients & Engagements

RadicalSimplicity.AI:
Stephen is the solo founder. It is an AI consulting and agent systems business operating at stephennickerson.com.

Active offerings:
- Agents for Everybody: recorded extraction interview to transcript to self-configuring Claude agent. Two-stage discovery/engagement fee structure.
- Three-Tier Services: Agent / Colony / Swarm mapped to the Wound Hierarchy.

Positioning: anti-consultant. Capability transfer, not dependency creation.

Helene Theriault / Function First Coaching:
- BScOT, MAdEd, MCC.
- Founder of Function First Coaching and Dive Into a Coach Approach (DICA).
- Built: FFC static site migration, Loom video processing pipeline, FFC agent fleet (Louis, Alignify, Wizard, Harmony, Mike), and 84-agent self-assembling org with Brene Brown as founder persona.
- Brand assets: DIVE IN Goals, Coaching Spectrum Framework, Coach Approach in Healthcare, AOTA CEU compliance, Bloom's Taxonomy learning objectives.

Mandie / Essentials 4 Balance:
- Wellness business and separate from FFC.
- In progress: E4B checkout architecture. DIY Stripe Elements + Apple Pay on Firebase to webhook to Podia. Rejected SamCart and Stan.

Joseph and Paul Fudge are no longer involved. RadicalSimplicity.AI is solo.

## Top of Mind

Admin Scale Graph Inspector:
Graph-based, card-based infinite-drilldown UI for inspecting organizations as recursive special team structures. Grounded in the Admin Scale's 10 dimensions: Goals, Purposes, Policy, Plans, Programs, Projects, Orders, Ideal Scenes, Statistics, VFPs. Framed as a real-time organizational coherence auditor. Status: design brief complete and ready for React/Tailwind build.

stephennickerson.com rebuild:
Rebuilding with Claude Designer. Refining Wound Hierarchy positioning, with scratches to knife wounds to hemorrhage mapped to Agent, Colony, Swarm tiers.

Three-Tier Service Framework:
Agent to Colony to Swarm. Refining the structure and Wound Hierarchy positioning that maps each tier to depth of organizational pain.

Capital Generation Lens:
Core entrepreneurial redirect: How does this generate capital today? Monetize what already exists before building what is next.

## Decisions & Analysis

Purpose: Resolved decision trees, capital grill reports, opportunity evaluations, and strategic memos. Every grill-me output, capital allocation decision, and opportunity memo lives here. These are thinking artifacts, distinct from projects, frameworks, and skills.

What belongs here:
- Capital Grill Reports.
- Architecture Grill Reports.
- Battle Plan Reports.
- Opportunity memos and re-evaluations.
- Strategic pivots with reasoning.

What does not belong here:
- Active project tracking goes in Projects.
- Reusable patterns or protocols go in Frameworks & Protocols.
- Skill definitions go in Skills Library.

### Capital Grill Report: Swarm Visibility & Utilization Engine

Date: May 10, 2026.
Tune: Capital / Munger.
Verdict: Capital-generating infrastructure. Ship it today.

Problem:
Stephen runs Claude Code instances across VPS machines on Linux and Windows for multiple clients. He lacks real-time visibility into agent work, whether Telegram commands landed, which agents are idle or working, and how much of the Claude Max 20x subscription is utilized. The current Telegram hook is fire-and-forget with no feedback loop.

Product:
A Rust application on each machine continuously aggregates data from Claude Code instances: JSON logs, running processes, window states, memory files, and MCP outputs. It persists data to a fully normalized database, exposes a queryable interface, handles auth, and can send commands to specific agent windows.

Audiences:
- Stephen globally sees all machines, clients, and agents. This enables a future Stephen-clone management agent.
- Clients see only their own agents, with peace of mind, working indicators, and stop notifications.

Core reframing:
This is not a dashboard or monitoring tool. It is a utilization optimization engine: a load balancer and dead-agent resurrector that keeps 20x Max subscriptions burning at 99% capacity across customers.

Capital logic:
Without it, unused tokens are wasted margin. With it, every token-hour converts to customer delivery at near-zero marginal cost.

Architecture:
- Fully normalized relational database with about 30 tables.
- Many-to-many relationships.
- Structured views for query performance.
- Access controlled through stored procedures executed by Rust.
- JSON is communication envelope, never storage.
- Boundary validation only.
- Entity spine: Machine to Client to Agent Folders to Agent Instances to Properties.
- Rust app inspects logs, processes, windows, persists data, handles auth, sends keypresses, and works on Linux and Windows.

Current assets:
- Telegram hook to shell to Claude Code is working but unhardened.
- JSON logs are available but not aggregated.
- Sendkeys proven on Linux.
- Normalized DB pattern proven multiple times.
- Rust binary compilation/distribution proven.
- Database architect agent exists.

Stephen-clone endgame:
A management agent monitors accounts, resurrects stalled agents, balances load across Max subscriptions, and handles routine operational decisions.

Open questions:
- Database engine choice.
- Windows sendkeys parity.
- Web UI design.
- Notification transport.
- Stephen-clone agent training timeline.

Verdict:
Capital-generating infrastructure. Economic engine underneath every product Stephen sells. Same-day build candidate. First-dollar impact is immediate.

### Capital Grill Report: Coaching Certification Brain Product

Date: May 10, 2026.
Tune: Capital / Munger.
Verdict: Capital-generating. Ship it.

Product:
An AI agent reviews coaching session transcripts for Brene Brown / Dare to Lead / Daring Way coach certifiers. It identifies coaching techniques used, highlights strengths, flags missed opportunities, and produces a certification-ready draft report. Certifiers use the draft to reduce a 2-hour review task to 30 minutes or less.

Brain:
The core asset is a compiled Rust binary containing 25 years of domain-specific coaching evaluation knowledge, built by Stephen using three AI agents and two skills AI does not yet possess. The brain is distributed read-only via MCP to any Claude Code instance needing coaching capability. It is the moat.

Capital timing:
First dollar expected May 2026. Helene is prototype customer. Her Monday May 12 contact is going through early recertification under new AI-era policies and can become a customer/referral source if the demo impresses.

June forcing function:
New recertification policies with increased complexity land in June 2026. Stephen has built the mind map graph of the latest documentation.

Unit economics:
- Claude Max 20x subscription: $300/month.
- VPS hosting: $12/month.
- Total cost per 20 clients: $312/month.
- Revenue at $1,500/client x 20: $30,000/month.
- Revenue at $3,000/client x 20: $60,000/month.
- Gross margin: 99%+.

Pricing:
$1,500 prices to time savings. $3,000+ prices to revenue enablement. The product allows certifiers to certify more coaches, unlocking revenue capacity. Price should reflect revenue enabled, not hours saved.

Referral structure:
15% commission shared between Helene and the contact for qualified 3-way calls that lead to sale.

Defensibility:
Surface-level transcript scanners are easy. Certification-grade evaluation requires the 25-year brain. Corrections from multiple certifiers can become a compounding moat.

Delivery:
- One Claude Code agent per customer.
- Private folder with memory and sub-agents.
- Read-only MCP coaching brain.
- Customer data backed up to GitHub.
- VPS at $12/month per 20-client block.
- Managed service keeps secret sauce with Stephen.

Open questions:
- What does a certifier earn per coach certified?
- Final price point.
- Client count before second Max subscription.
- IP/licensing considerations with Brene Brown Education and Research Group.

### AI Product Opportunity Memo - April 30 Re-evaluation

Decision:
Earlier choices were directionally right, but ranking changes after a broader pass. The best overall company thesis is AI Systems Architect Copilot for service businesses. AI Growth Concierge OS should be the first wedge, not the whole company.

Revised ranking:
1. AI Systems Architect Copilot for service SMBs.
2. AI Growth Concierge OS for coaches, consultants, and cohorts.
3. FOMO Wine as strongest non-software business contender.
4. AI Speaker, Webinar, and Shared-Promotion Orchestrator.
5. Vertical AI Concierge Recruiting.

Why Systems Architect moved to number one:
The recurring pattern is the ability to inspect messy business systems, find bottlenecks, design processes, and turn them into operating rhythms. This appears across the archive.

Why Growth Concierge remains the first wedge:
Coach, consultant, webinar, and cohort material is immediately sellable because pain is concrete: intake, qualification, onboarding, outreach, accountability, follow-up, and KPI reporting.

FOMO Wine:
Clear consumer positioning and more complete brand/product material than most archive ideas, but CPG brings production, health-claim, compliance, distribution, and capital constraints.

Trading bots:
Technically substantial but crowded, exchange-native competitors are strong, and financial/compliance risk is high.

Generic AI customer support:
Real problem, but dense/platform-dominated category unless paired with a specific vertical or proprietary data angle.

Bottom line:
Build broader AI Systems Architect through narrower AI Growth Concierge wedge.

### Architecture Grill Report: Carter - Master Coach System

Verdict: Sound. Ship it.

Problem:
Build an AI coaching system that coaches clients directly and evaluates coach conversation transcripts for certification. Target users are coach certifiers, starting with Helene, who spend 2+ hours per transcript review. Target outcome: certification-grade draft report in 30 minutes or less. Pricing floor: $1,500/month per certifier.

Root agent:
Carter. Behavioral identity: Marshall Goldsmith: stakeholder-centered, action-oriented, behavioral-change-driven. Not Brene Brown, not Carl Rogers. Carter asks what the client is working on and routes to the right sub-agent team.

Org chart:
```text
Carter root
  Coaching Team
    Goldsmith lead: executive coaching, behavioral triggers, feedforward
    Whitmore sub-agent: GROW model, structured questioning
    Rogers sub-agent: reflective listening, unconditional positive regard
  Evaluator Team
    Reynolds lead: ICF competency-level transcript analysis
    Hawkins sub-agent: seven-eyed supervision model
    Clutterbuck sub-agent: mentoring/coaching quality assessment
```

Key architecture decision:
Use sub-agents, not teams. Agent teams spin up with full default context and waste money on repeated initialization. Sub-agents are leaner and controllable. All specialist work runs through sub-agents.

CLAUDE.md inheritance:
- Inheritance is additive, not replacive.
- Recency takes behavioral precedence.
- Identity override works.
- All files are visible.
- .claude/claude.md should not be used because it confuses inheritance.

Thin Boot Pattern:
Root CLAUDE.md stays small. Every token in root is inherited by every sub-agent as dead weight. Root should contain identity, Solid Rock Protocol, instruction that all capabilities live in skills, and shared voice/output defaults. Everything else moves to skills.

Sub-agent prompts:
Identity override, explicit skill list, explicit tool allowlist, and no inherited knowledge that is not operationally needed.

Master Coach MCP:
Read-only Rust binary containing 25 years of coaching evaluation expertise. All six agents have MCP access. Read-only means no write contention. The brain is the moat.

Model selection:
Parent Carter on Opus 4.6. Sub-agents can run on Opus 4.7 or other models. Managing Partner pattern: MP layer optimized for alignment and low annoyance; worker layer optimized for capability per dollar.

Open items:
- Carter final name.
- Skill files for each sub-agent.
- Sub-agent spawn prompts.
- Testing sequence.
- Evaluator identity depth.
- Tool and skill audit.
- Manually spawned sub-agent pattern for clean leaf-node agents.

Capital summary:
First dollar this month. $312 cost floor. $30k+ revenue ceiling per 20-client block at $1,500/month per certifier. Price reflects revenue enabled, not hours saved.

## Projects Database

Schema:
- Project: title.
- Client: Internal, RadicalSimplicity.AI, FFC / Helene, E4B / Mandie.
- Status: Active, Concept, Shipped, Paused, Archived.
- Summary: text.
- Tags: multi-agent, cloudflare, web, content, ops, persona, pipeline, infrastructure, positioning.
- Tier: Agent, Colony, Swarm, Infrastructure, Brand, Engagement.
- URL: page URL.

Rows:

### E4B Checkout
- Client: E4B / Mandie.
- Status: Concept.
- Tier: Infrastructure.
- Tags: web, ops.
- Summary: DIY Stripe Elements + Apple Pay on Firebase to webhook to Podia. Rejected SamCart/Stan. Next: architecture spec.
- URL: https://www.notion.so/3594b2507c2081a3bd31e143e67baaab

### FFC Agent Fleet
- Client: FFC / Helene.
- Status: Shipped.
- Tier: Colony.
- Tags: multi-agent, persona.
- Summary: Louis, Alignify, Wizard, Harmony, Mike. Iterative builds for FFC.
- URL: https://www.notion.so/3594b2507c208167abf0c6c8b7dfb243

### FFC Loom Video Pipeline
- Client: FFC / Helene.
- Status: Shipped.
- Tier: Infrastructure.
- Tags: pipeline, content.
- Summary: CLAUDE.md + design-brief-template.md. Semantically extracts frame-accurate screenshots from Loom recordings for design briefs. Used for Helene's review recordings.
- URL: https://www.notion.so/3594b2507c20817d9f64f6b57299b5fc

### FFC Static Site Migration
- Client: FFC / Helene.
- Status: Shipped.
- Tier: Brand.
- Tags: web, infrastructure, cloudflare.
- Summary: Cloudflare Worker reverse proxy routing migrated pages to static HTML. Manifest-driven routing controls Worker, sitemap.xml, llms.txt. Visual Verification Protocol with exact constants, 5-step sequence, 9-point checklist, and Phase 0 tool validation gate. Encoded in merged CLAUDE.md and README.md in March 2026.
- URL: https://www.notion.so/3594b2507c20815db889eff78a19a80d

### Marketing Intelligence Operating System
- Client: Internal.
- Status: Active.
- Tier: Engagement.
- Tags: content, pipeline, ops, positioning.
- Summary: Two-part marketing system: doctrine/training book plus repeatable daily workflow for 24-hour public pulse research, article-ready briefs, source drafts, and authored final articles.
- URL: https://www.notion.so/35a4b2507c2081faa58ed1c7b33abd06

Details:
Marketing Intelligence Operating System is the working marketing pipeline Stephen is building for himself, clients, and friends. It combines a Marketing Series reference book with a daily intelligence workflow that turns current public reality into article-ready briefs and authored promotional articles.

Products:
- Marketing Series Modern Examples: doctrine/training reference for marketing, promotion, dissemination, surveys, buttons, message, positioning, demand creation, response measurement, and related concepts.
- Daily Marketing Intelligence: operating workflow with baseline research, 24-hour public pulse research, article-ready brief creation, source draft generation, author-subagent rewriting, verification, commit/push history.

Current working example: Function First Coaching / DICA 2 Full produced a 90-day baseline, daily public-pulse brief, article-source draft, and authored final article using hardened author prompt.

Operating rule: every document-changing step gets committed and pushed immediately. History is part of the product because it preserves drafts, comparisons, prompt failures, and improvements.

### Self-Assembling AI Org Architecture
- Client: Internal.
- Status: Active.
- Tier: Swarm.
- Tags: multi-agent.
- Summary: v3.0 live, v3.1 in progress. Agents instantiate themselves into a functional org using Universal Agent Definition Template with 12 sections: Identity, Purpose, Locus, Jurisdiction, Instruments, Protocol, Criteria, Discourse, Chain, Vigil, Ledger, Mutation. FFC deployment used Brene Brown as founder persona and produced 84 agents with 135 communication contracts.
- URL: https://www.notion.so/3594b2507c208142aea1d3af8c521d8e

### Board of Advisor Agents
- Client: Internal.
- Status: Active.
- Tier: Colony.
- Tags: multi-agent, persona.
- Summary: Innovation Department board. Michael Gerber as CEO/Founder Agent. Beer, Meadows, Conway as advisory board members.
- URL: https://www.notion.so/3594b2507c208116bcecc073e8fa6c42

### Synchronous Hook Dispatcher
- Client: Internal.
- Status: Shipped.
- Tier: Infrastructure.
- Tags: infrastructure, ops.
- Summary: Rust binary + JS handlers, one per event type. Context-limit restart loop: at 180k tokens a Stop hook injects a handover prompt, the agent writes a handover document, the orchestrator restarts fresh. Agents never resume; they restart clean.
- URL: https://www.notion.so/3594b2507c208154934bee1b0fc9b3aa

### Three-Tier Service Framework
- Client: RadicalSimplicity.AI.
- Status: Active.
- Tier: Brand.
- Tags: positioning.
- Summary: Agent to Colony to Swarm. Service tier structure mapped to organizational pain depth via the Wound Hierarchy.
- URL: https://www.notion.so/3594b2507c20812ba214c23e6bf5739b

### Mike (Telegram Persona)
- Client: Internal.
- Status: Active.
- Tier: Agent.
- Tags: persona, multi-agent.
- Summary: Telegram-delivered agent persona. Single Anthropic-built JS plugin running alongside Claude Code. Watchdog and context-threshold logic added to this existing JS file, not as a separate daemon.
- URL: https://www.notion.so/3594b2507c20813bb9e8e2e873f7bb0e

### Swarm V5
- Client: Internal.
- Status: Active.
- Tier: Swarm.
- Tags: multi-agent, infrastructure, ops.
- Summary: Multi-agent fleet on Claude Code Linux. Database-driven, MCP-based dispatcher. Model-agnostic; switching models is one DB row change. All Claude Code CLI flags stored in DB tables for dynamic optimization by a watchtower agent.
- URL: https://www.notion.so/3594b2507c2081c89ccfd647eab6c9c0

### Multi-Account Claude Code Setup
- Client: Internal.
- Status: Shipped.
- Tier: Infrastructure.
- Tags: infrastructure, ops.
- Summary: Linux. CLAUDE_CONFIG_DIR + shell aliases for isolation. VS Code extension workarounds explored.
- URL: https://www.notion.so/3594b2507c20811ca76bf8f2b240ff21

### Telegram Memory Compression Pipeline
- Client: Internal.
- Status: Shipped.
- Tier: Infrastructure.
- Tags: pipeline, persona.
- Summary: CLAUDE.md spec, SQLite, multi-pass extraction. Conversation memory compression for Telegram-delivered agents.
- URL: https://www.notion.so/3594b2507c2081d49732da814365bc56

### Admin Scale Graph Inspector
- Client: Internal.
- Status: Concept.
- Tier: Infrastructure.
- Tags: web, multi-agent.
- Summary: Graph-based, card-based infinite-drilldown UI for inspecting organizations as recursive special team structures. Grounded in Admin Scale dimensions. Real-time organizational coherence auditor. Design brief complete and ready for React/Tailwind build.
- URL: https://www.notion.so/3594b2507c2081d588bacdae1816b08f

### MMS (Mind Management System)
- Client: Internal.
- Status: Active.
- Tier: Infrastructure.
- Tags: cloudflare, infrastructure.
- Summary: Cloudflare D1/KV/R2-based mind management system. Part of Cloudflare-as-nervous-system architecture.
- URL: https://www.notion.so/3594b2507c208139be42efafd0b581f4

### Agents for Everybody
- Client: RadicalSimplicity.AI.
- Status: Active.
- Tier: Engagement.
- Tags: persona, content.
- Summary: Engagement model: recorded extraction interview whose transcript becomes the foundation for a self-configuring Claude agent. Two-stage discovery/engagement fee structure.
- URL: https://www.notion.so/3594b2507c208131bae4d114e3703624

### stephennickerson.com Rebuild
- Client: RadicalSimplicity.AI.
- Status: Active.
- Tier: Brand.
- Tags: web, positioning.
- Summary: Rebuilding with Claude Designer. Wound Hierarchy positioning mapped to Agent, Colony, Swarm tiers. Cloudflare AI bot blocking issue resolved during build.
- URL: https://www.notion.so/3594b2507c20812cab66d095ec023a31

## Frameworks & Protocols Database

Schema:
- Name: title.
- Domain: agents, voice, ops, decisions, architecture, persona, positioning, communication.
- Origin: Stephen, Adapted, External.
- Type: Protocol, Pattern, Methodology, Principle, Template, Heuristic.
- Summary: text.
- URL: page URL.

Rows:

### Generator Suite Pattern
- Domain: architecture.
- Origin: Stephen.
- Type: Pattern.
- Summary: One architecture, many parameterized configurations. Coined about 7 years ago, now manifesting again as the agent fleet. Throughline across all work.

### Triangulated Architecture Protocol
- Domain: agents, decisions.
- Origin: Stephen.
- Type: Protocol.
- Summary: Claude drafts. GPT/Gemini/sub-agent reviews. Cross-model review turns disagreement into signal.

### Narrowest Scope Principle
- Domain: architecture.
- Origin: Stephen.
- Type: Principle.
- Summary: Place configuration at the narrowest scope where it is actually used. Method-level, class-level, file-level, global. Zero magic numbers in business logic.

### Closed-System Principle
- Domain: architecture.
- Origin: Stephen.
- Type: Principle.
- Summary: True Zero Error Handling. In a system fully controlled end-to-end, defensive code is dead weight. Validation belongs at the system boundary; inside, trust the pipeline.

### Solid Rock Protocol
- Domain: decisions, ops.
- Origin: Stephen.
- Type: Protocol.
- Summary: Clarify objective, evaluate options by certainty of progress, weight toward strongest foundation, execute, repeat. Escalation rule: when uncertain, ask to clarify the objective, never which step.

### Visual Verification Protocol
- Domain: ops.
- Origin: Stephen.
- Type: Protocol.
- Summary: Exact constants including LSCWP_CTRL before_optm cache-bust and 2000x1200 viewport. 5-step sequence. 9-point checklist. Phase 0 tool validation gate. Built for FFC migration.

### FRANK Methodology / Persona Wizard
- Domain: persona, voice.
- Origin: Stephen.
- Type: Methodology.
- Summary: 5 masters to one distilled trait each to synthesized named deployable persona. Origin of FRANK.

### Universal Agent Definition Template
- Domain: agents, persona.
- Origin: Stephen.
- Type: Template.
- Summary: 12 sections: Identity, Purpose, Locus, Jurisdiction, Instruments, Protocol, Criteria, Discourse, Chain, Vigil, Ledger, Mutation. Substrate for self-assembling agents.

### Admin Scale (10 dimensions)
- Domain: ops, decisions.
- Origin: External.
- Type: Methodology.
- Summary: Goals, Purposes, Policy, Plans, Programs, Projects, Orders, Ideal Scenes, Statistics, VFPs. Source for the Admin Scale Graph Inspector from the intel MCP knowledge base.

### Ralph Loop
- Domain: agents, ops.
- Origin: Stephen.
- Type: Pattern.
- Summary: Iterative skill improvement pattern. Used for refining SKILL.md files and other agent capabilities through repeated execution-and-revise cycles.

### Anti-Consultant Positioning
- Domain: positioning.
- Origin: Stephen.
- Type: Principle.
- Summary: Capability transfer, not dependency creation. Foundational to the brand.

### Locality of Behavior
- Domain: architecture.
- Origin: Stephen.
- Type: Principle.
- Summary: Code that changes together lives together. Cleverness via indirection costs more than it saves.

### Wound Hierarchy
- Domain: positioning.
- Origin: Stephen.
- Type: Methodology.
- Summary: Scratches to knife wounds to hemorrhage. Mapped to Agent, Colony, Swarm service tiers. Pain-depth positioning for stephennickerson.com.

### Clear [word] Command
- Domain: voice, communication.
- Origin: Stephen.
- Type: Methodology.
- Summary: Etymological breakdown. Trace roots, break prefix/root/suffix, contrast true vs modern meaning, go 2-3 levels deep.

### Classify-by-Committee, Respond-by-Persona
- Domain: agents, communication.
- Origin: Stephen.
- Type: Pattern.
- Summary: Email architecture. A committee of agents classifies the inbound; the matching persona responds.

### Problem-Earned Code
- Domain: architecture.
- Origin: Stephen.
- Type: Principle.
- Summary: Stricter than YAGNI. Do not write a line until reality has handed the problem to this codebase with a real use case. No solutions without problems. No abstraction without lived demand.

### Voice DNA Framework
- Domain: voice, persona.
- Origin: Stephen.
- Type: Methodology.
- Summary: Foundational to brand voice work. Captures and reproduces a person's distinct cadence and word choice.

### Solve Once, Configure Forever
- Domain: architecture.
- Origin: Stephen.
- Type: Principle.
- Summary: Solve the real problem as a configurable instance of a general pattern, so future variations are configuration changes, not new code. The third path between speculative abstraction and pure YAGNI. DRY as bug prevention by structure.

## Skills Library Database

Schema:
- Skill: title.
- Domain: Architecture, Voice, Brand, Research, Ops, Persona.
- Status: Active, Draft, Archived.
- Purpose: text.
- Trigger: text.
- URL: page URL.

Rows:

### stephen-email-voice
- Domain: Voice.
- Status: Active.
- Purpose: Stephen's email voice. No em dashes. No projection. Michel Thomas structure: Anchor to Sequence to Weight to Bridge to Space.
- Trigger: Any email being drafted as Stephen.

### ffc-brand-architect
- Domain: Brand.
- Status: Active.
- Purpose: Senior language architect for Function First Coaching and Dive Into a Coach Approach. Bloom, Willison, Goodside operating sequence.
- Trigger: /ffc, /helene, /dica, any FFC content, DICA modules, Coaching Spectrum Framework, AOTA compliance, learning objectives.

### playbook-architect
- Domain: Ops.
- Status: Active.
- Purpose: Transform messy procedural knowledge into bulletproof SOPs and decision flowcharts. HRO methodology, Simplified Technical English.
- Trigger: /playbook, document this process, create an SOP, or procedural narrative without structure.

### chaos-to-clarity
- Domain: Architecture.
- Status: Active.
- Purpose: Transform unstructured information into rigorous, actionable strategic frameworks using cognitive science principles such as Miller's Law, Dual Coding, MECE.
- Trigger: Help me explain this, turn this chaos into clarity, I need a framework for this.

### strategic-architect
- Domain: Architecture.
- Status: Active.
- Purpose: TypeScript monolith modernization through virtual decomposition. Identifies logical clusters, breaks circular dependencies, decomposes monoliths without physical file splitting.
- Trigger: analyze this monolith, find the clusters, refactor this file, decompose.

### case-study-architect
- Domain: Brand.
- Status: Active.
- Purpose: Forensic-grade B2B case studies that function as commercial proof, not marketing fluff. Extracts buried metrics and navigates NDA constraints.
- Trigger: case study, customer success story, client story, proof point.

### white-paper-architect
- Domain: Brand.
- Status: Active.
- Purpose: Transform research, analysis, and insights into publishable B2B thought leadership white papers. Enforces thesis validation, data verification, anti-buzzword discipline.
- Trigger: whitepaper, thought leadership piece, turn this into a paper.

### exa-research
- Domain: Research.
- Status: Active.
- Purpose: Exa-powered research patterns for in-depth web search and synthesis.
- Trigger: Deep research tasks beyond a single web_search call.

### clarity
- Domain: Persona.
- Status: Active.
- Purpose: Transform rough ideas into comprehensive research prompts for deep research on specialist types/roles.
- Trigger: research a specialist, rambling descriptions of expert types to investigate.

### data-evaluator
- Domain: Ops.
- Status: Active.
- Purpose: Root-cause analysis using Data Series 14 outpoints and 14 pluspoints. Surfaces a real Why and a numbered handling program.
- Trigger: figure out what is really going on, why is X happening, diagnose, evaluate.

### autonomous-boardroom
- Domain: Architecture.
- Status: Active.
- Purpose: Architecture guide for autonomous multi-agent systems with recursive feedback loops under $100/month budget.
- Trigger: Multi-agent debate systems, recursive AI workflows, budget-constrained agentic systems.

### prompt-enhancer
- Domain: Voice.
- Status: Active.
- Purpose: Transform vague or ambiguous prompts into clear, actionable requests.
- Trigger: /enhance command or ambiguous requests lacking clear objectives.

### antigravity-migration
- Domain: Ops.
- Status: Active.
- Purpose: Migrate bloated WordPress/Elementor sites to clean Next.js/Tailwind. Treat source as Visual Spec only.
- Trigger: migrate site, antigravity URL, convert WordPress to Next.js.

### d1-database-architect
- Domain: Architecture.
- Status: Active.
- Purpose: Cloudflare D1 database design, schema architecture, and query patterns. Enforces singular naming, forward-keyed relationships, mandatory index coverage on FK columns, view-first query design.
- Trigger: design a database, create a schema, add a table, D1 schema, or any new feature implying persistent state.

## Coverage Notes

Pages fetched and included:
- Root page.
- Background & Identity.
- Architecture Principles.
- Fleet Router.
- Clients & Engagements.
- Top of Mind.
- Decisions & Analysis.
- Four Decisions & Analysis child pages.
- Projects database schema plus 17 project rows returned by database search.
- Frameworks & Protocols database schema plus 18 framework/protocol rows returned by database search.
- Skills Library database schema plus 14 skill rows returned by database search.
- Architecture Principles attachments added afterward: CODING_STANDARDS.md and SKILL.md.

Connector limitation observed:
The Notion database query tool was advertised in the tool schema but returned "Tool notion-query-data-sources not found" when called. Database item coverage was therefore collected through data-source scoped Notion search plus direct page fetch. The connector did not return binary/file attachment bodies for the two file references on Architecture Principles, so the canonical local copies were ingested into Cognee separately.
