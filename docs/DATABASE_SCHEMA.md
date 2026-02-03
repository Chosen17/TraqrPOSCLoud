\# Database Schema (v1)



\## Tenancy

\- organizations

\- franchises

\- stores



\## Auth

\- cloud\_users

\- cloud\_roles

\- org\_memberships

\- store\_memberships

\- auth\_sessions



\## Billing / Entitlements

\- plans

\- org\_entitlements

\- purchases



\## Devices

\- devices

\- device\_activation\_keys

\- device\_tokens



\## Menus

\- menu\_drafts

\- menu\_publishes (monotonic publish\_id)

\- store\_menu\_overrides (availability-only, history by publish\_id)



\## Orders Replicas

\- orders

\- order\_items

\- transactions

\- receipts

\- order\_events (append-only timeline)



\## Sync

\- device\_event\_log

\- device\_sync\_state

\- device\_command\_queue

\- approvals



