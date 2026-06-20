//! End-to-end integration tests against Solana devnet.
//!
//! These tests send real transactions and require a funded devnet keypair.
//! They are **ignored by default** and must be run explicitly:
//!
//! ```sh
//! cargo test --test devnet_e2e -- --ignored --nocapture
//! ```
//!
//! Prerequisites:
//! 1. Set `CERBERUS_TEST_KEYPAIR_PATH` (defaults to `~/.config/solana/id.json`).
//! 2. Fund the payer with at least 0.05 SOL:
//!    ```sh
//!    solana airdrop 0.1 --url devnet
//!    ```
//! 3. Optionally set `RPC_URL` to override the devnet endpoint.

use cerberus_skill::{
    lock::get_lock_state,
    recover::recover_partial_setup,
    spending_account::{create_governed_spending_account, SpendingPeriod, SpendingTierConfig},
    verify::assert_fully_locked,
};
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::{
    pubkey::Pubkey,
    signature::{read_keypair_file, Keypair},
    signer::Signer,
    system_instruction, system_program,
    transaction::Transaction,
};
use squads_multisig::{
    client::{spending_limit_use, SpendingLimitUseAccounts},
    pda::get_vault_pda,
    squads_multisig_program as program,
    squads_multisig_program::instructions::SpendingLimitUseArgs,
};

fn devnet_rpc() -> RpcClient {
    let url =
        std::env::var("RPC_URL").unwrap_or_else(|_| "https://api.devnet.solana.com".to_string());
    RpcClient::new(url)
}

fn test_payer() -> Keypair {
    let path = std::env::var("CERBERUS_TEST_KEYPAIR_PATH")
        .unwrap_or_else(|_| shellexpand::tilde("~/.config/solana/id.json").to_string());
    read_keypair_file(&path).expect("funded devnet keypair")
}

/// Full lifecycle: create → verify lock → spend under limit → rejection above limit.
///
/// This test covers the core promise of the entire crate:
/// - Tier 1 (under limit): agent signs alone, transaction succeeds.
/// - Over limit: Squads program rejects on-chain regardless of any off-chain check.
#[tokio::test]
#[ignore = "requires funded devnet keypair and live RPC — run with --ignored"]
async fn full_lifecycle_on_devnet() {
    let rpc = devnet_rpc();
    let payer = test_payer();

    // The agent has a real keypair so we can sign spending_limit_use later.
    let agent = Keypair::new();
    let governing_authority = payer.pubkey();

    // ── 1. Create governed spending account (TX1–TX4) ──────────────────────
    let limit_lamports: u64 = 1_000_000; // 0.001 SOL

    let account = create_governed_spending_account(
        &rpc,
        &payer,
        agent.pubkey(),
        governing_authority,
        SpendingTierConfig {
            max_auto_approve_lamports: limit_lamports,
            period: SpendingPeriod::Day,
            mint: Pubkey::default(), // native SOL
        },
    )
    .await
    .expect("TX1–TX4 should succeed");

    println!("multisig: {}", account.multisig_pda);
    println!("vault:    {}", account.vault_pda);
    println!("limit:    {limit_lamports} lamports");

    // ── 2. Assert fully locked ──────────────────────────────────────────────
    assert_fully_locked(&rpc, &account.multisig_pda, 1)
        .await
        .expect("multisig must be locked after setup");

    let state = get_lock_state(&rpc, &account.multisig_pda)
        .await
        .expect("lock state readable");
    assert_eq!(
        state.config_authority,
        Pubkey::default(),
        "config_authority must be disabled"
    );
    assert_eq!(state.threshold, 1);

    // ── 3. Fund the vault ──────────────────────────────────────────────────
    // The vault PDA needs SOL before the agent can spend from it.
    let fund_amount = 2_000_000u64; // 0.002 SOL — enough for tests below
    let fund_ix = system_instruction::transfer(&payer.pubkey(), &account.vault_pda, fund_amount);
    let bh = rpc.get_latest_blockhash().await.unwrap();
    let fund_tx =
        Transaction::new_signed_with_payer(&[fund_ix], Some(&payer.pubkey()), &[&payer], bh);
    rpc.send_and_confirm_transaction(&fund_tx)
        .await
        .expect("vault funding should succeed");

    // ── 4. Tier 1: spend under limit with agent signature only ──────────────
    //
    // This is the core Cerberus promise: the on-chain SpendingLimit allows the
    // agent to spend up to `limit_lamports` per period without any co-signer.
    // Only the agent's key is needed — governing_authority is NOT involved.
    let spend_amount = limit_lamports / 2; // 500_000 — well under the cap

    let (vault_pda, _) = get_vault_pda(&account.multisig_pda, 0, Some(&program::ID));

    let spend_ix = spending_limit_use(
        SpendingLimitUseAccounts {
            multisig: account.multisig_pda,
            member: agent.pubkey(),
            spending_limit: account.spending_limit_pda,
            vault: vault_pda,
            destination: payer.pubkey(), // send back to payer for simplicity
            system_program: Some(system_program::id()),
            mint: None,
            vault_token_account: None,
            destination_token_account: None,
            token_program: None,
        },
        SpendingLimitUseArgs {
            amount: spend_amount,
            decimals: 9,
            memo: None,
        },
        Some(program::ID),
    );

    let bh = rpc.get_latest_blockhash().await.unwrap();
    let spend_tx = Transaction::new_signed_with_payer(
        &[spend_ix],
        Some(&agent.pubkey()),
        &[&agent], // agent alone — no governing_authority
        bh,
    );
    rpc.send_and_confirm_transaction(&spend_tx)
        .await
        .expect("Tier 1 spend should succeed with agent signature only");

    println!("Tier 1 spend ({spend_amount} lamports) confirmed — agent signed alone");

    // ── 5. Attempt to exceed remaining budget → on-chain rejection ──────────
    //
    // After spending 500_000, the remaining budget is 500_000.
    // Trying to spend 600_000 exceeds it — the Squads program rejects this
    // on-chain. This is NOT a soft limit enforced by application code; the
    // transaction is rejected by the Solana runtime via the Squads program.
    let excess_amount = spend_amount + 100_000; // 600_000 — exceeds remaining 500_000

    let reject_ix = spending_limit_use(
        SpendingLimitUseAccounts {
            multisig: account.multisig_pda,
            member: agent.pubkey(),
            spending_limit: account.spending_limit_pda,
            vault: vault_pda,
            destination: payer.pubkey(),
            system_program: Some(system_program::id()),
            mint: None,
            vault_token_account: None,
            destination_token_account: None,
            token_program: None,
        },
        SpendingLimitUseArgs {
            amount: excess_amount,
            decimals: 9,
            memo: None,
        },
        Some(program::ID),
    );

    let bh = rpc.get_latest_blockhash().await.unwrap();
    let reject_tx =
        Transaction::new_signed_with_payer(&[reject_ix], Some(&agent.pubkey()), &[&agent], bh);
    let result = rpc.send_and_confirm_transaction(&reject_tx).await;

    assert!(
        result.is_err(),
        "spending above remaining budget must be rejected on-chain"
    );
    println!(
        "Tier 1 rejection ({excess_amount} lamports) confirmed — Squads rejected on-chain: {:?}",
        result.unwrap_err()
    );
}

/// Recovery: already-locked multisig is a no-op.
#[tokio::test]
#[ignore = "requires funded devnet keypair and live RPC — run with --ignored"]
async fn recover_already_locked_is_noop() {
    let rpc = devnet_rpc();
    let payer = test_payer();

    // The devnet proof-of-concept multisig from Metera development.
    // This is fully locked; recover_partial_setup should return Ok immediately.
    let multisig_pda: Pubkey = "JCxMQNWf2VbxM9dA2ut2Pu767Et447giMgSqufdegpKr"
        .parse()
        .unwrap();

    recover_partial_setup(&rpc, &payer, &multisig_pda, 1)
        .await
        .expect("recovery on already-locked multisig must be a no-op");
}

/// Validation: zero limit_lamports is rejected before any network call.
#[tokio::test]
async fn config_validation_zero_amount() {
    let rpc = devnet_rpc();
    let payer = Keypair::new();

    let result = create_governed_spending_account(
        &rpc,
        &payer,
        Keypair::new().pubkey(),
        Keypair::new().pubkey(),
        SpendingTierConfig {
            max_auto_approve_lamports: 0, // invalid
            period: SpendingPeriod::Day,
            mint: Pubkey::default(),
        },
    )
    .await;

    assert!(
        matches!(
            result,
            Err(cerberus_skill::CerberusError::InvalidSpendingLimitAmount { .. })
        ),
        "expected InvalidSpendingLimitAmount, got: {result:?}"
    );
}
