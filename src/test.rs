#![cfg(test)]

use super::*;
use soroban_sdk::{
    symbol_short,
    testutils::{storage::Persistent, Address as _, Ledger, LedgerInfo},
    token::StellarAssetClient,
    vec, Bytes, Env, String,
};
use types::{
    asset::Asset, contract_config::ContractConfig,
    subscription_init_params::SubscriptionInitParams, ticker_asset::TickerAsset,
};

fn init_contract_with_admin<'a>() -> (Env, SubscriptionContractClient<'a>, ContractConfig) {
    let env = Env::default();

    let admin = Address::generate(&env);

    let contract_id = env.register_contract(None, SubscriptionContract);
    let client: SubscriptionContractClient<'a> =
        SubscriptionContractClient::new(&env, &contract_id);

    let token = env.register_stellar_asset_contract(admin.clone());

    let init_data = ContractConfig {
        admin: admin.clone(),
        token,
        fee: 100000000,
    };

    env.mock_all_auths();

    //set admin
    client.config(&init_data);

    (env, client, init_data)
}

#[test]
fn test() {
    let (env, client, config) = init_contract_with_admin();

    let owner = Address::generate(&env);

    let token_client = StellarAssetClient::new(&env, &config.token);
    token_client.mint(&owner, &(config.fee * 1000).into());

    let subscription = SubscriptionInitParams {
        owner: owner.clone(),
        base: TickerAsset {
            asset: Asset::Other(symbol_short!("BTC")),
            source: String::from_str(&env, "source1"),
        },
        quote: TickerAsset {
            asset: Asset::Other(symbol_short!("ETH")),
            source: String::from_str(&env, "source2"),
        },
        threshold: 10,
        heartbeat: 5,
        webhook: Bytes::from_array(&env, &[0; 2048]),
    };

    let fee = calc_fee(config.fee, &subscription.base, &subscription.quote, subscription.heartbeat);

    // create subscription
    let (subscription_id, _) = client.create_subscription(&subscription, &(fee * 2));
    assert!(subscription_id == 1);

    env.as_contract(&client.address, || {
        let ttl = env.storage().persistent().get_ttl(&subscription_id);
        assert_eq!(ttl, 17280); //one day
    });

    let trigger_hash: BytesN<32> = BytesN::from_array(&env, &[0; 32]);
    // heartbeat subscription
    client.trigger(&1u64, &trigger_hash);

    // deposit subscription
    client.deposit(&owner, &1, &fee);

    env.as_contract(&client.address, || {
        let ttl = env.storage().persistent().get_ttl(&subscription_id);
        assert_eq!(ttl, 17280);
    });

    let mut subs = client.get_subscription(&subscription_id);
    assert_eq!(subs.balance, fee);

    let ledger_info = env.ledger().get();
    env.ledger().set(LedgerInfo {
        timestamp: 86400 * 2,
        ..ledger_info
    });

    // charge subscription
    client.charge(&vec![&env, 1u64]);

    // check balance and status
    subs = client.get_subscription(&subscription_id);
    assert_eq!(subs.balance, 0);
    assert_eq!(subs.status, SubscriptionStatus::Suspended);
    assert_eq!(subs.updated, 86400 * 2 * 1000);

    // deposit subscription to renew
    client.deposit(&owner, &1, &(fee * 2));
    subs = client.get_subscription(&subscription_id);
    assert_eq!(subs.balance, fee); // deposit amount - activation fee
    assert_eq!(subs.status, SubscriptionStatus::Active);

    // cancel subscription
    client.cancel(&1u64);
    env.as_contract(&client.address, || {
        let subs = env.get_subscription(subscription_id);
        assert_eq!(subs, None);
    });

    let last_id = client.last_id();
    assert_eq!(last_id, 1);
}

#[test]
fn fee_test() {
    let env = Env::default();
    let source1_asset = TickerAsset {
        asset: Asset::Other(symbol_short!("BTC")),
        source: String::from_str(&env, "source1"),
    };

    let source2_asset = TickerAsset {
        asset: Asset::Other(symbol_short!("ETH")),
        source: String::from_str(&env, "source2"),
    };

    let test_cases = [
        (100000000, &source1_asset, &source2_asset, 5, 979795896), // Cross-price, high heartbeat factor
        (100000000, &source1_asset, &source1_asset, 5, 489897948), // Same source, high heartbeat factor
        (100000000, &source1_asset, &source1_asset, 120, 100000000), // Reference heartbeat
        (100000000, &source1_asset, &source1_asset, 1000, 100000000), // Large heartbeat, min fee applied
        (10000000000, &source1_asset, &source1_asset, 1000, 10000000000), // Large base fee, large heartbeat, min fee applied
        (500000000, &source1_asset, &source1_asset, 10, 1732050807), // Large base fee, small heartbeat
        (500000000, &source1_asset, &source2_asset, 10, 3464101614), // Large base fee, small heartbeat, cross-price
        (100000000, &source1_asset, &source1_asset, u32::MAX, 100000000), // Maximum heartbeat, minimal fee
         (100000000 * 1000000, &source1_asset, &source2_asset, 5, 979795897113270), // Huge base fee, small heartbeat, cross-price
    ];

    for (i, &(base_fee, base, quote, heartbeat, expected_fee)) in test_cases.iter().enumerate() {
        let fee = calc_fee(base_fee, base, quote, heartbeat);
        assert_eq!(
            fee, expected_fee,
            "Test case {} failed. Expected: {}, Got: {}",
            i, expected_fee, fee
        );
    }
}
