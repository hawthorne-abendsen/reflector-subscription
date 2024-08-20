use soroban_sdk::{contracttype, Address, Bytes};

use super::ticker_asset::TickerAsset;

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]

// New subscription configuration params
pub struct SubscriptionInitParams {
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
    // Encrypted webhook URL where trigger notifications get POSTed
    pub webhook: Bytes,
}
