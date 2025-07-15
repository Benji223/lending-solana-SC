import { useConnection, useWallet } from '@solana/wallet-adapter-react';
import { PublicKey, SystemProgram, Transaction, LAMPORTS_PER_SOL } from '@solana/web3.js';
import { Program, AnchorProvider, web3, BN } from '@coral-xyz/anchor';
import { getAssociatedTokenAddress, TOKEN_PROGRAM_ID, ASSOCIATED_TOKEN_PROGRAM_ID } from '@solana/spl-token';
import { FC, useState, useEffect } from 'react';
import idl from './idl.json';

// --- Constants ---
const programId = new PublicKey(idl.address);
// Devnet Mints
const USDC_MINT = new PublicKey("Gh9ZwEmdLJ8DscKNTkTqPbNwLNNBjuSzaG9Vp2KGtKJr");
const SOL_MINT = new PublicKey("So11111111111111111111111111111111111111112"); // Wrapped SOL

type TokenName = 'USDC' | 'SOL';

export const LendingComponent: FC = () => {
    const { connection } = useConnection();
    const wallet = useWallet();
    const { publicKey, sendTransaction } = wallet;

    // --- State ---
    const [amount, setAmount] = useState('0');
    const [selectedToken, setSelectedToken] = useState<TokenName>('USDC');
    const [userSolBalance, setUserSolBalance] = useState<number | null>(null);
    const [userUsdcBalance, setUserUsdcBalance] = useState<number | null>(null);

    // --- Anchor Setup ---
    const getProvider = () => {
        if (!wallet) return null;
        // The wallet object from wallet-adapter is not directly compatible
        // with the Wallet interface from Anchor, so we cast it to 'any'.
        // This is a common and safe pattern.
        const provider = new AnchorProvider(connection, wallet as any, AnchorProvider.defaultOptions());
        return provider;
    };

    const getProgram = () => {
        const provider = getProvider();
        if (!provider) return null;
        // The "any" type is used here because the IDL structure from the JSON file
        // might not perfectly match the expected type definitions from the Anchor library.
        const program = new Program(idl as any, provider);
        return program;
    };

    // --- Balance Fetching ---
    const fetchBalances = async () => {
        if (!publicKey) return;
        // SOL balance
        const solBalance = await connection.getBalance(publicKey);
        setUserSolBalance(solBalance / LAMPORTS_PER_SOL);

        // USDC balance
        try {
            const usdcAta = await getAssociatedTokenAddress(USDC_MINT, publicKey);
            const usdcAccountInfo = await connection.getTokenAccountBalance(usdcAta);
            setUserUsdcBalance(usdcAccountInfo.value.uiAmount);
        } catch (e) {
            setUserUsdcBalance(0); // User likely doesn't have a USDC account yet
        }
    };

    useEffect(() => {
        if (wallet.connected) {
            fetchBalances();
        }
    }, [wallet.connected, connection, publicKey]);


    // --- Instruction Handlers ---

    const initBank = async (mint: PublicKey) => {
        const program = getProgram();
        const provider = getProvider();
        if (!program || !provider || !publicKey) return alert("Wallet not connected");

        const [bank] = PublicKey.findProgramAddressSync(
            [mint.toBuffer()],
            program.programId
        );
        const [bankTokenAccount] = PublicKey.findProgramAddressSync(
            [Buffer.from("treasury"), mint.toBuffer()],
            program.programId
        );

        try {
            const tx = await program.methods
                .initBank(new BN(80), new BN(70)) // 80% liquidation_threshold, 70% max_ltv
                .accounts({
                    signer: publicKey,
                    mint: mint,
                    bank: bank,
                    bankTokenAccount: bankTokenAccount,
                    tokenProgram: TOKEN_PROGRAM_ID,
                    systemProgram: SystemProgram.programId,
                })
                .transaction();

            const signature = await sendTransaction(tx, connection);
            await connection.confirmTransaction(signature, 'confirmed');
            alert("Bank initialized successfully!");
        } catch (err) {
            console.error("Error initializing bank:", err);
            alert("Failed to initialize bank. It may already exist.");
        }
    };

    const initUser = async () => {
        const program = getProgram();
        const provider = getProvider();
        if (!program || !provider || !publicKey) return alert("Wallet not connected");

        const [userAccount] = PublicKey.findProgramAddressSync(
            [publicKey.toBuffer()],
            program.programId
        );

        try {
            const tx = await program.methods
                .initUser(USDC_MINT) // Storing the USDC mint for reference
                .accounts({
                    userAccount: userAccount,
                    signer: publicKey,
                    systemProgram: SystemProgram.programId,
                })
                .transaction();
            
            const signature = await sendTransaction(tx, connection);
            await connection.confirmTransaction(signature, 'confirmed');
            alert("User initialized successfully!");
        } catch (err) {
            console.error("Error initializing user:", err);
            alert("Failed to initialize user. It may already exist.");
        }
    };

    const deposit = async () => {
        const program = getProgram();
        if (!program || !publicKey) return;

        const mint = selectedToken === 'USDC' ? USDC_MINT : SOL_MINT;
        const depositAmount = new BN(parseFloat(amount) * (10 ** (selectedToken === 'USDC' ? 6 : 9)));

        const [bank] = PublicKey.findProgramAddressSync([mint.toBuffer()], program.programId);
        const [bankTokenAccount] = PublicKey.findProgramAddressSync([Buffer.from("treasury"), mint.toBuffer()], program.programId);
        const [userAccount] = PublicKey.findProgramAddressSync([publicKey.toBuffer()], program.programId);
        const userTokenAccount = await getAssociatedTokenAddress(mint, publicKey);

        try {
            const tx = await program.methods
                .deposit(depositAmount)
                .accounts({
                    signer: publicKey,
                    mint: mint,
                    bank: bank,
                    bankTokenAccount: bankTokenAccount,
                    userAccount: userAccount,
                    userTokenAccount: userTokenAccount,
                    tokenProgram: TOKEN_PROGRAM_ID,
                    associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
                    systemProgram: SystemProgram.programId,
                })
                .transaction();

            const signature = await sendTransaction(tx, connection);
            await connection.confirmTransaction(signature, 'confirmed');
            alert("Deposit successful!");
            fetchBalances();
        } catch (err) {
            console.error("Error during deposit:", err);
            alert("Deposit failed.");
        }
    };

    // --- Render Logic ---

    if (!wallet.connected || !publicKey) {
        return <p>Please connect your wallet to continue.</p>;
    }

    return (
        <div className="lending-container">
            <div className="balances">
                <p>SOL Balance: {userSolBalance?.toFixed(4) ?? 'Loading...'}</p>
                <p>USDC Balance: {userUsdcBalance?.toFixed(4) ?? 'Loading...'}</p>
            </div>

            <div className="action-section">
                <h3>Admin Actions</h3>
                <button onClick={() => initBank(USDC_MINT)}>Init USDC Bank</button>
                <button onClick={() => initBank(SOL_MINT)}>Init SOL Bank</button>
                <button onClick={initUser}>Init User Account</button>
            </div>

            <div className="action-section">
                <h3>User Actions</h3>
                <div className="form-group">
                    <label htmlFor="token-select">Token</label>
                    <select id="token-select" value={selectedToken} onChange={e => setSelectedToken(e.target.value as TokenName)}>
                        <option value="USDC">USDC</option>
                        <option value="SOL">SOL</option>
                    </select>
                </div>
                <div className="form-group">
                    <label htmlFor="amount">Amount</label>
                    <input
                        id="amount"
                        type="number"
                        value={amount}
                        onChange={(e) => setAmount(e.target.value)}
                        placeholder="e.g., 100"
                    />
                </div>
                <button onClick={deposit}>Deposit</button>
                <button onClick={() => alert("Not implemented")} >Withdraw</button>
                <button onClick={() => alert("Not implemented")} >Borrow</button>
                <button onClick={() => alert("Not implemented")} >Repay</button>
            </div>
        </div>
    );
};