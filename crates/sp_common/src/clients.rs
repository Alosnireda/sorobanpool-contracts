//! Cross-contract interfaces. Callers use the generated `*Client` types, so
//! contract crates never depend on one another.
use crate::types::{Commitment, Params, Pool, RepStats, Role, Status};
use soroban_sdk::{contractclient, Address, BytesN, Env, Symbol};

#[contractclient(name = "ConfigClient")]
pub trait ConfigIface {
    fn admin(env: Env) -> Address;
    fn usdc(env: Env) -> Address;
    fn treasury(env: Env) -> Address;
    fn get_params(env: Env) -> Params;
    fn get_address(env: Env, key: Symbol) -> Address;
    fn is_attestor(env: Env, a: Address) -> bool;
    fn is_arbiter(env: Env, a: Address) -> bool;
    fn is_category_allowed(env: Env, cat: Symbol) -> bool;
    fn is_paused(env: Env, scope: Symbol) -> bool;
}

#[contractclient(name = "RegistryClient")]
pub trait RegistryIface {
    fn status(env: Env, user: Address, role: Role) -> Status;
    fn level(env: Env, user: Address, role: Role) -> u32;
    fn cluster_of(env: Env, user: Address) -> Option<Symbol>;
    fn suspend(env: Env, caller: Address, user: Address, role: Role, reason: Symbol);
}

#[contractclient(name = "ReputationClient")]
pub trait ReputationIface {
    fn record(env: Env, caller: Address, subject: Address, role: Role, event: Symbol);
    fn stats(env: Env, subject: Address, role: Role) -> RepStats;
    fn tier(env: Env, subject: Address, role: Role) -> u32;
}

#[contractclient(name = "BondClient")]
pub trait BondIface {
    fn reserve_for_advance(env: Env, caller: Address, supplier: Address, amount: i128);
    fn release_advance(env: Env, caller: Address, supplier: Address, amount: i128);
    fn slash(env: Env, caller: Address, supplier: Address, amount: i128) -> i128;
    fn bond_of(env: Env, supplier: Address) -> (i128, i128);
}

#[contractclient(name = "GroupBuyClient")]
pub trait GroupBuyIface {
    fn pool(env: Env, id: u64) -> Pool;
    fn commitment(env: Env, pool_id: u64, member: Address) -> Option<Commitment>;
    fn allocated_units_of(env: Env, pool_id: u64, member: Address) -> u32;
    fn confirm_deadline(env: Env, pool_id: u64) -> u64;
    fn freeze(env: Env, caller: Address, pool_id: u64, amount: i128);
    fn apply_outcome(
        env: Env,
        caller: Address,
        pool_id: u64,
        member: Option<Address>,
        to_member: i128,
        to_supplier: i128,
    );
}

#[allow(dead_code)]
type _Hash = BytesN<32>;

#[cfg(test)]
mod test {
    use super::*;
    use soroban_sdk::{
        contract, contracterror, contractimpl, panic_with_error, symbol_short,
        testutils::Address as _, Env,
    };

    #[contracterror]
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    #[repr(u32)]
    pub enum MockError {
        NotAuthorizedCaller = 900,
    }

    /// Minimal stand-in for `config`: only `get_address` is needed by the guard.
    #[contract]
    pub struct MockConfig;

    #[contractimpl]
    impl MockConfig {
        pub fn __constructor(env: Env, registered: Address) {
            env.storage()
                .instance()
                .set(&symbol_short!("caller"), &registered);
        }

        pub fn get_address(env: Env, _key: Symbol) -> Address {
            env.storage()
                .instance()
                .get(&symbol_short!("caller"))
                .unwrap()
        }
    }

    /// Mirrors the privileged-caller pattern used by the contracts: authorise
    /// the caller, then check it against `config.get_address`.
    #[contract]
    pub struct Guarded;

    #[contractimpl]
    impl Guarded {
        pub fn ping(env: Env, caller: Address, cfg: Address, key: Symbol) -> Address {
            caller.require_auth();
            let registered = ConfigClient::new(&env, &cfg).get_address(&key);
            if caller != registered {
                panic_with_error!(&env, MockError::NotAuthorizedCaller);
            }
            registered
        }
    }

    fn setup(env: &Env) -> (Address, Address) {
        let registered = Address::generate(env);
        let cfg = env.register(MockConfig, (&registered,));
        let guarded = env.register(Guarded, ());
        (cfg, guarded)
    }

    #[test]
    fn unregistered_caller_is_rejected() {
        let env = Env::default();
        env.mock_all_auths();
        let (cfg, guarded) = setup(&env);
        let stranger = Address::generate(&env);
        let guard = GuardedClient::new(&env, &guarded);
        assert!(guard
            .try_ping(&stranger, &cfg, &symbol_short!("group_buy"))
            .is_err());
    }

    #[test]
    fn registered_caller_without_auth_is_rejected() {
        let env = Env::default();
        env.mock_auths(&[]);
        let (cfg, guarded) = setup(&env);
        let registered = ConfigClient::new(&env, &cfg).get_address(&symbol_short!("group_buy"));
        let guard = GuardedClient::new(&env, &guarded);
        assert!(guard
            .try_ping(&registered, &cfg, &symbol_short!("group_buy"))
            .is_err());
    }

    #[test]
    fn authorised_caller_succeeds() {
        let env = Env::default();
        env.mock_all_auths();
        let (cfg, guarded) = setup(&env);
        let registered = ConfigClient::new(&env, &cfg).get_address(&symbol_short!("group_buy"));
        let guard = GuardedClient::new(&env, &guarded);
        assert_eq!(
            guard.ping(&registered, &cfg, &symbol_short!("group_buy")),
            registered
        );
    }
}
