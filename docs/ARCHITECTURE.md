\# Traqr Cloud Architecture (v1)



\## Goals

\- Multi-tenant: organizations → (optional) franchises → stores

\- Cloud portal for ops/reporting/support

\- Device-authoritative POS with offline-first incremental sync

\- Menu publishing by head office; store can override availability only

\- Two-person approval for sensitive actions (refund/void/order-changing commands)



\## Services (v1)

Single Rust workspace/monolith:

\- cloud\_api (Axum HTTP API)

\- domain (business rules + shared types)

\- db (SQLx queries + migrations runner)

\- auth (sessions/JWT, membership checks, device auth)

\- sync (event ingestion + command delivery)

\- ui (marketing + portal pages with Tailwind)

\- billing (plan/entitlement gating)

\- storage (S3-compatible object storage wrapper)



\## Sync Model

\- Device → Cloud: append-only events (idempotent)

\- Cloud → Device: commands queue (delivered by polling)

\- Sensitive commands require two-person approval



