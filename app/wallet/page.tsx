"use client";

import { useState, useEffect, useCallback } from "react";
import { Wallet, Send, RefreshCw, Copy, Check, ExternalLink, Search, Loader2, Lock, Upload, FileText } from "lucide-react";
import Link from "next/link";
import { parseTransactions, parseAddresses, zatoshisToZec, zecToZatoshis, ParsedTransaction, ParsedAddress } from "@/lib/parse-wallet";

type WalletType = "zcash" | "miden";

export default function WalletPage() {
  const [walletType, setWalletType] = useState<WalletType>("zcash");

  // Zcash wallet state
  const [balance, setBalance] = useState<string>("0");
  const [loading, setLoading] = useState(false);
  const [syncing, setSyncing] = useState(false);
  const [addresses, setAddresses] = useState<ParsedAddress[]>([]);
  const [transactions, setTransactions] = useState<ParsedTransaction[]>([]);
  const [copied, setCopied] = useState<string | null>(null);
  const [sendAddress, setSendAddress] = useState("");
  const [sendAmount, setSendAmount] = useState("");
  const [sendMemo, setSendMemo] = useState("");
  const [sending, setSending] = useState(false);
  const [sendResult, setSendResult] = useState<{ success: boolean; message: string } | null>(null);

  // Miden wallet state
  const [client, setClient] = useState<any>(null);
  const [connected, setConnected] = useState(false);
  const [accountId, setAccountId] = useState("");
  const [account, setAccount] = useState<any>(null);
  const [scanning, setScanning] = useState(false);
  const [notes, setNotes] = useState<any[]>([]);
  const [consuming, setConsuming] = useState(false);
  const [error, setError] = useState("");
  const [success, setSuccess] = useState<string | null>(null);
  const [connecting, setConnecting] = useState(false);
  const [midenBalance, setMidenBalance] = useState<string>("0");
  const [loadingBalance, setLoadingBalance] = useState(false);
  const [uploadingMno, setUploadingMno] = useState(false);
  const [mnoFile, setMnoFile] = useState<File | null>(null);

  // Zcash wallet functions - defined before useEffect
  const loadBalance = useCallback(async () => {
    setLoading(true);
    try {
      const res = await fetch("/api/wallet/balance");
      const data = await res.json();
      console.log("Balance API response:", data);
      if (data.success && data.balance) {
        setBalance(data.balance.total || "0");
      } else {
        console.error("Balance API error:", data.error || "Unknown error");
      }
    } catch (error) {
      console.error("Failed to load balance:", error);
    } finally {
      setLoading(false);
    }
  }, []);

  const loadAddresses = useCallback(async () => {
    try {
      const res = await fetch("/api/wallet/addresses");
      const data = await res.json();
      console.log("Addresses API response:", data);
      if (data.success) {
        const parsed = parseAddresses(data.raw);
        setAddresses(parsed);
      } else {
        console.error("Addresses API error:", data.error || "Unknown error");
      }
    } catch (error) {
      console.error("Failed to load addresses:", error);
    }
  }, []);

  const loadTransactions = useCallback(async () => {
    try {
      const res = await fetch("/api/wallet/transactions");
      const data = await res.json();
      console.log("Transactions API response:", data);
      if (data.success) {
        const parsed = parseTransactions(data.raw);
        setTransactions(parsed);
      } else {
        console.error("Transactions API error:", data.error || "Unknown error");
      }
    } catch (error) {
      console.error("Failed to load transactions:", error);
    }
  }, []);

  // Miden wallet functions - defined before useEffect
  const setupMidenClient = useCallback(async () => {
    if (typeof window === "undefined") return;
    
    try {
      setConnecting(true);
      setError("");
      
      const { WebClient, AccountStorageMode, NetworkId } = await import("@demox-labs/miden-sdk");
      const client = await WebClient.createClient("https://rpc.testnet.miden.io");
      setClient(client);
      
      // Note: Bridge note tag registration is handled by the backend
      // The WebClient SDK doesn't have addNoteTag method, so we skip this
      // The backend handles note tag registration when consuming notes
      
      const storedAccountId = localStorage.getItem("miden_account_id");
      
      if (storedAccountId) {
        setAccountId(storedAccountId);
        await client.syncState();
        // Get account object
        const accounts = await client.getAccounts();
        const userAccount = accounts.find((acc: any) => {
          try {
            return (acc.id() as any).toBech32?.(NetworkId.Testnet) === storedAccountId;
          } catch {
            return false;
          }
        });
        if (userAccount) {
          setAccount(userAccount);
          // Auto-scan for notes
          await scanForNotes(userAccount);
        }
        setConnected(true);
        setConnecting(false);
        return;
      }
      
      // Create wallet via backend (backend stores the key)
      const backendUrl = process.env.NEXT_PUBLIC_BACKEND_URL || "http://127.0.0.1:8001";
      const createResponse = await fetch(`${backendUrl}/account/create`, {
        method: "POST",
        headers: {
          "Content-Type": "application/json",
        },
      });
      
      if (!createResponse.ok) {
        const errorText = await createResponse.text();
        throw new Error(`Backend error creating wallet: ${errorText}`);
      }
      
      const accountData = await createResponse.json();
      console.log("✅ Backend created wallet:");
      console.log("   Bech32:", accountData.account_id);
      console.log("   Hex:", accountData.account_id_hex);
      
      // Store both formats
      localStorage.setItem("miden_account_id", accountData.account_id); // bech32
      // Store hex without 0x prefix for consistency
      const hexWithoutPrefix = accountData.account_id_hex.startsWith("0x") 
        ? accountData.account_id_hex.slice(2) 
        : accountData.account_id_hex;
      localStorage.setItem("miden_account_id_hex", hexWithoutPrefix); // hex without 0x
      
      setAccountId(accountData.account_id);
      
      // Sync client state and try to load account (might not be in client store yet)
      await client.syncState();
      
      // Try to get account from client (might not exist if it's a new wallet)
      const accounts = await client.getAccounts();
      const userAccount = accounts.find((acc: any) => {
        try {
          return (acc.id() as any).toBech32?.(NetworkId.Testnet) === accountData.account_id;
        } catch {
          return false;
        }
      });
      
      if (userAccount) {
        setAccount(userAccount);
        await scanForNotes(userAccount);
      } else {
        console.warn("Account not found in client store yet. It will be available after sync.");
      }
      
      setConnected(true);
      setConnecting(false);
    } catch (err: any) {
      console.error("Failed to setup wallet:", err);
      setError(`Failed to setup wallet: ${err.message || String(err)}`);
      setConnecting(false);
      setConnected(false);
    }
  }, []);

  // Load Zcash data when Zcash wallet is selected
  useEffect(() => {
    if (walletType === "zcash") {
      loadBalance();
      loadAddresses();
      loadTransactions();
    }
  }, [walletType, loadBalance, loadAddresses, loadTransactions]);

  // Load Miden balance
  const loadMidenBalance = useCallback(async (accountObj?: any) => {
    if (!accountId) return;
    
    try {
      setLoadingBalance(true);
      
      // Use backend endpoint to get balance (backend has the account)
      const backendUrl = process.env.NEXT_PUBLIC_BACKEND_URL || "http://127.0.0.1:8001";
      const balanceResponse = await fetch(`${backendUrl}/account/balance`, {
        method: "POST",
        headers: {
          "Content-Type": "application/json",
        },
        body: JSON.stringify({
          account_id: accountId, // Backend accepts bech32 or hex
        }),
      });
      
      if (!balanceResponse.ok) {
        let errorMessage = "Failed to get balance";
        try {
          const errorData = await balanceResponse.json();
          errorMessage = errorData.error || errorMessage;
        } catch {
          const errorText = await balanceResponse.text();
          errorMessage = errorText || errorMessage;
        }
        console.error("Backend balance error:", errorMessage);
        setMidenBalance("0");
        return;
      }
      
      const balanceData = await balanceResponse.json();
      if (balanceData.success && balanceData.balance !== undefined) {
        console.log("✅ Balance from backend:", balanceData.balance);
        setMidenBalance(balanceData.balance);
      } else {
        console.warn("Invalid balance response:", balanceData);
        setMidenBalance("0");
      }
      
      // Fallback: Try WebClient if available (for accounts created in browser)
      if (client && accountObj) {
        try {
      await client.syncState();
      
      // Get WTAZ faucet ID from env
      const { AccountId } = await import("@demox-labs/miden-sdk");
      const faucetIdHex = process.env.NEXT_PUBLIC_FAUCET_ID;
      if (!faucetIdHex) {
            return; // Already set balance from backend
      }
      const faucetId = AccountId.fromHex(faucetIdHex);
      
      // Get account record from client - this should have the vault
          const accountRecord = await client.getAccount(accountObj.id());
      
        if (accountRecord) {
          // Try to get vault from account record
          if (typeof accountRecord.vault === 'function') {
            const vault = accountRecord.vault();
              
              if (vault && typeof vault.getBalance === 'function') {
                const balance = vault.getBalance(faucetId);
                  if (balance !== null && balance !== undefined) {
                    const balanceNum = typeof balance === 'bigint' ? Number(balance) : Number(balance);
                    const balanceInTokens = balanceNum / 1e8;
                    const balanceStr = balanceInTokens % 1 === 0 
                      ? balanceInTokens.toString() 
                      : balanceInTokens.toFixed(8).replace(/\.?0+$/, '');
                  console.log("Balance from WebClient:", balanceStr);
                    setMidenBalance(balanceStr);
                  }
                }
            }
                      }
                    } catch (e) {
          console.warn("WebClient balance fallback failed:", e);
          // Keep backend balance
        }
      }
    } catch (err: any) {
      console.error("Failed to load balance:", err);
      setError(`Failed to load balance: ${err.message}`);
    } finally {
      setLoadingBalance(false);
    }
  }, [client, accountId, account]);

  // Load balance when account changes
  useEffect(() => {
    if (walletType === "miden" && connected && account) {
      loadMidenBalance();
    }
  }, [walletType, connected, account, loadMidenBalance]);

  // Load accountId from localStorage on mount
  useEffect(() => {
    if (typeof window !== "undefined") {
      const storedAccountId = localStorage.getItem("miden_account_id");
      if (storedAccountId && !accountId) {
        setAccountId(storedAccountId);
      }
    }
  }, []);

  // Initialize Miden wallet when Miden wallet is selected
  useEffect(() => {
    if (walletType === "miden") {
      setupMidenClient();
    }
  }, [walletType, setupMidenClient]);

  const handleSync = async () => {
    setSyncing(true);
    try {
      const res = await fetch("/api/wallet/sync", { method: "POST" });
      const data = await res.json();
      if (data.success) {
        setTimeout(() => {
          loadBalance();
          loadTransactions();
        }, 2000);
      }
    } catch (error) {
      console.error("Failed to sync:", error);
    } finally {
      setSyncing(false);
    }
  };

  const handleSend = async (e: React.FormEvent) => {
    e.preventDefault();
    setSending(true);
    setSendResult(null);

    try {
      const amountInZatoshis = zecToZatoshis(sendAmount);
      const res = await fetch("/api/wallet/send", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          address: sendAddress,
          amount: amountInZatoshis,
          memo: sendMemo || undefined,
        }),
      });

      const data = await res.json();
      if (data.success) {
        setSendResult({ success: true, message: `Transaction sent! TXID: ${data.txid}` });
        setSendAddress("");
        setSendAmount("");
        setSendMemo("");
        setTimeout(() => {
          loadBalance();
          loadTransactions();
        }, 2000);
      } else {
        setSendResult({ success: false, message: data.error || "Failed to send transaction" });
      }
    } catch (error: any) {
      setSendResult({ success: false, message: error.message || "Failed to send transaction" });
    } finally {
      setSending(false);
    }
  };

  const scanForNotes = async (accountObj?: any) => {
    if (!client || !accountId) {
      return;
    }

    try {
      setError("");
      setScanning(true);
      
      // Try to get account object - load from client if not available
      let acc = accountObj || account;
      if (!acc && accountId) {
        try {
          await client.syncState();
          const accounts = await client.getAccounts();
          const { NetworkId } = await import("@demox-labs/miden-sdk");
          acc = accounts.find((a: any) => {
            try {
              return (a.id() as any).toBech32?.(NetworkId.Testnet) === accountId;
            } catch {
              return false;
            }
          });
          if (acc) {
            setAccount(acc);
          }
        } catch (e) {
          console.warn("Failed to load account from client:", e);
        }
      }
      
      if (!acc) {
        console.warn("No account object available for scanning notes. Account may not be in WebClient store yet.");
        console.warn("The account was created by the backend and may not be in the WebClient's IndexedDB yet.");
        console.warn("Notes will be available after the account is synced to the WebClient store.");
        return;
      }
      
      console.log("Syncing state before scanning notes...");
      await client.syncState();
      
      // Log account info for debugging
      const accountIdObj = acc.id();
      const accountIdHex = (accountIdObj as any).toHex ? (accountIdObj as any).toHex() : accountIdObj.toString();
      const accountIdHexOnly = accountIdHex.startsWith('0x') ? accountIdHex.slice(2) : accountIdHex;
      const { NetworkId } = await import("@demox-labs/miden-sdk");
      const accountIdBech32 = accountIdObj.toBech32(NetworkId.Testnet);
      
      console.log("=== Account Info ===");
      console.log("Account ID (toString):", accountIdObj.toString());
      console.log("Account ID (toHex):", accountIdHex);
      console.log("Account ID (hex only, no padding):", accountIdHexOnly);
      console.log("Account ID (hex length):", accountIdHexOnly.length);
      console.log("Account ID (bech32):", accountIdBech32);
      console.log("===================");
      
      console.log("Getting consumable notes for account:", acc.id().toString());
      const consumableNotes = await client.getConsumableNotes(acc.id());
      console.log(`Found ${consumableNotes.length} consumable note(s)`);
      
      // Log note details for debugging
      for (const note of consumableNotes) {
        const noteRecord = note.inputNoteRecord();
        const noteId = noteRecord.id().toString();
        console.log("Note ID:", noteId);
        // Try to get note tag if available
        try {
          const noteTag = (noteRecord as any).metadata?.tag;
          console.log("  Note tag:", noteTag);
        } catch (e) {
          // Ignore if tag not accessible
        }
      }
      
      const matchingNotes = [];
      for (const note of consumableNotes) {
        const noteRecord = note.inputNoteRecord();
        const noteId = noteRecord.id().toString();
        console.log("Note ID:", noteId);
        matchingNotes.push({ id: noteId, note: noteRecord });
      }
      
      setNotes(matchingNotes);
      
      if (matchingNotes.length === 0) {
        console.log("No consumable notes found. Make sure:");
        console.log("1. The note transaction has been confirmed on-chain");
        console.log("2. You've synced your wallet (click the refresh button)");
        console.log("3. For private notes, ensure you have the correct account");
      }
    } catch (err: any) {
      const errorMsg = `Failed to scan notes: ${err.message}`;
      setError(errorMsg);
      console.error("Scan error:", err);
    } finally {
      setScanning(false);
    }
  };

  const consumeNote = async (noteId: string) => {
    if (!client || !accountId || !account) {
      setError("Please connect wallet");
      return;
    }

    try {
      setError("");
      setConsuming(true);
      
      // Use the correct API: submitNewTransaction (not newTransaction + submitTransaction)
      const consumeTxRequest = client.newConsumeTransactionRequest([noteId]);
      await client.submitNewTransaction(account.id(), consumeTxRequest);
      
      // Wait for transaction confirmation
      console.log("Waiting 5 seconds for transaction confirmation...");
      await new Promise((resolve) => setTimeout(resolve, 5000));
      
      // Sync state to update balance
      await client.syncState();
      
      // Reload account to get fresh state
      const accounts = await client.getAccounts();
      const updatedAccount = accounts.find((a: any) => {
        try {
          return a.id().toString() === account.toString();
        } catch {
          return false;
        }
      });
      if (updatedAccount) {
        setAccount(updatedAccount);
      }
      
      setSuccess("Note consumed successfully! Your balance has been updated.");
      await loadMidenBalance(updatedAccount);
      await scanForNotes(updatedAccount);
      
      // Clear success message after 5 seconds
      setTimeout(() => setSuccess(null), 5000);
    } catch (err: any) {
      setError(`Failed to consume note: ${err.message}`);
      console.error("Consume error:", err);
    } finally {
      setConsuming(false);
    }
  };

  const consumeMnoFile = async (file: File) => {
    if (!client || !accountId) {
      setError("Please connect wallet first");
      return;
    }
    
    // Account object is optional - backend handles consumption
    // Try to load it for balance refresh, but don't require it
    let accountToUse = account;
    if (!accountToUse && accountId) {
      try {
        await client.syncState();
        const accounts = await client.getAccounts();
        const { NetworkId } = await import("@demox-labs/miden-sdk");
        accountToUse = accounts.find((acc: any) => {
          try {
            return (acc.id() as any).toBech32?.(NetworkId.Testnet) === accountId;
          } catch {
            return false;
          }
        });
        if (accountToUse) {
          setAccount(accountToUse);
        }
      } catch (e) {
        console.warn("Failed to load account from client:", e);
      }
    }
    
    // Don't require account object - backend handles everything
    // We'll just skip balance refresh if account is not available

    try {
      setError("");
      setUploadingMno(true);
      
      // Read and parse .mno file
      const text = await file.text();
      const mnoData = JSON.parse(text);
      
      console.log("Parsed .mno file:", mnoData);
      
      // Validate required fields
      if (!mnoData.account_id || !mnoData.secret || !mnoData.faucet_id || !mnoData.amount) {
        throw new Error("Invalid .mno file: missing required fields (account_id, secret, faucet_id, amount)");
      }
      
      // Backend handles account_id parsing (accepts both bech32 and hex)
      // We just pass the account_id as-is from the .mno file or use current wallet
      const currentAccountIdBech32 = accountId;
      const mnoAccountId = mnoData.account_id.trim();
      
      // Simple check: if both are bech32, compare directly; otherwise backend will validate
      if (mnoAccountId.startsWith("mtst") && currentAccountIdBech32 && mnoAccountId !== currentAccountIdBech32) {
        console.warn("⚠️ Note account_id doesn't match current wallet, but proceeding (backend will validate)");
      }
      
      // Call backend to consume the note (backend executes the transaction)
      console.log("Calling backend to consume note...");
      const backendUrl = process.env.NEXT_PUBLIC_BACKEND_URL || "http://127.0.0.1:8001";
      
      // Use current account ID (can be bech32 or hex, backend handles both)
      const accountIdToSend = currentAccountIdBech32 || mnoAccountId;
      
      const consumeResponse = await fetch(`${backendUrl}/note/consume`, {
        method: "POST",
        headers: {
          "Content-Type": "application/json",
        },
        body: JSON.stringify({
          account_id: accountIdToSend, // Backend accepts bech32 or hex
          secret: mnoData.secret,
          faucet_id: mnoData.faucet_id,
          amount: mnoData.amount,
        }),
      });
      
      if (!consumeResponse.ok) {
        let errorMessage = "Backend error consuming note";
        try {
          const errorData = await consumeResponse.json();
          errorMessage = errorData.error || errorMessage;
        } catch {
          // If response is not JSON, try text
          const errorText = await consumeResponse.text();
          errorMessage = errorText || errorMessage;
        }
        throw new Error(errorMessage);
      }
      
      const consumeData = await consumeResponse.json();
      console.log("✅ Backend consumed note:");
      console.log("   Transaction ID:", consumeData.transaction_id);
      console.log("   Note ID:", consumeData.note_id);
      
      // Wait a bit for transaction to be processed
      await new Promise((resolve) => setTimeout(resolve, 3000));
      
      // Sync state to update balance
      if (client) {
        await client.syncState();
        
        // Try to reload account for balance refresh (optional)
        const accounts = await client.getAccounts();
        const { NetworkId } = await import("@demox-labs/miden-sdk");
        const updatedAccount = accounts.find((a: any) => {
          try {
            return (a.id() as any).toBech32?.(NetworkId.Testnet) === accountId;
          } catch {
            return false;
          }
        });
        if (updatedAccount) {
          setAccount(updatedAccount);
          await loadMidenBalance(updatedAccount);
          await scanForNotes(updatedAccount);
        } else {
          // Account not in client yet, but consumption succeeded
          // Try to refresh balance using accountId if we have it
          console.log("Account not in client store yet, but note consumption succeeded");
          // Balance will be updated on next manual refresh or reconnect
        }
      }
      
      const amountTaz = mnoData.amount_taz || (Number(mnoData.amount) / 100_000_000).toFixed(8);
      setSuccess(`Successfully consumed note! Received ${amountTaz} wTAZ. Transaction: ${consumeData.transaction_id}`);
      
      // Clear file
      setMnoFile(null);
      
      // Clear success message after 8 seconds
      setTimeout(() => setSuccess(null), 8000);
    } catch (err: any) {
      const errorMsg = `Failed to consume .mno file: ${err.message}`;
      setError(errorMsg);
      console.error("MNO consume error:", err);
    } finally {
      setUploadingMno(false);
    }
  };

  const handleMnoFileUpload = async (event: React.ChangeEvent<HTMLInputElement>) => {
    const file = event.target.files?.[0];
    if (!file) return;
    
    if (!file.name.endsWith('.mno')) {
      setError("Please upload a .mno file");
      return;
    }
    
    setMnoFile(file);
    await consumeMnoFile(file);
  };

  const clearWallet = async () => {
    if (!confirm("Are you sure you want to clear your wallet?")) {
      return;
    }

    try {
      localStorage.removeItem("miden_account_id");
      localStorage.removeItem("miden_account_id_hex");
      
      if (window.indexedDB) {
        const deleteReq = indexedDB.deleteDatabase("miden-wallet");
        deleteReq.onsuccess = () => {
          window.location.reload();
        };
      } else {
        window.location.reload();
      }
    } catch (err: any) {
      console.error("Failed to clear wallet:", err);
      setError(`Failed to clear wallet: ${err.message}`);
    }
  };

  const copyToClipboard = async (text: string) => {
    try {
      await navigator.clipboard.writeText(text);
      setCopied(text);
      setTimeout(() => setCopied(null), 2000);
    } catch (err) {
      console.error("Failed to copy:", err);
    }
  };

  const formatAddress = (addr: string) => {
    if (addr.length <= 20) return addr;
    return `${addr.slice(0, 10)}...${addr.slice(-10)}`;
  };

  return (
    <div className="min-h-screen bg-black text-white relative overflow-hidden">
      {/* Animated Grid Background */}
      <div className="fixed inset-0 pointer-events-none">
        <div 
          className="absolute inset-0 opacity-[0.15]"
          style={{
            backgroundImage: `
              linear-gradient(#FF6B35 1px, transparent 1px),
              linear-gradient(90deg, #FF6B35 1px, transparent 1px)
            `,
            backgroundSize: '60px 60px',
            animation: 'gridMove 20s linear infinite'
          }}
        />
      </div>

      {/* Header */}
      <header className="fixed top-6 left-1/2 -translate-x-1/2 z-50">
        <div className="bg-black/80 backdrop-blur-2xl border border-[#FF6B35]/20 rounded-2xl px-8 py-3.5 shadow-[0_0_30px_rgba(255,107,53,0.1)]">
          <nav className="flex items-center gap-8">
            <Link href="/" className="text-sm text-zinc-400 hover:text-[#FF6B35] transition-colors font-medium tracking-wide">Bridge</Link>
            <div className="w-px h-5 bg-[#FF6B35]/30" />
            <Link href="/wallet" className="text-sm text-[#FF6B35] font-medium tracking-wide">Wallet</Link>
          </nav>
        </div>
      </header>

      {/* Main Content */}
      <div className="relative z-10 pt-36 pb-24 px-4">
        <div className="max-w-4xl mx-auto">
          {/* Title */}
          <div className="text-center mb-12">
            <div className="inline-flex items-center gap-3 mb-4">
              <Wallet className="w-8 h-8 text-[#FF6B35]" />
              <h1 className="text-5xl font-bold tracking-tight bg-clip-text text-transparent" style={{ backgroundImage: 'linear-gradient(to right, #ffffff, #FF6B35)' }}>
                Wallet
              </h1>
            </div>
            <p className="text-zinc-500 text-base font-light tracking-wider mb-6">Personal Wallet Dashboard</p>
            
            {/* Wallet Type Toggle */}
            <div className="inline-flex bg-zinc-950/80 border border-[#FF6B35]/20 rounded-xl p-1">
              <button
                onClick={() => setWalletType("zcash")}
                className={`px-6 py-2 rounded-lg text-sm font-medium transition-all ${
                  walletType === "zcash"
                    ? "bg-[#FF6B35] text-black"
                    : "text-zinc-400 hover:text-white"
                }`}
              >
                Zcash
              </button>
              <button
                onClick={() => setWalletType("miden")}
                className={`px-6 py-2 rounded-lg text-sm font-medium transition-all ${
                  walletType === "miden"
                    ? "bg-[#FF6B35] text-black"
                    : "text-zinc-400 hover:text-white"
                }`}
              >
                Miden
              </button>
            </div>
          </div>

          {/* Zcash Wallet View */}
          {walletType === "zcash" && (
            <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
              {/* Balance Card */}
              <div className="bg-black/60 backdrop-blur-xl border border-[#FF6B35]/20 rounded-2xl p-6 shadow-[0_0_60px_rgba(255,107,53,0.15)]">
                <div className="flex items-center justify-between mb-4">
                  <h2 className="text-lg font-semibold text-zinc-300">Balance</h2>
                  <button
                    onClick={loadBalance}
                    disabled={loading}
                    className="p-2 hover:bg-[#FF6B35]/10 rounded-lg transition-colors"
                  >
                    <RefreshCw className={`w-4 h-4 text-[#FF6B35] ${loading ? 'animate-spin' : ''}`} />
                  </button>
                </div>
                <div className="text-4xl font-bold text-white mb-4">
                  {loading ? "..." : `${balance} TAZ`}
                </div>
                
                {/* Wallet Address */}
                <div className="mb-4 p-3 bg-zinc-950/80 rounded-xl border border-zinc-900">
                  <div className="text-xs text-zinc-500 mb-2 uppercase tracking-widest font-semibold">Wallet Address</div>
                  {addresses.length > 0 ? (
                    <div className="flex items-center gap-2">
                      <div className="flex-1 min-w-0">
                        <div className="text-sm font-mono text-zinc-300 break-all">
                          {formatAddress(addresses[0].address)}
                        </div>
                      </div>
                      <button
                        onClick={() => copyToClipboard(addresses[0].address)}
                        className="flex-shrink-0 p-2 hover:bg-[#FF6B35]/10 rounded-lg transition-colors"
                        title="Copy address"
                      >
                        {copied === addresses[0].address ? (
                          <Check className="w-4 h-4 text-green-400" />
                        ) : (
                          <Copy className="w-4 h-4 text-[#FF6B35]" />
                        )}
                      </button>
                    </div>
                  ) : (
                    <div className="text-sm text-zinc-500 italic">Loading address...</div>
                  )}
                </div>
                
                <button
                  onClick={handleSync}
                  disabled={syncing}
                  className="w-full py-2 bg-[#FF6B35]/10 hover:bg-[#FF6B35]/20 border border-[#FF6B35]/30 rounded-lg text-sm font-medium text-[#FF6B35] transition-all flex items-center justify-center gap-2"
                >
                  <RefreshCw className={`w-4 h-4 ${syncing ? 'animate-spin' : ''}`} />
                  {syncing ? "Syncing..." : "Sync Wallet"}
                </button>
              </div>

              {/* Send Card */}
              <div className="bg-black/60 backdrop-blur-xl border border-[#FF6B35]/20 rounded-2xl p-6 shadow-[0_0_60px_rgba(255,107,53,0.15)]">
                <h2 className="text-lg font-semibold text-zinc-300 mb-4">Send</h2>
                <form onSubmit={handleSend} className="space-y-4">
                  <div>
                    <label className="block text-xs text-zinc-400 mb-2 uppercase tracking-widest font-semibold">Address</label>
                    <input
                      type="text"
                      value={sendAddress}
                      onChange={(e) => setSendAddress(e.target.value)}
                      placeholder="Enter recipient address"
                      className="w-full px-4 py-3 bg-zinc-950/80 border border-zinc-900 rounded-xl text-sm focus:outline-none focus:border-[#FF6B35]/50 transition-all"
                      required
                    />
                  </div>
                  <div>
                    <label className="block text-xs text-zinc-400 mb-2 uppercase tracking-widest font-semibold">
                      Amount (TAZ)
                    </label>
                    <input
                      type="number"
                      step="0.00000001"
                      min="0"
                      value={sendAmount}
                      onChange={(e) => setSendAmount(e.target.value)}
                      placeholder="0.1"
                      className="w-full px-4 py-3 bg-zinc-950/80 border border-zinc-900 rounded-xl text-sm focus:outline-none focus:border-[#FF6B35]/50 transition-all"
                      required
                    />
                    {sendAmount && !isNaN(parseFloat(sendAmount)) && (
                      <div className="mt-1 text-xs text-zinc-500">
                        = {zecToZatoshis(sendAmount)} Zatoshis
                      </div>
                    )}
                  </div>
                  <div>
                    <label className="block text-xs text-zinc-400 mb-2 uppercase tracking-widest font-semibold">Memo (Optional)</label>
                    <input
                      type="text"
                      value={sendMemo}
                      onChange={(e) => setSendMemo(e.target.value)}
                      placeholder="Enter memo (max 512 chars)"
                      maxLength={512}
                      className="w-full px-4 py-3 bg-zinc-950/80 border border-zinc-900 rounded-xl text-sm focus:outline-none focus:border-[#FF6B35]/50 transition-all"
                    />
                  </div>
                  {sendResult && (
                    <div className={`p-3 rounded-lg text-sm ${sendResult.success ? 'bg-green-500/10 text-green-400' : 'bg-red-500/10 text-red-400'}`}>
                      {sendResult.message}
                    </div>
                  )}
                  <button
                    type="submit"
                    disabled={sending}
                    className="w-full py-3 bg-[#FF6B35] text-black font-bold rounded-xl hover:bg-[#FF6B35]/90 transition-all flex items-center justify-center gap-2 disabled:opacity-50"
                  >
                    <Send className="w-4 h-4" />
                    {sending ? "Sending..." : "Send"}
                  </button>
                </form>
              </div>

              {/* Transactions Card - Full Width */}
              <div className="lg:col-span-2 bg-black/60 backdrop-blur-xl border border-[#FF6B35]/20 rounded-2xl p-6 shadow-[0_0_60px_rgba(255,107,53,0.15)]">
                <div className="flex items-center justify-between mb-4">
                  <h2 className="text-lg font-semibold text-zinc-300">Transactions</h2>
                  <button
                    onClick={loadTransactions}
                    className="p-2 hover:bg-[#FF6B35]/10 rounded-lg transition-colors"
                  >
                    <RefreshCw className="w-4 h-4 text-[#FF6B35]" />
                  </button>
                </div>
                <div className="space-y-4 max-h-96 overflow-y-auto">
                  {transactions.length > 0 ? (
                    transactions.map((tx, idx) => (
                      <div key={idx} className="bg-zinc-950/80 rounded-xl p-4 border border-zinc-900">
                        <div className="flex items-start justify-between gap-3 mb-3">
                          <div className="flex-1 min-w-0">
                            <div className="flex items-center gap-2 mb-1">
                              <span className="text-xs font-mono text-zinc-400">
                                {formatAddress(tx.txid)}
                              </span>
                              <button
                                onClick={() => copyToClipboard(tx.txid)}
                                className="p-1 hover:bg-[#FF6B35]/10 rounded transition-colors"
                                title="Copy TXID"
                              >
                                {copied === tx.txid ? (
                                  <Check className="w-3 h-3 text-green-400" />
                                ) : (
                                  <Copy className="w-3 h-3 text-zinc-500" />
                                )}
                              </button>
                            </div>
                            <div className="flex items-center gap-3 text-xs text-zinc-500">
                              <span className={`px-2 py-0.5 rounded ${
                                tx.status === 'mined' ? 'bg-green-500/10 text-green-400' :
                                tx.status === 'unmined' ? 'bg-yellow-500/10 text-yellow-400' :
                                'bg-red-500/10 text-red-400'
                              }`}>
                                {tx.status.toUpperCase()}
                              </span>
                              {tx.date && <span>{tx.date}</span>}
                              {tx.height && <span>Block: {tx.height}</span>}
                            </div>
                          </div>
                          <div className="text-right">
                            <div className={`text-sm font-semibold ${
                              parseFloat(tx.amount) >= 0 ? 'text-green-400' : 'text-red-400'
                            }`}>
                              {parseFloat(tx.amount) >= 0 ? '+' : ''}{tx.amount} TAZ
                            </div>
                            <div className="text-xs text-zinc-500 mt-1">
                              {tx.sentNotes > 0 && tx.receivedNotes > 0 ? 'Sent & Received' :
                               tx.sentNotes > 0 ? 'Sent' : 'Received'}
                            </div>
                            {tx.fee !== 'Unknown' && (
                              <div className="text-xs text-zinc-500">Fee: {tx.fee} TAZ</div>
                            )}
                          </div>
                        </div>
                        {tx.outputs && tx.outputs.length > 0 && (
                          <div className="mt-3 pt-3 border-t border-zinc-800 space-y-2">
                            {tx.outputs.filter(o => !o.isChange).map((output, oIdx) => (
                              <div key={oIdx} className="text-xs text-zinc-400">
                                <div className="flex items-center gap-2 flex-wrap">
                                  <span className="text-[#FF6B35]">{output.pool}</span>
                                  {output.fromAccount && (
                                    <span className="text-red-400">From: {output.fromAccount}</span>
                                  )}
                                  {output.toAccount && (
                                    <span className="text-green-400">To: {output.toAccount}</span>
                                  )}
                                  {output.toAddress && !output.toAccount && (
                                    <>
                                      <span>→</span>
                                      <span className="font-mono">{formatAddress(output.toAddress)}</span>
                                    </>
                                  )}
                                  {output.value && (
                                    <span className="ml-auto text-zinc-300">{output.value} TAZ</span>
                                  )}
                                </div>
                                {output.memo && (
                                  <div className="mt-1 text-zinc-500 italic">Memo: {output.memo}</div>
                                )}
                              </div>
                            ))}
                          </div>
                        )}
                      </div>
                    ))
                  ) : (
                    <div className="text-center text-zinc-500 py-8">No transactions found</div>
                  )}
                </div>
              </div>
            </div>
          )}

          {/* Miden Wallet View */}
          {walletType === "miden" && (
            <div className="max-w-2xl mx-auto">
              <div className="bg-black/60 backdrop-blur-xl border border-[#FF6B35]/20 rounded-2xl p-6 shadow-[0_0_60px_rgba(255,107,53,0.15)]">
                {connecting && (
                  <div className="mb-6 p-4 bg-zinc-950/80 border border-zinc-900 rounded-xl">
                    <div className="flex items-center gap-3">
                      <Loader2 className="w-5 h-5 animate-spin text-[#FF6B35]" />
                      <div className="text-sm text-zinc-400">Setting up wallet...</div>
                    </div>
                  </div>
                )}
                
                {error && (
                  <div className="mb-6 p-4 bg-red-500/10 border border-red-500/30 rounded-xl">
                    <div className="text-sm text-red-400">{error}</div>
                  </div>
                )}
                
                {/* Show account ID if available (even if not fully connected) */}
                {accountId && (
                  <div className="mb-6 p-4 bg-[#FF6B35]/10 border border-[#FF6B35]/30 rounded-xl">
                    <div className="flex items-start justify-between gap-4">
                      <div className="flex-1">
                        <div className="text-xs text-zinc-400 mb-1 uppercase">Your Miden Account</div>
                        <div className="text-sm font-mono text-[#FF6B35] break-all">{accountId}</div>
                      </div>
                      <button
                        onClick={clearWallet}
                        className="px-3 py-1.5 text-xs bg-red-500/20 hover:bg-red-500/30 border border-red-500/30 text-red-400 rounded-lg"
                      >
                        Clear
                      </button>
                    </div>
                  </div>
                )}
                
                {connected && accountId && (
                  <>

                    {/* Balance Display */}
                    <div className="mb-6 p-4 bg-zinc-950/80 border border-zinc-900 rounded-xl">
                      <div className="flex items-center justify-between mb-2">
                        <div className="text-xs text-zinc-400 uppercase">wTAZ Balance</div>
                        <button
                          onClick={() => {
                            loadMidenBalance();
                            scanForNotes();
                          }}
                          disabled={loadingBalance || scanning}
                          className="p-1 hover:bg-[#FF6B35]/10 rounded transition-colors"
                        >
                          <RefreshCw className={`w-3 h-3 text-[#FF6B35] ${(loadingBalance || scanning) ? 'animate-spin' : ''}`} />
                        </button>
                      </div>
                      <div className="text-2xl font-bold text-white">
                        {loadingBalance ? "..." : `${midenBalance} wTAZ`}
                      </div>
                    </div>

                    {/* Notes Section */}
                    <div className="mb-6">
                      <div className="flex items-center justify-between mb-3">
                        <div className="text-xs text-zinc-400 uppercase">Consumable Notes</div>
                        <div className="flex items-center gap-2">
                          {scanning && (
                            <div className="flex items-center gap-2 text-xs text-zinc-500">
                              <Loader2 className="w-3 h-3 animate-spin" />
                              Scanning...
                            </div>
                          )}
                          <button
                            onClick={() => scanForNotes()}
                            disabled={scanning}
                            className="px-3 py-1.5 text-xs bg-[#FF6B35]/20 hover:bg-[#FF6B35]/30 border border-[#FF6B35]/30 text-[#FF6B35] rounded-lg disabled:opacity-50"
                          >
                            {scanning ? "Scanning..." : "Scan Notes"}
                          </button>
                        </div>
                      </div>
                      
                      {/* Upload .mno File Section */}
                      <div className="mb-4 p-4 bg-zinc-950/80 border border-zinc-900 rounded-xl">
                        <div className="flex items-center gap-2 mb-2">
                          <FileText className="w-4 h-4 text-[#FF6B35]" />
                          <div className="text-xs text-zinc-400 uppercase">Upload .mno File</div>
                        </div>
                        <p className="text-xs text-zinc-500 mb-3">
                          Upload a .mno file to consume a P2ID note (e.g., from bridge deposits)
                        </p>
                        <label className="block">
                          <input
                            type="file"
                            accept=".mno"
                            onChange={handleMnoFileUpload}
                            disabled={uploadingMno || !connected}
                            className="hidden"
                            id="mno-file-input"
                          />
                          <button
                            onClick={() => document.getElementById('mno-file-input')?.click()}
                            disabled={uploadingMno || !connected}
                            className="w-full px-4 py-2 bg-[#FF6B35]/10 hover:bg-[#FF6B35]/20 border border-[#FF6B35]/30 text-[#FF6B35] rounded-lg disabled:opacity-50 disabled:cursor-not-allowed flex items-center justify-center gap-2 transition-all"
                          >
                            {uploadingMno ? (
                              <>
                                <Loader2 className="w-4 h-4 animate-spin" />
                                Consuming...
                              </>
                            ) : (
                              <>
                                <Upload className="w-4 h-4" />
                                Choose .mno File
                              </>
                            )}
                          </button>
                        </label>
                        {mnoFile && (
                          <div className="mt-2 text-xs text-zinc-400">
                            Selected: {mnoFile.name}
                          </div>
                        )}
                      </div>
                      {notes.length > 0 ? (
                        <div className="space-y-3">
                          {notes.map((note, idx) => (
                            <div key={idx} className="p-4 bg-zinc-950/80 border border-zinc-900 rounded-xl flex justify-between items-center">
                              <div className="flex-1 min-w-0">
                                <div className="text-xs text-zinc-500 mb-1">Note ID</div>
                                <div className="text-sm font-mono text-zinc-300 truncate">{note.id}</div>
                              </div>
                              <button
                                onClick={() => consumeNote(note.id)}
                                disabled={consuming}
                                className="ml-4 px-4 py-2 bg-[#FF6B35] text-black font-bold rounded-lg hover:bg-[#FF6B35]/90 disabled:opacity-50 whitespace-nowrap"
                              >
                                {consuming ? "Consuming..." : "Consume"}
                              </button>
                            </div>
                          ))}
                        </div>
                      ) : (
                        <div className="p-4 bg-zinc-950/80 border border-zinc-900 rounded-xl text-center text-sm text-zinc-500">
                          {scanning ? "Scanning for notes..." : "No consumable notes found"}
                        </div>
                      )}
                    </div>

                    {success && (
                      <div className="mb-6 p-4 bg-green-500/10 border border-green-500/30 rounded-xl">
                        <div className="flex items-center gap-2">
                          <div className="text-sm text-green-400">{success}</div>
                          <button
                            onClick={() => setSuccess(null)}
                            className="ml-auto text-green-400 hover:text-green-300"
                          >
                            ×
                          </button>
                        </div>
                      </div>
                    )}
                    {error && (
                      <div className="mb-6 p-4 bg-red-500/10 border border-red-500/30 rounded-xl">
                        <div className="flex items-center gap-2">
                          <div className="text-sm text-red-400">{error}</div>
                          <button
                            onClick={() => setError("")}
                            className="ml-auto text-red-400 hover:text-red-300"
                          >
                            ×
                          </button>
                        </div>
                      </div>
                    )}
                  </>
                )}

                <div className="flex items-center justify-center gap-2.5 text-xs text-zinc-500 pt-6 mt-6 border-t border-zinc-900">
                  <Lock className="w-3.5 h-3.5 text-[#FF6B35]/60" />
                  <span>Private • Zero-knowledge • Non-custodial</span>
                </div>
              </div>
            </div>
          )}
        </div>
      </div>

      <style jsx>{`
        @keyframes gridMove {
          0% { transform: translate(0, 0); }
          100% { transform: translate(60px, 60px); }
        }
      `}</style>
    </div>
  );
}
