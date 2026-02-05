//! Billing & entitlements helpers.
//!
//! For now we only implement a simple org-level check for the "cloud_sync"
//! add-on, keyed by `plans.code = 'cloud_sync'`. An entitlement is considered
//! active if there is an `org_entitlements` row for the org + plan where
//! `valid_until` is NULL or in the future.

use sqlx::MySqlPool;
use uuid::Uuid;

/// Returns true if the given org currently has an active entitlement for the
/// provided plan code (e.g. "cloud_sync").
pub async fn has_active_entitlement(
    pool: &MySqlPool,
    org_id: Uuid,
    plan_code: &str,
) -> Result<bool, sqlx::Error> {
    // We deliberately keep this logic minimal for v1:
    // - Join plans by code
    // - Check org_entitlements for that plan + org
    // - Require valid_until IS NULL or in the future
    //
    // This matches the "Cloud Sync" add-on model where a subscription period
    // is represented by valid_from/valid_until.
    let (exists,): (i64,) = sqlx::query_as(
        r#"
        SELECT EXISTS(
          SELECT 1
          FROM org_entitlements oe
          JOIN plans p ON p.id = oe.plan_id
          WHERE oe.org_id = ?
            AND p.code = ?
            AND (oe.valid_until IS NULL OR oe.valid_until > CURRENT_TIMESTAMP(3))
        ) AS has_entitlement
        "#,
    )
    .bind(org_id.to_string())
    .bind(plan_code)
    .fetch_one(pool)
    .await?;

    Ok(exists != 0)
}

/// Grant the org an active entitlement for the given plan code (e.g. "cloud_sync").
/// Idempotent: if a row exists for org+plan, valid_until is set to NULL (no expiry).
pub async fn grant_entitlement(
    pool: &MySqlPool,
    org_id: Uuid,
    plan_code: &str,
) -> Result<(), sqlx::Error> {
    // Look up plan id as string to match the CHAR(36) schema.
    let plan_id: (String,) = sqlx::query_as("SELECT id FROM plans WHERE code = ?")
        .bind(plan_code)
        .fetch_one(pool)
        .await?;
    sqlx::query(
        r#"
        INSERT INTO org_entitlements (org_id, plan_id, cloud_sync_add_on, device_limit, valid_from, valid_until)
        VALUES (?, ?, 1, NULL, CURRENT_TIMESTAMP(3), NULL)
        ON DUPLICATE KEY UPDATE
          cloud_sync_add_on = VALUES(cloud_sync_add_on),
          valid_until = VALUES(valid_until)
        "#,
    )
    .bind(org_id.to_string())
    .bind(plan_id.0)
    .execute(pool)
    .await?;
    Ok(())
}

/// Suspend Cloud Sync for an org (e.g. for non-payment). Sets valid_until to now
/// so has_active_entitlement returns false and sync endpoints return 403.
/// No-op if org has no cloud_sync entitlement.
pub async fn suspend_cloud_sync(pool: &MySqlPool, org_id: Uuid) -> Result<bool, sqlx::Error> {
    let result = sqlx::query(
        r#"
        UPDATE org_entitlements oe
        JOIN plans p ON p.id = oe.plan_id AND p.code = 'cloud_sync'
        SET oe.valid_until = CURRENT_TIMESTAMP(3)
        WHERE oe.org_id = ?
        "#,
    )
    .bind(org_id.to_string())
    .execute(pool)
    .await?;
    Ok(result.rows_affected() > 0)
}

/// Reactivate Cloud Sync for an org (e.g. after payment). Sets valid_until to NULL.
/// If no org_entitlements row exists, creates one (same as grant).
pub async fn reactivate_cloud_sync(pool: &MySqlPool, org_id: Uuid) -> Result<(), sqlx::Error> {
    let plan_id: (String,) = sqlx::query_as("SELECT id FROM plans WHERE code = 'cloud_sync'")
        .fetch_one(pool)
        .await?;
    let result = sqlx::query(
        r#"
        UPDATE org_entitlements
        SET valid_until = NULL
        WHERE org_id = ? AND plan_id = ?
        "#,
    )
    .bind(org_id.to_string())
    .bind(&plan_id.0)
    .execute(pool)
    .await?;
    if result.rows_affected() == 0 {
        // No row to update: grant a new entitlement.
        grant_entitlement(pool, org_id, "cloud_sync").await?;
    }
    Ok(())
}

