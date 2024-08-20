use soroban_sdk::contracttype;


#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq, Copy)]
pub enum SubscriptionStatus {
    // Subscription tracks price feeds and triggers notifications
    Active = 0,
    // Subscription won't receive updates nor trigger notifications
    Suspended = 1
}