//! Minimal end-to-end client walkthrough of SorobanPool.
//!
//! Demonstrates the complete group-buying lifecycle:
//! 1. Environment and contract configuration
//! 2. Organizer and supplier register and receive KYC/KYB attestations
//! 3. Organizer creates a pool with multi-unit price tiers
//! 4. Members commit USDC into escrow
//! 5. Pool reaches MOQ and locks final price tier
//! 6. Supplier accepts and dispatches the order
//! 7. Delivery is confirmed
//! 8. Pool settles: supplier is paid, platform/organizer fees deducted, and member refunds distributed.

use config::{Config, ConfigClient};
use disputes::Disputes;
use group_buy::{GroupBuy, GroupBuyClient};
use registry::{Registry, RegistryClient};
use reputation::Reputation;
use soroban_sdk::testutils::{Address as _, Ledger};
use soroban_sdk::{symbol_short, token, vec, Address, BytesN, Env};
use sp_common::keys;
use sp_common::types::{PoolTerms, Role, Tier};
use supplier_bond::SupplierBond;

const USDC: i128 = 10_000_000; // 1 USDC = 10,000,000 stroops (7 decimals)

fn format_usdc(stroops: i128) -> String {
    format!("{:.2} USDC", (stroops as f64) / 10_000_000.0)
}

fn print_balances(
    label: &str,
    token: &token::Client,
    organizer: &Address,
    supplier: &Address,
    treasury: &Address,
    gb_address: &Address,
    members: &[Address],
) {
    println!("\n--- [Balances: {}] ---", label);
    println!("  Organizer: {}", format_usdc(token.balance(organizer)));
    println!("  Supplier:  {}", format_usdc(token.balance(supplier)));
    println!("  Treasury:  {}", format_usdc(token.balance(treasury)));
    println!("  Escrow:    {}", format_usdc(token.balance(gb_address)));
    for (i, m) in members.iter().enumerate() {
        println!(
            "  Member {}:  {}",
            (b'A' + i as u8) as char,
            format_usdc(token.balance(m))
        );
    }
}

fn main() {
    println!("============================================================");
    println!("  SorobanPool Group-Buy Walkthrough");
    println!("============================================================");

    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(1_000_000);

    // Setup protocol addresses
    let admin = Address::generate(&env);
    let treasury = Address::generate(&env);
    let attestor = Address::generate(&env);

    // Deploy mock USDC asset contract
    let sac = env.register_stellar_asset_contract_v2(admin.clone());
    let token = token::Client::new(&env, &sac.address());
    let mint = token::StellarAssetClient::new(&env, &sac.address());

    // Deploy core contracts
    let cfg_id = env.register(Config, (&admin, sac.address(), &treasury));
    let cfg = ConfigClient::new(&env, &cfg_id);
    let reg_id = env.register(Registry, (&cfg_id,));
    let reg = RegistryClient::new(&env, &reg_id);
    let rep_id = env.register(Reputation, (&cfg_id,));
    let bond_id = env.register(SupplierBond, (&cfg_id,));
    let gb_id = env.register(GroupBuy, (&cfg_id,));
    let gb = GroupBuyClient::new(&env, &gb_id);
    let dis_id = env.register(Disputes, (&cfg_id,));

    // Register contract addresses in config
    cfg.set_address(&keys::REGISTRY, &reg_id);
    cfg.set_address(&keys::REPUTATION, &rep_id);
    cfg.set_address(&keys::BOND, &bond_id);
    cfg.set_address(&keys::GROUP_BUY, &gb_id);
    cfg.set_address(&keys::DISPUTES, &dis_id);
    cfg.set_category(&symbol_short!("grains"), &true);
    cfg.set_attestor(&attestor, &true);

    // Setup actors
    let organizer = Address::generate(&env);
    let supplier = Address::generate(&env);
    let member_a = Address::generate(&env);
    let member_b = Address::generate(&env);
    let member_c = Address::generate(&env);
    let members = [member_a.clone(), member_b.clone(), member_c.clone()];

    // Verify participants in registry via attestor
    let cluster = Some(symbol_short!("lagos"));
    let h_dummy = BytesN::from_array(&env, &[1u8; 32]);
    reg.register(&organizer, &Role::Organizer, &h_dummy, &cluster);
    reg.attest(&attestor, &organizer, &Role::Organizer, &h_dummy, &1);

    reg.register(&supplier, &Role::Supplier, &h_dummy, &cluster);
    reg.attest(&attestor, &supplier, &Role::Supplier, &h_dummy, &1);

    for m in &members {
        reg.register(m, &Role::Trader, &h_dummy, &cluster);
        reg.attest(&attestor, m, &Role::Trader, &h_dummy, &1);
        mint.mint(m, &(200 * USDC)); // 200 USDC starting balance per member
    }

    print_balances(
        "Initial State",
        &token,
        &organizer,
        &supplier,
        &treasury,
        &gb_id,
        &members,
    );

    // 1. Create a pool
    println!("\n[Step 1] Organizer creates pool for 'grains'");
    println!("  - Tiers: 100 units @ 1.00 USDC, 200 units @ 0.90 USDC, 300 units @ 0.80 USDC");
    println!("  - Organizer fee: 1.00% (100 bp)");
    let tiers = vec![
        &env,
        Tier {
            min_units: 100,
            unit_price: 10_000_000,
        }, // Tier 1: 1.00 USDC
        Tier {
            min_units: 200,
            unit_price: 9_000_000,
        }, // Tier 2: 0.90 USDC
        Tier {
            min_units: 300,
            unit_price: 8_000_000,
        }, // Tier 3: 0.80 USDC
    ];
    let terms = PoolTerms {
        supplier: supplier.clone(),
        offer_hash: BytesN::from_array(&env, &[2u8; 32]),
        unit_label_hash: BytesN::from_array(&env, &[3u8; 32]),
        category: symbol_short!("grains"),
        tiers,
        moq: 100,
        max_units: 500,
        max_per_member: 200,
        lead_time_secs: 48 * 3600,
        perishable: false,
    };
    let pool_id = gb.create_pool(
        &organizer,
        &terms,
        &BytesN::from_array(&env, &[4u8; 32]),
        &100, // 1% organizer fee
        &cluster,
        &(env.ledger().timestamp() + 86400 * 3),
    );
    println!("  Pool created successfully with ID: {}", pool_id);

    // 2. Members commit units
    println!("\n[Step 2] Members commit orders into escrow:");
    println!("  - Member A commits 60 units (pays 60.00 USDC @ Tier 1 max price)");
    gb.commit(&member_a, &pool_id, &60);

    println!("  - Member B commits 80 units (pool total 140, reaches Tier 1)");
    gb.commit(&member_b, &pool_id, &80);

    println!("  - Member C commits 160 units (pool total 300, reaches Tier 3 @ 0.80 USDC)");
    gb.commit(&member_c, &pool_id, &160);

    print_balances(
        "After Commitments",
        &token,
        &organizer,
        &supplier,
        &treasury,
        &gb_id,
        &members,
    );

    // 3. Pool reaches deadline and fills
    println!("\n[Step 3] Pool closes and locks final price tier:");
    env.ledger()
        .set_timestamp(env.ledger().timestamp() + 86400 * 3 + 1); // Pass fill deadline
    gb.close(&pool_id);
    let p = gb.pool(&pool_id);
    println!("  - Total units committed: {}", p.total_units);
    println!("  - Locked final price:    {}", format_usdc(p.final_price));
    println!("  - Total paid into escrow:{}", format_usdc(p.total_paid));

    // 4. Supplier accepts and dispatches
    println!("\n[Step 4] Supplier accepts order and marks dispatched:");
    gb.accept(&supplier, &pool_id, &0); // No advance
    gb.dispatch(&supplier, &pool_id, &None);
    println!("  Order dispatched to Lagos delivery hub.");

    // 5. Organizer confirms delivery
    println!("\n[Step 5] Organizer confirms full delivery (300 units received):");
    gb.confirm_delivery(
        &organizer,
        &pool_id,
        &300,
        &BytesN::from_array(&env, &[5u8; 32]),
    );

    // 6. Settle pool
    println!("\n[Step 6] Pool settles and distributes payouts and refunds:");
    env.ledger()
        .set_timestamp(env.ledger().timestamp() + 48 * 3600); // Pass confirm window
    gb.settle(&pool_id);
    gb.push_refunds(&pool_id, &10); // Push refunds to members
    gb.sweep_dust(&pool_id);

    print_balances(
        "Final Settled State",
        &token,
        &organizer,
        &supplier,
        &treasury,
        &gb_id,
        &members,
    );

    println!("\n============================================================");
    println!("  Walkthrough completed successfully!");
    println!("  - Supplier received net delivery payment.");
    println!("  - Organizer and treasury collected fee shares.");
    println!("  - Members received automatic tier volume savings refunds.");
    println!("  - Escrow contract holds 0 stroops.");
    println!("============================================================");
}
