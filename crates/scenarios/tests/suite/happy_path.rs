use scenarios::*;
use sp_common::types::{PoolState, Tier};

#[test]
fn full_flow_with_tier_refunds_leaves_no_dust() {
    let c = setup();
    let pool = c.pool(100);
    let [a, b, cc, d] = c.fill_standard(pool);

    // Members paid the ceiling price at commit time.
    assert_eq!(c.gb.commitment(&pool, &a).unwrap().paid, 60 * USDC);
    assert_eq!(c.gb.commitment(&pool, &cc).unwrap().paid, 100 * USDC);
    assert_eq!(c.gb.commitment(&pool, &d).unwrap().paid, 180 * USDC);
    let p = c.gb.pool(&pool);
    assert_eq!(p.state, PoolState::Filled);
    assert_eq!(p.total_units, 420);
    assert_eq!(p.final_price, USDC * 8 / 10);
    assert_eq!(c.bal(&c.gb.address), 400 * USDC);

    c.deliver(pool, 420);
    for m in [&a, &b, &cc, &d] {
        c.gb.confirm_pickup(m, &pool);
    }
    // 100% pickup weight releases early.
    c.gb.settle(&pool);

    let gross = 336 * USDC; // 420 * 0.8
    let plat = gross * 150 / 10_000;
    let org = gross * 100 / 10_000;
    assert_eq!(c.bal(&c.treasury), plat);
    assert_eq!(c.bal(&c.organizer), org);
    assert_eq!(c.bal(&c.supplier), 1_000 * USDC + gross - plat - org);

    // Tier-difference refunds: 12, 12, 20, 20 USDC.
    assert_eq!(c.gb.claim_refund(&a, &pool), 12 * USDC);
    assert_eq!(c.gb.claim_refund(&b, &pool), 12 * USDC);
    assert_eq!(c.gb.claim_refund(&cc, &pool), 20 * USDC);
    assert_eq!(c.gb.claim_refund(&d, &pool), 20 * USDC);
    assert_eq!(c.bal(&c.gb.address), 0);
    assert_eq!(c.gb.pool(&pool).state, PoolState::Settled);
    assert!(c.gb.try_claim_refund(&a, &pool).is_err());
}

#[test]
fn settles_after_window_without_pickups() {
    let c = setup();
    let pool = c.pool(0);
    c.fill_standard(pool);
    c.deliver(pool, 420);
    assert!(c.gb.try_settle(&pool).is_err());
    c.advance(48 * 3600);
    c.gb.settle(&pool);
    assert_eq!(c.gb.pool(&pool).state, PoolState::Settled);
}

#[test]
fn push_refunds_pays_everyone_in_batches() {
    let c = setup();
    let pool = c.pool(0);
    let t = c.fill_standard(pool);
    c.deliver(pool, 420);
    c.advance(48 * 3600);
    c.gb.settle(&pool);
    let before: i128 = t.iter().map(|m| c.bal(m)).sum();
    assert_eq!(c.gb.push_refunds(&pool, &3), 3);
    assert_eq!(c.gb.push_refunds(&pool, &3), 1);
    assert_eq!(c.gb.push_refunds(&pool, &3), 0);
    let after: i128 = t.iter().map(|m| c.bal(m)).sum();
    assert_eq!(after - before, 64 * USDC);
    assert_eq!(c.gb.sweep_dust(&pool), 0);
}

// Issue #147: refunds_pending view tracks keeper progress across batches
#[test]
fn refunds_pending_decrements_with_each_push_batch() {
    let c = setup();
    let pool = c.pool(0);
    let _t = c.fill_standard(pool);
    c.deliver(pool, 420);
    c.advance(48 * 3600);
    c.gb.settle(&pool);

    // Before any batch: all 4 members are pending.
    let (rem, amt) = c.gb.refunds_pending(&pool);
    assert_eq!(rem, 4, "all members should be pending before first push");
    assert!(amt > 0, "pending amount should be positive");

    // Push 3: 1 member left.
    c.gb.push_refunds(&pool, &3);
    let (rem2, amt2) = c.gb.refunds_pending(&pool);
    assert_eq!(rem2, 1);
    assert!(amt2 < amt);

    // Push the final batch: 0 members left, 0 amount.
    c.gb.push_refunds(&pool, &3);
    let (rem3, amt3) = c.gb.refunds_pending(&pool);
    assert_eq!(rem3, 0);
    assert_eq!(amt3, 0);
}

// refunds_pending returns (0, 0) when pool is not in a refundable state
#[test]
fn refunds_pending_is_zero_for_non_refundable_pool() {
    let c = setup();
    let pool = c.pool(0);
    c.fill_standard(pool);
    // Pool is Filled — not yet in a refundable state.
    let (rem, amt) = c.gb.refunds_pending(&pool);
    assert_eq!(rem, 0);
    assert_eq!(amt, 0);
}

#[test]
fn expired_pool_refunds_in_full() {
    let c = setup();
    let pool = c.pool(0);
    let a = c.trader();
    let b = c.trader();
    c.gb.commit(&a, &pool, &40);
    c.gb.commit(&b, &pool, &30);
    assert!(c.gb.try_close(&pool).is_err()); // before the deadline
    c.advance(3 * DAY);
    c.gb.close(&pool);
    assert_eq!(c.gb.pool(&pool).state, PoolState::Expired);
    assert_eq!(c.gb.claim_refund(&a, &pool), 40 * USDC);
    assert_eq!(c.gb.push_refunds(&pool, &10), 2);
    assert_eq!(c.bal(&b), 1_000 * USDC);
    assert_eq!(c.bal(&c.gb.address), 0);
}

#[test]
fn close_early_and_auto_fill_at_max() {
    let c = setup();
    let pool = c.pool(0);
    let a = c.trader();
    c.gb.commit(&a, &pool, &100);
    assert!(c.gb.try_close_early(&c.organizer, &pool).is_ok());
    assert_eq!(c.gb.pool(&pool).state, PoolState::Filled);

    let pool2 = c.pool(0);
    let t = [c.trader(), c.trader()];
    c.gb.commit(&t[0], &pool2, &250);
    c.gb.commit(&t[1], &pool2, &250);
    assert_eq!(c.gb.pool(&pool2).state, PoolState::Filled);
}

#[test]
fn early_release_at_exact_weight_threshold_and_rounding() {
    let c = setup();
    let mut terms = c.terms();
    terms.moq = 10;
    terms.max_units = 10;
    terms.max_per_member = 5;
    terms.tiers = soroban_sdk::vec![
        &c.env,
        Tier {
            min_units: 10,
            unit_price: USDC,
        },
    ];
    let dl = c.now() + 2 * DAY;
    let pool = c
        .gb
        .create_pool(&c.organizer, &terms, &h(&c.env, 7), &0, &None, &dl);

    let m1 = c.trader();
    let m2 = c.trader();
    let m3 = c.trader();

    c.gb.commit(&m1, &pool, &5);
    c.gb.commit(&m2, &pool, &1);
    c.gb.commit(&m3, &pool, &4);

    // Pool reaches 10 units -> Filled
    assert_eq!(c.gb.pool(&pool).state, PoolState::Filled);

    // Deliver
    c.deliver(pool, 10);
    assert_eq!(c.gb.pool(&pool).state, PoolState::Delivered);

    // Confirmation window is still active:
    // With 0 confirmed pickups: settle fails with SettleTooEarly
    let err0 = c.gb.try_settle(&pool).err();
    assert_eq!(
        err0,
        Some(Ok(soroban_sdk::Error::from_contract_error(
            group_buy::Error::SettleTooEarly as u32
        )))
    );

    // 5 confirmed units (one unit below 60% of 10) fails with SettleTooEarly
    c.gb.confirm_pickup(&m1, &pool);
    let err5 = c.gb.try_settle(&pool).err();
    assert_eq!(
        err5,
        Some(Ok(soroban_sdk::Error::from_contract_error(
            group_buy::Error::SettleTooEarly as u32
        )))
    );

    // 6 confirmed units (exactly 60% of 10: 5 + 1) succeeds
    c.gb.confirm_pickup(&m2, &pool);
    assert!(c.gb.try_settle(&pool).is_ok());
    assert_eq!(c.gb.pool(&pool).state, PoolState::Settled);

    // --- 7-unit pool to test rounding ---
    let mut terms7 = c.terms();
    terms7.moq = 7;
    terms7.max_units = 7;
    terms7.max_per_member = 3;
    terms7.tiers = soroban_sdk::vec![
        &c.env,
        Tier {
            min_units: 7,
            unit_price: USDC,
        },
    ];
    let pool7 = c
        .gb
        .create_pool(&c.organizer, &terms7, &h(&c.env, 8), &0, &None, &dl);

    let u1 = c.trader();
    let u2 = c.trader();
    let u3 = c.trader();
    let u4 = c.trader();

    c.gb.commit(&u1, &pool7, &3);
    c.gb.commit(&u2, &pool7, &1);
    c.gb.commit(&u3, &pool7, &1);
    c.gb.commit(&u4, &pool7, &2);

    assert_eq!(c.gb.pool(&pool7).state, PoolState::Filled);
    c.deliver(pool7, 7);
    assert_eq!(c.gb.pool(&pool7).state, PoolState::Delivered);

    // 60% of 7 is 4.2 units.
    // 4 confirmed units (3 + 1 = 4, 40_000 < 42_000) is one unit below rounded threshold (5 units)
    c.gb.confirm_pickup(&u1, &pool7);
    c.gb.confirm_pickup(&u2, &pool7);
    let err4 = c.gb.try_settle(&pool7).err();
    assert_eq!(
        err4,
        Some(Ok(soroban_sdk::Error::from_contract_error(
            group_buy::Error::SettleTooEarly as u32
        )))
    );

    // 5 confirmed units (3 + 1 + 1 = 5, 50_000 >= 42_000) reaches the rounded-up 60% threshold
    c.gb.confirm_pickup(&u3, &pool7);
    assert!(c.gb.try_settle(&pool7).is_ok());
    assert_eq!(c.gb.pool(&pool7).state, PoolState::Settled);
}

