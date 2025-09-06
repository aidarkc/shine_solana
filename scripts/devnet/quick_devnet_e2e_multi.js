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

// Programs
const MPL = new PublicKey("metaqbxxUerdq28cj1RbAWkYQm3ybzjb6a8bt518x1s");

// utils
const BASE58_RE = /[1-9A-HJ-NP-Za-km-z]{32,}/g;
function mustEnv(name) {
  const v = (process.env[name] || "").trim();
  if (!v) throw new Error(`ENV ${name} is required`);
  return v;
}
function pickBase58(raw, name) {
  const m = (raw || "").toString().match(BASE58_RE);
  if (!m) throw new Error(`${name} invalid: "${raw}"`);
  return m[0];
}
function disc8(name) {
  const preimage = `global:${name}`;
  const h = crypto.createHash("sha256").update(preimage).digest();
  return h.subarray(0, 8);
}
function u64le(n) {
  const bn = BigInt(n.toString());
  const buf = Buffer.alloc(8);
  buf.writeBigUInt64LE(bn);
  return buf;
}
async function sendTx(provider, tx, signers = []) {
  const conn = provider.connection;
  const { blockhash, lastValidBlockHeight } = await conn.getLatestBlockhash("confirmed");
  tx.recentBlockhash = blockhash;
  tx.feePayer = provider.wallet.publicKey;
  for (const s of signers) tx.partialSign(s);
  const raw = await provider.wallet.signTransaction(tx);
  const sig = await conn.sendRawTransaction(raw.serialize(), {
    skipPreflight: false,
    preflightCommitment: "confirmed",
    maxRetries: 3,
  });
  await conn.confirmTransaction({ signature: sig, blockhash, lastValidBlockHeight }, "confirmed");
  return sig;
}

(async () => {
  const count = Number(process.argv[2] || "3"); // сколько NFT сделать
  const RPC = mustEnv("ANCHOR_PROVIDER_URL");
  const PROGRAM_ID = new PublicKey(pickBase58(mustEnv("PROGRAM_ID"), "PROGRAM_ID"));
  const COLLECTION_MINT = new PublicKey(pickBase58(mustEnv("COLLECTION_MINT"), "COLLECTION_MINT"));

  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);
  const conn = provider.connection;
  const wallet = provider.wallet;

  console.log("────────────────────────────────────────────────────────");
  console.log("RPC                  :", RPC);
  console.log("Wallet               :", wallet.publicKey.toBase58());
  console.log("Program ID           :", PROGRAM_ID.toBase58());
  console.log("Collection mint      :", COLLECTION_MINT.toBase58());
  console.log("TokenMetadata PID    :", MPL.toBase58());
  console.log("ATA Program PID      :", ASSOCIATED_TOKEN_PROGRAM_ID.toBase58());
  console.log("Count                :", count);
  console.log("────────────────────────────────────────────────────────");

  // ensure init (если уже есть — просто пропустим)
  const [statePda] = PublicKey.findProgramAddressSync(
    [Buffer.from("shine_investments_state")],
    PROGRAM_ID
  );
  const initIx = new TransactionInstruction({
    programId: PROGRAM_ID,
    keys: [
      { pubkey: wallet.publicKey, isSigner: true, isWritable: true }, // payer
      { pubkey: statePda, isSigner: false, isWritable: true },        // state_pda
      { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
    ],
    data: Buffer.from([...disc8("init")]),
  });
  try {
    const sigInit = await sendTx(provider, new Transaction().add(initIx));
    console.log("init() tx:", sigInit, `https://explorer.solana.com/tx/${sigInit}?cluster=devnet`);
  } catch (e) {
    console.log("init(): возможно уже выполнен ->", e.message);
  }

  const minted = [];

  for (let i = 0; i < count; i++) {
    // 1) создаём новый mint (NFT)
    const mintPubkey = await createMint(conn, wallet.payer, wallet.publicKey, wallet.publicKey, 0);
    console.log(`\n[${i + 1}/${count}] NFT mint:`, mintPubkey.toBase58());

    // 2) создаём ATA при необходимости
    const recipientOwner = wallet.publicKey;
    const recipientAta = await getAssociatedTokenAddress(mintPubkey, recipientOwner);
    const ataInfo = await conn.getAccountInfo(recipientAta);
    if (!ataInfo) {
      const createAtaIx = createAssociatedTokenAccountInstruction(
        wallet.publicKey, recipientAta, recipientOwner, mintPubkey
      );
      const sigAta = await sendTx(provider, new Transaction().add(createAtaIx));
      console.log("  ATA created:", recipientAta.toBase58(), sigAta);
    } else {
      console.log("  ATA exists:", recipientAta.toBase58());
    }

    // PDA для нашего NFT (metadata/master edition)
    const [metadataPda] = PublicKey.findProgramAddressSync(
      [Buffer.from("metadata"), MPL.toBuffer(), mintPubkey.toBuffer()],
      MPL
    );
    const [masterEditionPda] = PublicKey.findProgramAddressSync(
      [Buffer.from("metadata"), MPL.toBuffer(), mintPubkey.toBuffer(), Buffer.from("edition")],
      MPL
    );

    // PDA коллекции
    const [collectionMetadataPda] = PublicKey.findProgramAddressSync(
      [Buffer.from("metadata"), MPL.toBuffer(), COLLECTION_MINT.toBuffer()],
      MPL
    );
    const [collectionMasterEditionPda] = PublicKey.findProgramAddressSync(
      [Buffer.from("metadata"), MPL.toBuffer(), COLLECTION_MINT.toBuffer(), Buffer.from("edition")],
      MPL
    );

    // 3) add_bonus(investor, amount)
    const investor = recipientOwner;
    // Для наглядности — разные суммы
    const amount = BigInt(100_000_000 + i * 10_000_000); // 100M, 110M, 120M...

    const addBonusData = Buffer.concat([
      disc8("add_bonus"),
      investor.toBuffer(),
      u64le(amount),
    ]);

    const addBonusIx = new TransactionInstruction({
      programId: PROGRAM_ID,
      keys: [
        { pubkey: wallet.publicKey, isSigner: true,  isWritable: false }, // signer
        { pubkey: statePda,        isSigner: false, isWritable: true  }, // state_pda
        { pubkey: mintPubkey,      isSigner: false, isWritable: true  }, // mint_pda
        { pubkey: recipientAta,    isSigner: false, isWritable: true  }, // recipient_ata
        { pubkey: recipientOwner,  isSigner: false, isWritable: false }, // recipient_owner
        { pubkey: COLLECTION_MINT, isSigner: false, isWritable: false }, // collection_mint
        { pubkey: collectionMetadataPda,      isSigner: false, isWritable: true  },
        { pubkey: collectionMasterEditionPda, isSigner: false, isWritable: true  },
        { pubkey: wallet.publicKey, isSigner: true,  isWritable: false }, // collection_update_authority
        { pubkey: metadataPda,      isSigner: false, isWritable: true  },
        { pubkey: masterEditionPda, isSigner: false, isWritable: true  },
        { pubkey: MPL,                       isSigner: false, isWritable: false },
        { pubkey: TOKEN_PROGRAM_ID,          isSigner: false, isWritable: false },
        { pubkey: ASSOCIATED_TOKEN_PROGRAM_ID,isSigner: false, isWritable: false },
        { pubkey: SystemProgram.programId,   isSigner: false, isWritable: false },
      ],
      data: addBonusData,
    });

    const sig = await sendTx(provider, new Transaction().add(addBonusIx));
    console.log("  add_bonus() tx:", sig, `https://explorer.solana.com/tx/${sig}?cluster=devnet`);

    // 4) проверки
    const acc = await getAccount(conn, recipientAta);
    console.log("  ATA frozen:", acc.isFrozen);

    minted.push({
      mint: mintPubkey.toBase58(),
      ata: recipientAta.toBase58(),
      addBonusSig: sig,
      metadataPda: metadataPda.toBase58(),
    });
  }

  console.log("\n==================== SUMMARY ====================");
  console.log("Wallet:", wallet.publicKey.toBase58());
  console.log("Collection:", COLLECTION_MINT.toBase58());
  console.table(minted);
  console.log("Открой каждую сигнатуру (add_bonus tx) и mint в Explorer:");
  minted.forEach((m, i) => {
    console.log(`[${i + 1}] Mint: https://explorer.solana.com/address/${m.mint}?cluster=devnet`);
    console.log(`    TX : https://explorer.solana.com/tx/${m.addBonusSig}?cluster=devnet`);
  });
  console.log("=================================================");
})().catch((e) => {
  console.error("Ошибка:", e);
  process.exit(1);
});
