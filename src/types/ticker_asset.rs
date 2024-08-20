use soroban_sdk::{contracttype, String};

use super::asset::Asset;

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]

// Ticker symbol descriptor
pub struct TickerAsset {
    // Asset identifier
    pub asset: Asset,
    // Price feed source
    pub source: String
}