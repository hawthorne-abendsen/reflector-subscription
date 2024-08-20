use soroban_sdk::{contracttype, Address, Bytes};

use super::{subscription_status::SubscriptionStatus, ticker_asset::TickerAsset};

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]

// Subscription record properties
pub struct Subscription {
    // Address of account that owns this subscription
    pub owner: Address,
    // Base symbol
    pub base: TickerAsset,
    // Quote symbol
    pub quote: TickerAsset,
    // Price movement threshold that triggers subscription, in ‰
    pub threshold: u32,
    // Interval of periodic invocations, in minutes
    pub heartbeat: u32,
    // The webhook.
    pub webhook: Bytes,
    // Current outstanding subscription balance
    pub balance: u64,
    // Current status
    pub status: SubscriptionStatus,
    // Last updated timestamp
    pub updated: u64
}
