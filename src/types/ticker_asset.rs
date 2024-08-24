use soroban_sdk::{contracttype, String};
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]

// Ticker symbol descriptor
pub struct TickerAsset {
    // Asset identifier
    pub asset: String,
    // Price feed source
    pub source: String
}