use soroban_sdk::contracterror;

#[contracterror]
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
// Contract error codes
pub enum Error {
    // Contract has been already initialized
    AlreadyInitialized = 0,
    // Caller is not authorized to perform this operation
    Unauthorized = 1,
    // Subscription with this ID does not exist
    SubscriptionNotFound = 2,
    // Contract has not been initialized
    NotInitialized = 3,
    // Initial subscription amount is not valid
    InvalidAmount = 4,
    // Heartbeat is not valid
    InvalidHeartbeat = 5,
    // Threshold percentage is not valid
    InvalidThreshold = 6,
    // Subscription webhook URL is too long
    WebhookTooLong = 7,
    // Current subscription status is not valid for the operation
    InvalidSubscriptionStatusError = 8
}
