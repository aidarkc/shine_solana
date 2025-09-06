const anchor = require("@coral-xyz/anchor");
const {
  Connection,
  PublicKey,
  SystemProgram,
  Transaction,
  TransactionInstruction,
} = require("@solana/web3.js");
const {
  getAssociatedTokenAddress,
  createAssociatedTokenAccountInstruction,
  TOKEN_PROGRAM_ID,
  ASSOCIATED_TOKEN_PROGRAM_ID,
  createMint,
  getAccount,
} = require("@solana/spl-token");
const crypto = require("crypto");

// Адрес программы метаданных Metaplex (фиксированный)
const METAPLEX_TOKEN_METADATA_PROGRAM_ID = new PublicKey(
  "metaqbxxUerdq28cj1RbAWkYQm3ybzjb6a8bt518x1s"
);

// ────────────────────────────────
// Утилиты
// ────────────────────────────────
const BASE58_RE = /[1-9A-HJ-NP-Za-km-z]{32,}/g;

function mustEnv(name) {
  const v = (process.env[name] || "").trim();
  if (!v) throw new Error(`Переменная окружения ${name} не задана`);
  return v;
}

function pickBase58(raw, name) {
  const m = (raw || "").toString().match(BASE58_RE);
  if (!m) throw new Error(`${name} не найден/невалиден: "${raw}"`);
  return m[0];
}

// Anchor discriminator: sha256("global:<ix_name>") первые 8 байт
function disc8(ixName) {
  const preimage = `global:${ixName}`;
  const h = crypto.createHash("sha256").update(preimage).digest();
  return h.subarray(0, 8);
}

function u64le(n) {
  const bn = BigInt(n.toString());
  const buf = Buffer.alloc(8);
  buf.writeBigUInt64LE(bn);
  return buf;
}

// Надёжная отправка транзакций с ретраями при «Blockhash not found»
async function sendTx(provider, tx, signers = []) {
  const conn = provider.connection;
  let lastErr;

  for (let attempt = 0; attempt < 3; attempt++) {
    try {
      const { blockhash, lastValidBlockHeight } = await conn.getLatestBlockhash(
        "confirmed"
      );
      tx.recentBlockhash = blockhash;
      tx.feePayer = provider.wallet.publicKey;

      for (const s of signers) tx.partialSign(s);

      const signed = await provider.wallet.signTransaction(tx);

      const sig = await conn.sendRawTransaction(signed.serialize(), {
        skipPreflight: false,
        preflightCommitment: "confirmed",
        maxRetries: 3,
      });

      await conn.confirmTransaction(
        { signature: sig, blockhash, lastValidBlockHeight },
        "confirmed"
      );

      return sig;
    } catch (e) {
      lastErr = e;
      const msg = String(e?.message || e).toLowerCase();
      if (msg.includes("blockhash not found") || msg.includes("expired")) {
        // пробуем ещё раз со свежим blockhash
        continue;
      }
      throw e;
    }
  }

  throw lastErr;
}

// ────────────────────────────────
// Основной сценарий
// ────────────────────────────────
(async () => {
  // Провайдер / окружение
  const RPC = mustEnv("ANCHOR_PROVIDER_URL");
  const WALLET_PATH = mustEnv("ANCHOR_WALLET");
  const PROGRAM_ID = new PublicKey(
    pickBase58(mustEnv("PROGRAM_ID"), "PROGRAM_ID")
  );
  const COLLECTION_MINT = new PublicKey(
    pickBase58(mustEnv("COLLECTION_MINT"), "COLLECTION_MINT")
  );

  const provider = anchor.AnchorProvider.env(); // читает из ENV
  anchor.setProvider(provider);
  const conn = provider.connection;
  const wallet = provider.wallet;

  console.log("────────────────────────────────────────────────────────");
  console.log("RPC                  :", RPC);
  console.log("Wallet               :", wallet.publicKey.toBase58());
  console.log("Program ID           :", PROGRAM_ID.toBase58());
  console.log("Collection mint      :", COLLECTION_MINT.toBase58());
  console.log(
    "TokenMetadata PID    :",
    METAPLEX_TOKEN_METADATA_PROGRAM_ID.toBase58()
  );
  console.log("ATA Program PID      :", ASSOCIATED_TOKEN_PROGRAM_ID.toBase58());
  console.log("────────────────────────────────────────────────────────");

  // 1) INIT (создаёт PDA состояния)
  const [statePda] = PublicKey.findProgramAddressSync(
    [Buffer.from("shine_investments_state")],
    PROGRAM_ID
  );

  const initIx = new TransactionInstruction({
    programId: PROGRAM_ID,
    keys: [
      { pubkey: wallet.publicKey, isSigner: true, isWritable: true }, // payer
      { pubkey: statePda, isSigner: false, isWritable: true }, // state_pda
      { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
    ],
    data: Buffer.from([...disc8("init")]), // без аргументов
  });

  try {
    const sigInit = await sendTx(provider, new Transaction().add(initIx));
    console.log(
      "init() tx:",
      sigInit,
      `https://explorer.solana.com/tx/${sigInit}?cluster=devnet`
    );
  } catch (e) {
    console.log("init(): возможно уже выполнен ->", e.message);
  }

  // 2) Локально создаём mint нового NFT и ATA получателя
  const mintPubkey = await createMint(
    conn,
    wallet.payer,
    wallet.publicKey,
    wallet.publicKey,
    0
  );
  console.log("NFT mint:", mintPubkey.toBase58());

  const recipientOwner = wallet.publicKey;
  const recipientAta = await getAssociatedTokenAddress(
    mintPubkey,
    recipientOwner
  );
  const ataInfo = await conn.getAccountInfo(recipientAta);
  if (!ataInfo) {
    const createAtaIx = createAssociatedTokenAccountInstruction(
      wallet.publicKey,
      recipientAta,
      recipientOwner,
      mintPubkey
    );
    const sigAta = await sendTx(
      provider,
      new Transaction().add(createAtaIx)
    );
    console.log("Created ATA:", recipientAta.toBase58(), sigAta);
  } else {
    console.log("ATA exists:", recipientAta.toBase58());
  }

  // PDA для metadata/master edition нашего нового NFT
  const [metadataPda] = PublicKey.findProgramAddressSync(
    [
      Buffer.from("metadata"),
      METAPLEX_TOKEN_METADATA_PROGRAM_ID.toBuffer(),
      mintPubkey.toBuffer(),
    ],
    METAPLEX_TOKEN_METADATA_PROGRAM_ID
  );
  const [masterEditionPda] = PublicKey.findProgramAddressSync(
    [
      Buffer.from("metadata"),
      METAPLEX_TOKEN_METADATA_PROGRAM_ID.toBuffer(),
      mintPubkey.toBuffer(),
      Buffer.from("edition"),
    ],
    METAPLEX_TOKEN_METADATA_PROGRAM_ID
  );

  // PDA коллекции (metadata/master edition)
  const [collectionMetadataPda] = PublicKey.findProgramAddressSync(
    [
      Buffer.from("metadata"),
      METAPLEX_TOKEN_METADATA_PROGRAM_ID.toBuffer(),
      COLLECTION_MINT.toBuffer(),
    ],
    METAPLEX_TOKEN_METADATA_PROGRAM_ID
  );
  const [collectionMasterEditionPda] = PublicKey.findProgramAddressSync(
    [
      Buffer.from("metadata"),
      METAPLEX_TOKEN_METADATA_PROGRAM_ID.toBuffer(),
      COLLECTION_MINT.toBuffer(),
      Buffer.from("edition"),
    ],
    METAPLEX_TOKEN_METADATA_PROGRAM_ID
  );

  // 3) add_bonus(investor: Pubkey, amount: u64) — raw-инструкция
  // Порядок аккаунтов должен совпасть с #[derive(Accounts)] AddBonusCtx:
  // signer(Signer), state_pda(mut), mint_pda(mut), recipient_ata(mut), recipient_owner,
  // collection_mint, collection_metadata_pda(mut), collection_master_edition_pda(mut),
  // collection_update_authority(Signer), metadata_pda(mut), master_edition_pda(mut),
  // token_metadata_program, token_program, associated_token_program, system_program
  const investor = recipientOwner;
  const amount = 123_000_000n; // u64

  const addBonusData = Buffer.concat([
    disc8("add_bonus"),
    investor.toBuffer(),
    u64le(amount),
  ]);

  const addBonusIx = new TransactionInstruction({
    programId: PROGRAM_ID,
    keys: [
      { pubkey: wallet.publicKey, isSigner: true, isWritable: false }, // signer
      { pubkey: statePda, isSigner: false, isWritable: true }, // state_pda
      { pubkey: mintPubkey, isSigner: false, isWritable: true }, // mint_pda
      { pubkey: recipientAta, isSigner: false, isWritable: true }, // recipient_ata
      { pubkey: recipientOwner, isSigner: false, isWritable: false }, // recipient_owner
      { pubkey: COLLECTION_MINT, isSigner: false, isWritable: false }, // collection_mint
      { pubkey: collectionMetadataPda, isSigner: false, isWritable: true }, // collection_metadata_pda
      { pubkey: collectionMasterEditionPda, isSigner: false, isWritable: true }, // collection_master_edition_pda
      { pubkey: wallet.publicKey, isSigner: true, isWritable: false }, // collection_update_authority
      { pubkey: metadataPda, isSigner: false, isWritable: true }, // metadata_pda
      { pubkey: masterEditionPda, isSigner: false, isWritable: true }, // master_edition_pda
      { pubkey: METAPLEX_TOKEN_METADATA_PROGRAM_ID, isSigner: false, isWritable: false },
      { pubkey: TOKEN_PROGRAM_ID, isSigner: false, isWritable: false },
      { pubkey: ASSOCIATED_TOKEN_PROGRAM_ID, isSigner: false, isWritable: false },
      { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
    ],
    data: addBonusData,
  });

  const sig = await sendTx(provider, new Transaction().add(addBonusIx));
  console.log(
    "add_bonus() tx:",
    sig,
    `https://explorer.solana.com/tx/${sig}?cluster=devnet`
  );

  // 4) простые проверки
  const acc = await getAccount(conn, recipientAta);
  console.log("isFrozen (ATA):", acc.isFrozen);
  if (!acc.isFrozen) throw new Error("Ожидали заморозку ATA после add_bonus()");

  const mdInfo = await conn.getAccountInfo(metadataPda);
  if (!mdInfo || mdInfo.data.length === 0)
    throw new Error("Metadata PDA отсутствует или пуст");

  console.log(
    "Готово: raw-инструкции прошли, NFT создан/верифицирован и ATA заморожен"
  );
})().catch((e) => {
  console.error("Ошибка e2e:", e);
  process.exit(1);
});
