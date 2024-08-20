use soroban_sdk::{contracttype, Address};

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]

// Initial contract configuration parameters
pub struct ContractConfig {
    // Contract admin address
    pub admin: Address,
    // Retention fee token address
    pub token: Address,
    // Base contract fee amount
    pub fee: u64
}
