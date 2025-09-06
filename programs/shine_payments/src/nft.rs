use anchor_lang::prelude::*;
use anchor_lang::solana_program::{program::invoke, program::invoke_signed};
use anchor_spl::{associated_token::AssociatedToken, token::Token};

use mpl_token_metadata::{
    ID as TM_ID,
    instructions::{
        CreateMasterEditionV3Builder,
        CreateMetadataAccountV3Builder,
        SetAndVerifySizedCollectionItemBuilder,
    },
    types::{Collection, Creator, DataV2, Uses, UseMethod},
};

use spl_token::instruction as spl_ix;

/// Параметры для минта NFT
#[derive(Clone)]
pub struct CreateNftParams {
    pub name: String,
    pub symbol: String,
    pub uri: String,
    pub index: u64,
    pub recipient: Pubkey,
}

/// Создание metadata, чеканка 1 токена, freeze ATA, создание master edition, verify в коллекции.
pub fn create_nft_with_freeze(
    ctx: &Context<crate::AddBonusCtx>,
    params: CreateNftParams,
) -> Result<()> {
    let a = &ctx.accounts;

    // Проверяем что это именно программа Metaplex Token Metadata
    require_keys_eq!(a.token_metadata_program.key(), TM_ID, CustomError::InvalidMetadataProgram);

    // 1) Создание Metadata для нового NFT
    let creators = Some(vec![Creator {
        address: a.collection_update_authority.key(),
        verified: true,
        share: 100,
    }]);

    let data = DataV2 {
        name: truncate(&params.name, 32),
        symbol: truncate(&params.symbol, 10),
        uri: truncate(&params.uri, 256),
        seller_fee_basis_points: 0,
        creators,
        collection: Some(Collection {
            verified: false, // отметим как часть коллекции позже через verify
            key: a.collection_mint.key(),
        }),
        uses: Some(Uses {
            use_method: UseMethod::Burn,
            remaining: 1,
            total: 1,
        }),
    };

    // В mpl-token-metadata v5 update_authority(pubkey, is_signer: bool)
    let ix_md = CreateMetadataAccountV3Builder::new()
        .metadata(a.metadata_pda.key())
        .mint(a.mint_pda.key())
        .mint_authority(a.signer.key())
        .payer(a.signer.key())
        .update_authority(a.collection_update_authority.key(), true)
        .system_program(a.system_program.key())
        .data(data)
        .is_mutable(true)
        .instruction();

    invoke_signed(
        &ix_md,
        &[
            a.metadata_pda.to_account_info(),
            a.mint_pda.to_account_info(),
            a.signer.to_account_info(),
            a.collection_update_authority.to_account_info(),
            a.system_program.to_account_info(),
            a.token_metadata_program.to_account_info(),
        ],
        &[],
    )?;

    // 2) Чеканим 1 токен на ATA получателя
    let ix_mint_to = spl_ix::mint_to(
        &a.token_program.key(),
        &a.mint_pda.key(),
        &a.recipient_ata.key(),
        &a.signer.key(),
        &[],
        1,
    )?;
    invoke(
        &ix_mint_to,
        &[
            a.mint_pda.to_account_info(),
            a.recipient_ata.to_account_info(),
            a.signer.to_account_info(),
            a.token_program.to_account_info(),
        ],
    )?;

    // 3) Замораживаем ATA получателя (freeze authority = signer)
    let ix_freeze = spl_ix::freeze_account(
        &a.token_program.key(),
        &a.recipient_ata.key(),
        &a.mint_pda.key(),
        &a.signer.key(),
        &[],
    )?;
    invoke(
        &ix_freeze,
        &[
            a.recipient_ata.to_account_info(),
            a.mint_pda.to_account_info(),
            a.signer.to_account_info(),
            a.token_program.to_account_info(),
        ],
    )?;

    // 4) Создаём Master Edition
    let ix_me = CreateMasterEditionV3Builder::new()
        .edition(a.master_edition_pda.key())
        .mint(a.mint_pda.key())
        .update_authority(a.collection_update_authority.key())
        .mint_authority(a.signer.key())
        .payer(a.signer.key())
        .metadata(a.metadata_pda.key())
        .token_program(a.token_program.key())
        .system_program(a.system_program.key())
        .max_supply(0)
        .instruction();

    invoke_signed(
        &ix_me,
        &[
            a.master_edition_pda.to_account_info(),
            a.mint_pda.to_account_info(),
            a.collection_update_authority.to_account_info(),
            a.signer.to_account_info(),
            a.metadata_pda.to_account_info(),
            a.token_program.to_account_info(),
            a.system_program.to_account_info(),
            a.token_metadata_program.to_account_info(),
        ],
        &[],
    )?;

    // 5) Verify как часть коллекции
    // Метод называется collection_master_edition_account(...)
    let ix_verify = SetAndVerifySizedCollectionItemBuilder::new()
        .metadata(a.metadata_pda.key())
        .collection_authority(a.collection_update_authority.key())
        .payer(a.signer.key())
        .update_authority(a.collection_update_authority.key())
        .collection_mint(a.collection_mint.key())
        .collection(a.collection_metadata_pda.key())
        .collection_master_edition_account(a.collection_master_edition_pda.key())
        .instruction();

    invoke_signed(
        &ix_verify,
        &[
            a.metadata_pda.to_account_info(),
            a.collection_update_authority.to_account_info(),
            a.signer.to_account_info(),
            a.collection_update_authority.to_account_info(),
            a.collection_mint.to_account_info(),
            a.collection_metadata_pda.to_account_info(),
            a.collection_master_edition_pda.to_account_info(),
            a.token_metadata_program.to_account_info(),
        ],
        &[],
    )?;

    msg!("NFT создан, заморожен, мастер-издание создано и верифицировано в коллекции (index={})", params.index);
    Ok(())
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max { s.to_string() } else { s.chars().take(max).collect() }
}

#[error_code]
pub enum CustomError {
    #[msg("Invalid Token Metadata program account")]
    InvalidMetadataProgram,
}
