#![no_std]

mod extensions;
mod types;

use extensions::env_extensions::EnvExtensions;
use soroban_sdk::{
    contract, contractimpl, panic_with_error, symbol_short, token::TokenClient, Address, BytesN, Env, Symbol, Vec,
};
use types::{
    contract_config::ContractConfig, error::Error, subscription::Subscription,
    subscription_init_params::SubscriptionInitParams, subscription_status::SubscriptionStatus,
};

const REFLECTOR: Symbol = symbol_short!("reflector");

// 1 day in milliseconds
const DAY: u64 = 86400 * 1000;

// Maximum allowed encrypted webhook size, in bytes
const MAX_WEBHOOK_SIZE: u32 = 2048;

// Minimum heartbeat in minutes
const MIN_HEARTBEAT: u32 = 5;

#[contract]
pub struct SubscriptionContract;

#[contractimpl]
impl SubscriptionContract {
    // Admin only

    // Initialize the newly created contract
    // Can be invoked only once
    //
    // # Arguments
    //
    // * `config` - Contract configuration
    //
    // # Panics
    //
    // Panics if the contract is already initialized
    pub fn config(e: Env, config: ContractConfig) {
        config.admin.require_auth();
        if e.is_initialized() {
            e.panic_with_error(Error::AlreadyInitialized);
        }

        e.set_admin(&config.admin);
        e.set_fee(config.fee);
        e.set_token(&config.token);
        e.set_last_subscription_id(0);
    }

    // Update base Reflector subscriptions fee
    // Can be invoked only by the admin account
    //
    // # Arguments
    //
    // * `fee` - New base fee
    //
    // # Panics
    //
    // Panics if the caller doesn't match admin address
    pub fn set_fee(e: Env, fee: u64) {
        e.panic_if_not_admin();
        e.set_fee(fee);
    }

    // Publish subscription trigger event
    // Can be invoked only by the admin account
    //
    // # Arguments
    //
    // * `timestamp` - Timestamp of the trigger
    // * `trigger_hash` - Hash of the trigger data
    //
    // # Panics
    //
    // Panics if the caller doesn't match admin address
    pub fn trigger(e: Env, timestamp: u64, trigger_hash: BytesN<32>) {
        e.panic_if_not_admin();
        // Publish triggered event with root hash of all generated notifications
        e.events().publish(
            (REFLECTOR, symbol_short!("triggered")),
            (timestamp, trigger_hash),
        );
    }

    // Charge retention fees from the subscription balances
    // Can be invoked only by the admin account
    //
    // # Arguments
    //
    // * `subscription_ids` - List of subscription IDs to process
    //
    // # Panics
    //
    // Panics if the caller doesn't match admin address
    pub fn charge(e: Env, subscription_ids: Vec<u64>) {
        e.panic_if_not_admin();
        let mut total_charge: u64 = 0;
        let now = now(&e);
        for subscription_id in subscription_ids.iter() {
            if let Some(mut subscription) = e.get_subscription(subscription_id) {
                // We can charge fees for several days in case if there was an interruption in background worker charge process
                let days_charged = (now - subscription.updated) / DAY;
                if days_charged == 0 {
                    continue;
                }
                let fee = calc_fee(&e, &subscription.heartbeat, &subscription.threshold);
                let mut charge = days_charged * fee;
                // Do not charge more than left on the subscription balance
                if subscription.balance < charge {
                    charge = subscription.balance;
                }
                // Deduct calculated retention fees
                subscription.balance -= charge;
                subscription.updated = now;
                // Publish charged event
                e.events().publish(
                    (
                        REFLECTOR,
                        symbol_short!("charged"),
                        subscription.owner.clone(),
                    ),
                    (subscription_id, charge, now),
                );
                // Deactivate the subscription if the balance is less than the daily retention fee
                if subscription.balance < fee {
                    subscription.status = SubscriptionStatus::Suspended;
                    // Publish suspended event
                    e.events().publish(
                        (
                            REFLECTOR,
                            symbol_short!("suspended"),
                            subscription.owner.clone(),
                        ),
                        (subscription_id, now),
                    );
                }
                // Update subscription properties
                e.set_subscription(subscription_id, &subscription);
                // Sum all retention fee charges
                total_charge += charge;
            }
        }
        // Burn tokens charged from all subscriptions
        if total_charge > 0 {
            get_token_client(&e).burn(&e.current_contract_address(), &(total_charge as i128));
        }
    }

    // Update the contract source code
    // Can be invoked only by the admin account
    //
    // # Arguments
    //
    // * `admin` - Admin account address
    // * `wasm_hash` - WASM hash of the contract source code
    //
    // # Panics
    //
    // Panics if the caller doesn't match admin address
    pub fn update_contract(e: Env, wasm_hash: BytesN<32>) {
        e.panic_if_not_admin();
        e.deployer().update_current_contract_wasm(wasm_hash)
    }

    // Public

    // Create new Reflector subscription with given parameters
    //
    // # Arguments
    //
    // * `new_subscription` - Initialization parameters
    // * `amount` - Initial deposit amount
    //
    // # Returns
    //
    // Subscription ID
    //
    // # Panics
    //
    // Panics if the contract is not initialized
    // Panics if the amount is less than the base fee
    // Panics if the caller doesn't match the owner address
    // Panics if the subscription is invalid
    // Panics if the token transfer fails
    pub fn create_subscription(
        e: Env,
        new_subscription: SubscriptionInitParams,
        amount: u64,
    ) -> (u64, Subscription) {
        panic_if_not_initialized(&e);
        // Check the authorization
        new_subscription.owner.require_auth();
        // Calculate daily retention fee based on subscription params
        let retention_fee = calc_fee(&e, &new_subscription.heartbeat, &new_subscription.threshold);
        // Creation fee is 2 times the daily retention fee
        let init_fee = retention_fee * 2;
        // Check the amount
        if amount < init_fee {
            e.panic_with_error(Error::InvalidAmount);
        }
        // Check subscription heartbeat
        if MIN_HEARTBEAT > new_subscription.heartbeat {
            e.panic_with_error(Error::InvalidHeartbeat);
        }
        // Check threshold
        if new_subscription.threshold == 0 || new_subscription.threshold > 10000 {
            e.panic_with_error(Error::InvalidThreshold);
        }
        // Check subscription webhook size
        if new_subscription.webhook.len() > MAX_WEBHOOK_SIZE {
            e.panic_with_error(Error::WebhookTooLong);
        }
        // Transfer and burn the tokens
        deposit(&e, &new_subscription.owner, amount);
        burn(&e, init_fee, amount);
        // Create subscription itself
        let subscription_id = e.get_last_subscription_id() + 1;
        let subscription = Subscription {
            owner: new_subscription.owner,
            base: new_subscription.base,
            quote: new_subscription.quote,
            threshold: new_subscription.threshold,
            heartbeat: new_subscription.heartbeat,
            webhook: new_subscription.webhook,
            balance: amount - init_fee,
            status: SubscriptionStatus::Active,
            updated: now(&e), // normalize to milliseconds
        };
        // Store
        e.set_subscription(subscription_id, &subscription);
        e.set_last_subscription_id(subscription_id);
        // Extend TTL based on the subscription retention fee and balance
        e.extend_subscription_ttl(subscription_id, calc_ledgers_to_live(&e, &retention_fee, &subscription.balance));
        // Publish subscription created event
        let data = (subscription_id, subscription.clone());
        e.events()
            .publish((REFLECTOR, symbol_short!("created"), subscription.owner), data.clone());
        return data;
    }

    // Deposit Reflector tokens to subscription balance
    //
    // # Arguments
    //
    // * `from` - Account to transfer tokens from
    // * `subscription_id` -  Subscription ID to top up
    // * `amount` - Amount of tokens to deposit
    //
    // # Panics
    //
    // Panics if the contract is not initialized
    // Panics if the amount is zero
    // Panics if the subscription does not exist
    // Panics if the token transfer fails
    pub fn deposit(e: Env, from: Address, subscription_id: u64, amount: u64) {
        panic_if_not_initialized(&e);
        from.require_auth();
        // Check deposit amount
        if amount == 0 {
            e.panic_with_error(Error::InvalidAmount);
        }
        // Load subscription
        let mut subscription = e
            .get_subscription(subscription_id)
            .unwrap_or_else(|| panic_with_error!(e, Error::SubscriptionNotFound));
        // Calculate daily retention fee based on subscription params
        let retention_fee = calc_fee(&e, &subscription.heartbeat, &subscription.threshold);
        // Transfer tokens
        deposit(&e, &from, amount);
        // Update subscription balance
        subscription.balance += amount;
        // Update subscription status if it was suspended
        match subscription.status {
            SubscriptionStatus::Suspended => {
                // Burn tokens as a revival fee
                burn(&e, retention_fee, amount);
                subscription.balance -= retention_fee;
                // Re-activate saubscription
                subscription.status = SubscriptionStatus::Active;
            }
            _ => {}
        }
        // Update state
        e.set_subscription(subscription_id, &subscription);
        // Extend TTL based on the subscription retention fee and balance
        e.extend_subscription_ttl(subscription_id, calc_ledgers_to_live(&e, &retention_fee, &subscription.balance));
        // Publish subscription deposited event
        e.events().publish(
            (REFLECTOR, symbol_short!("deposited"), subscription.owner.clone()),
            (subscription_id, subscription, amount),
        );
    }

    // Cancel active subscription and reimburse the balance to subscription owner account
    //
    // # Arguments
    //
    // * `subscription_id` - Subscription ID
    //
    // # Panics
    //
    // Panics if the contract is not initialized
    // Panics if the subscription does not exist
    // Panics if the caller doesn't match the owner address
    // Panics if the subscription is not active
    // Panics if the token transfer fails
    pub fn cancel(e: Env, subscription_id: u64) {
        panic_if_not_initialized(&e);
        // Load subscription
        let subscription = e
            .get_subscription(subscription_id)
            .unwrap_or_else(|| panic_with_error!(e, Error::SubscriptionNotFound));
        // Only owner can cancel the subscription
        subscription.owner.require_auth();
        match subscription.status {
            SubscriptionStatus::Active => {}
            _ => { // Panic if the subscription is not active at the moment
                e.panic_with_error(Error::InvalidSubscriptionStatusError);
            }
        }
        // Transfer the remaining balance to the owner account
        withdraw(&e, &subscription.owner, subscription.balance);
        // Remove subscription from the state
        e.remove_subscription(subscription_id);
        // Publish subscription cancelled event
        e.events()
            .publish((REFLECTOR, symbol_short!("cancelled"), subscription.owner), subscription_id);
    }

    // Get subscription by ID
    //
    // # Arguments
    //
    // * `subscription_id` - Unique subscription ID
    //
    // # Returns
    //
    // Subscription data
    //
    // # Panics
    //
    // Panics if the contract is not initialized
    // Panics if the subscription is not found
    pub fn get_subscription(e: Env, subscription_id: u64) -> Subscription {
        panic_if_not_initialized(&e);
        // Load subscription
        e.get_subscription(subscription_id)
            .unwrap_or_else(|| panic_with_error!(e, Error::SubscriptionNotFound))
    }

    // Calculate daily retention fee for a given subscription
    //
    // # Arguments
    //
    // * `subscription_id` - Subscription ID
    //
    // # Returns
    //
    // Daily retention fees
    //
    // # Panics
    //
    // Panics if the contract is not initialized
    // Panics if the subscription is not found
    pub fn get_retention_fee(e: Env, subscription_id: u64) -> u64 {
        panic_if_not_initialized(&e);
        // Load subscription
        let subscription = e.get_subscription(subscription_id)
            .unwrap_or_else(|| panic_with_error!(e, Error::SubscriptionNotFound));
        // Calculate daily retention fee based on subscription params
        calc_fee(&e, &subscription.heartbeat, &subscription.threshold)
    }

    // Get the last subscription ID
    //
    // # Returns
    //
    // Last subscription ID
    //
    // # Panics
    //
    // Panics if the contract is not initialized
    pub fn last_id(e: Env) -> u64 {
        panic_if_not_initialized(&e);
        // Retrieve the last value from the subscription ID counter
        e.get_last_subscription_id()
    }

    // Get contract admin address
    //
    // # Returns
    //
    // Contract admin account address
    pub fn admin(e: Env) -> Option<Address> {
        e.get_admin()
    }

    // Get contract version
    //
    // # Returns
    //
    // Contract protocol version
    pub fn version(_e: Env) -> u32 {
        // Retrieve protocol version based on the cargo package info
        env!("CARGO_PKG_VERSION")
            .split(".")
            .next()
            .unwrap()
            .parse::<u32>()
            .unwrap()
    }

    // Get base contract fee (used to calculate amounts charged from the account balance on the daily basis)
    //
    // # Returns
    //
    // Base fee
    //
    // # Panics
    //
    // Panics if the contract is not initialized
    pub fn fee(e: Env) -> u64 {
        panic_if_not_initialized(&e);
        // Retrieve base Reflector subscription fee
        e.get_fee()
    }

    // Retrieve Reflector token contract address
    //
    // # Returns
    //
    // Token address
    //
    // # Panics
    //
    // Panics if the contract is not initialized
    pub fn token(e: Env) -> Address {
        panic_if_not_initialized(&e);
        // Retrieve Reflector token contract address
        e.get_token()
    }
}

// Check that contract has been properly initialized already
fn panic_if_not_initialized(e: &Env) {
    if !e.is_initialized() {
        panic_with_error!(e, Error::NotInitialized);
    }
}

// Initialize a client for Reflector token contract
fn get_token_client(e: &Env) -> TokenClient {
    TokenClient::new(e, &e.get_token())
}

// Transfer tokens to the contract balance
fn deposit(e: &Env, from: &Address, amount: u64) {
    get_token_client(e).transfer(from, &e.current_contract_address(), &(amount as i128));
}

// Burn used tokens
fn burn(e: &Env, burn_amount: u64, max_burn: u64) {
    if burn_amount > max_burn {
        panic_with_error!(e, Error::InvalidAmount);
    }
    get_token_client(e).burn(&e.current_contract_address(), &(burn_amount as i128));
}

// Withdraw tokens from contract balance
fn withdraw(e: &Env, to: &Address, amount: u64) {
    get_token_client(e).transfer(&e.current_contract_address(), to, &(amount as i128));
}

// Get timestamp as milliseconds
fn now(e: &Env) -> u64 {
    e.ledger().timestamp() * 1000
}

fn calc_fee(e: &Env, heartbeat: &u32, threshold: &u32) -> u64 {
    //TODO: implement the fee calculation logic here
    e.get_fee()
}

// Calculate number of ledgers to live for subscription based on retention fee
fn calc_ledgers_to_live(e: &Env, fee: &u64, amount: &u64) -> u32 {
    let days: u32 = ((amount + fee - 1) / fee) as u32;
    let ledgers = days * 17280;
    if ledgers > e.storage().max_ttl() {
        panic_with_error!(e, Error::InvalidAmount);
    }
    ledgers
}

mod test;
