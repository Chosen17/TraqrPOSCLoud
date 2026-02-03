\# Sync Protocol (v1)



\## Device Authentication

\- Device enters activation key in POS UI

\- POS calls POST /device/activate with local device id + activation key

\- Cloud validates entitlement + scope and returns device\_token

\- Device uses device\_token for sync calls



\## Endpoints (device)

\- POST /sync/events

&nbsp; - body: { last\_ack\_seq, events\[] }

&nbsp; - cloud inserts idempotently (unique by device\_id + event\_id)

&nbsp; - response: { ack\_seq }



\- GET /sync/commands?limit=50

&nbsp; - returns deliverable commands (approved/queued) for device



\- POST /sync/commands/ack

&nbsp; - body: { command\_id, status: acked|failed, result }



\## Command approvals

\- Sensitive commands require approvals by two distinct users

\- Approvals recorded in approvals table

\- Only then does a command become deliverable



