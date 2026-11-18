use anchor_lang::AccountDeserialize;
use {
    anchor_spl::associated_token,
    litesvm::LiteSVM,
    litesvm_token::CreateMint,
    solana_keypair::Keypair,
    solana_message::{Instruction, Message, VersionedMessage},
    solana_pubkey::Pubkey,
    solana_signer::Signer,
    solana_transaction::versioned::VersionedTransaction,
};

mod ix_handlers;
use ix_handlers::*;

fn send(
    svm: &mut LiteSVM,
    ixs: &[Instruction],
    payer: &Keypair,
    signers: &[&Keypair],
) -> litesvm::types::TransactionResult {
    svm.expire_blockhash();
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(ixs, Some(&payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), signers).unwrap();
    svm.send_transaction(tx)
}

// Setup function to initialize LiteSVM and create a payer keypair
fn setup() -> (
    LiteSVM,
    Keypair,
    Pubkey,
    Pubkey,
    Pubkey,
    Pubkey,
    Pubkey,
    Pubkey,
) {
    let program_id = amm_video::id();
    let payer = Keypair::new();
    let mut svm = LiteSVM::new();
    let bytes = include_bytes!("../../../target/deploy/amm_video.so");
    svm.add_program(program_id, bytes).unwrap();
    svm.airdrop(&payer.pubkey(), 1_000_000_000).unwrap();

    // Create two mints (Mint A and Mint B) with 6 decimal places and the maker as the authority
    // This done using litesvm-token's CreateMint utility which creates the mint in the LiteSVM environment
    let mint_x = CreateMint::new(&mut svm, &payer)
        .decimals(6)
        .authority(&payer.pubkey())
        .send()
        .unwrap();

    let mint_y = CreateMint::new(&mut svm, &payer)
        .decimals(6)
        .authority(&payer.pubkey())
        .send()
        .unwrap();

    let config =
        Pubkey::find_program_address(&[b"config", &123u64.to_le_bytes()], &amm_video::id()).0;
    let mint_lp = Pubkey::find_program_address(&[b"lp", config.as_ref()], &amm_video::id()).0;

    // Derive the PDA for the vault associated token account using the config PDA and Mint A
    let vault_x = associated_token::get_associated_token_address(&config, &mint_x);
    let vault_y = associated_token::get_associated_token_address(&config, &mint_y);

    (
        svm, payer, mint_x, mint_y, config, mint_lp, vault_x, vault_y,
    )
}

#[test]
fn test_initialize() {
    let (mut svm, payer, mint_x, mint_y, config, mint_lp, vault_x, vault_y) = setup();

    let instruction = create_initialise_ix(
        &mut svm, &payer, mint_x, mint_y, config, mint_lp, vault_x, vault_y,
    );
    let res = send(&mut svm, &[instruction], &payer, &[&payer]);
    assert!(res.is_ok());
}

#[test]
pub fn test_deposit() {
    let (mut svm, payer, mint_x, mint_y, config, mint_lp, vault_x, vault_y) = setup();
    let init_ix = create_initialise_ix(
        &mut svm, &payer, mint_x, mint_y, config, mint_lp, vault_x, vault_y,
    );

    let deposit_ix = create_deposit_ix(
        &mut svm, &payer, mint_x, mint_y, mint_lp, config, vault_x, vault_y,
    );

    let res = send(&mut svm, &[init_ix, deposit_ix], &payer, &[&payer]);
    assert!(res.is_ok());
}

#[test]
pub fn test_withdraw() {
    let (mut svm, payer, mint_x, mint_y, config, mint_lp, vault_x, vault_y) = setup();
    let init_ix = create_initialise_ix(
        &mut svm, &payer, mint_x, mint_y, config, mint_lp, vault_x, vault_y,
    );

    let deposit_ix = create_deposit_ix(
        &mut svm, &payer, mint_x, mint_y, mint_lp, config, vault_x, vault_y,
    );

    let withdraw_ix = create_withdraw_ix(
        &mut svm, &payer, mint_x, mint_y, mint_lp, config, vault_x, vault_y,
    );
    let res = send(
        &mut svm,
        &[init_ix, deposit_ix, withdraw_ix],
        &payer,
        &[&payer],
    );
    assert!(res.is_ok());
}

#[test]
pub fn test_swap() {
    let (mut svm, payer, mint_x, mint_y, config, mint_lp, vault_x, vault_y) = setup();
    let init_ix = create_initialise_ix(
        &mut svm, &payer, mint_x, mint_y, config, mint_lp, vault_x, vault_y,
    );

    let deposit_ix = create_deposit_ix(
        &mut svm, &payer, mint_x, mint_y, mint_lp, config, vault_x, vault_y,
    );

    let swap_ix = create_swap_ix(
        &mut svm, &payer, mint_x, mint_y, mint_lp, config, vault_x, vault_y, true,
    );

    let res = send(&mut svm, &[init_ix, deposit_ix, swap_ix], &payer, &[&payer]);
    assert!(res.is_ok());

    let (treasury_x, _) = Pubkey::find_program_address(
        &[b"treasury_x", config.as_ref()],
        &amm_video::id(),
    );

    let account = svm.get_account(&treasury_x).unwrap();

    let treasury = anchor_spl::token::TokenAccount::try_deserialize(
        &mut account.data.as_slice(),
    ).unwrap();

    assert_eq!(treasury.amount, 30_000);

    let account = svm.get_account(&vault_x).unwrap();

    let vault = anchor_spl::token::TokenAccount::try_deserialize(
        &mut account.data.as_slice(),
    ).unwrap();

    assert_eq!(vault.amount, 209_970_000);

    let user_y = associated_token::get_associated_token_address(&payer.pubkey(), &mint_y);

    let account = svm.get_account(&user_y).unwrap();
    let user_tokens = anchor_spl::token::TokenAccount::try_deserialize(
        &mut account.data.as_slice(),
    ).unwrap();

    let account = svm.get_account(&vault_y).unwrap();
    let pool_tokens = anchor_spl::token::TokenAccount::try_deserialize(
        &mut account.data.as_slice(),
    ).unwrap();
    assert!(user_tokens.amount >= 805_000_000);
    assert_eq!(user_tokens.amount + pool_tokens.amount, 1_000_000_000);
}

#[test]
pub fn test_swap_y_to_x() {
    let (mut svm, payer, mint_x, mint_y, config, mint_lp, vault_x, vault_y) = setup();
    let init_ix = create_initialise_ix(
        &mut svm, &payer, mint_x, mint_y, config, mint_lp, vault_x, vault_y,
    );

    let deposit_ix = create_deposit_ix(
        &mut svm, &payer, mint_x, mint_y, mint_lp, config, vault_x, vault_y,
    );

    let swap_ix = create_swap_ix(
        &mut svm, &payer, mint_x, mint_y, mint_lp, config, vault_x, vault_y, false,
    );

    let res = send(&mut svm, &[init_ix, deposit_ix, swap_ix], &payer, &[&payer]);
    assert!(res.is_ok());

    let (treasury_y, _) = Pubkey::find_program_address(
        &[b"treasury_y", config.as_ref()],
        &amm_video::id(),
    );

    let account = svm.get_account(&treasury_y).unwrap();

    let treasury = anchor_spl::token::TokenAccount::try_deserialize(
        &mut account.data.as_slice(),
    ).unwrap();

    assert_eq!(treasury.amount, 30_000);

    
    let account = svm.get_account(&vault_y).unwrap();

    let vault = anchor_spl::token::TokenAccount::try_deserialize(
        &mut account.data.as_slice(),
    ).unwrap();

    assert_eq!(vault.amount, 209_970_000);

    let user_x = associated_token::get_associated_token_address(&payer.pubkey(), &mint_x);

    let account = svm.get_account(&user_x).unwrap();
    let user_tokens = anchor_spl::token::TokenAccount::try_deserialize(
        &mut account.data.as_slice(),
    ).unwrap();

    let account = svm.get_account(&vault_x).unwrap();
    let pool_tokens = anchor_spl::token::TokenAccount::try_deserialize(
        &mut account.data.as_slice(),
    ).unwrap();
    assert!(user_tokens.amount >= 805_000_000);
    assert_eq!(user_tokens.amount + pool_tokens.amount, 1_000_000_000);
}