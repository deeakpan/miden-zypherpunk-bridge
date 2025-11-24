"use client";

import { useState, useEffect, useCallback } from "react";
import { Wallet, Send, RefreshCw, Copy, Check, ExternalLink, Search, Loader2, Lock } from "lucide-react";
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
  const [secret, setSecret] = useState("");
  const [client, setClient] = useState<any>(null);
  const [connected, setConnected] = useState(false);
  const [accountId, setAccountId] = useState("");
  const [scanning, setScanning] = useState(false);
  const [notes, setNotes] = useState<any[]>([]);
  const [consuming, setConsuming] = useState(false);
  const [error, setError] = useState("");
  const [connecting, setConnecting] = useState(false);

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
      
      const storedAccountId = localStorage.getItem("miden_account_id");
      
      if (storedAccountId) {
        setAccountId(storedAccountId);
        setConnected(true);
        setConnecting(false);
        return;
      }
      
      await client.syncState();
      const account = await client.newWallet(AccountStorageMode.private(), true, 0);
      
      const accountIdHex = account.id().toString();
      const hexOnly = accountIdHex.startsWith('0x') ? accountIdHex.slice(2) : accountIdHex;
      const accountIdBech32 = (account.id() as any).toBech32?.(NetworkId.Testnet) || accountIdHex;
      
      localStorage.setItem("miden_account_id", accountIdBech32);
      localStorage.setItem("miden_account_id_hex", hexOnly);
      
      setAccountId(accountIdBech32);
      setConnected(true);
    } catch (err: any) {
      console.error("Failed to setup wallet:", err);
      setError(`Failed to setup wallet: ${err.message || String(err)}`);
    } finally {
      setConnecting(false);
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

  const scanForNotes = async () => {
    if (!client || !accountId || !secret) {
      setError("Please connect wallet and enter secret");
      return;
    }

    try {
      setError("");
      setScanning(true);
      
      const { AccountId } = await import("@demox-labs/miden-sdk");
      
      let account;
      if (accountId.startsWith('0x')) {
        const hexStr = accountId.slice(2);
        account = AccountId.fromHex(hexStr);
      } else {
        account = AccountId.fromHex(accountId);
      }
      
      await client.syncState();
      const consumableNotes = await client.getConsumableNotes(account);
      
      const matchingNotes = [];
      for (const note of consumableNotes) {
        const noteRecord = note.inputNoteRecord();
        const noteId = noteRecord.id().toString();
        matchingNotes.push({ id: noteId, note: noteRecord });
      }
      
      setNotes(matchingNotes);
      
      if (matchingNotes.length === 0) {
        setError("No consumable notes found.");
      }
    } catch (err: any) {
      setError(`Failed to scan notes: ${err.message}`);
      console.error("Scan error:", err);
    } finally {
      setScanning(false);
    }
  };

  const consumeNote = async (noteId: string) => {
    if (!client || !accountId) {
      setError("Please connect wallet");
      return;
    }

    try {
      setError("");
      setConsuming(true);
      
      const { AccountId } = await import("@demox-labs/miden-sdk");
      
      let account;
      if (accountId.startsWith('0x')) {
        const hexStr = accountId.slice(2);
        account = AccountId.fromHex(hexStr);
      } else {
        account = AccountId.fromHex(accountId);
      }
      
      const consumeTxRequest = client.newConsumeTransactionRequest([noteId]);
      const consumeTx = await client.newTransaction(account, consumeTxRequest);
      await client.submitTransaction(consumeTx);
      
      await new Promise((resolve) => setTimeout(resolve, 5000));
      await client.syncState();
      
      alert("Note consumed successfully!");
      await scanForNotes();
    } catch (err: any) {
      setError(`Failed to consume note: ${err.message}`);
      console.error("Consume error:", err);
    } finally {
      setConsuming(false);
    }
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
                
                {connected && accountId && (
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

                <div className="mb-6">
                  <label className="block text-xs text-zinc-400 mb-2 uppercase">Secret</label>
                  <input
                    type="text"
                    value={secret}
                    onChange={(e) => setSecret(e.target.value)}
                    placeholder="Paste your secret here"
                    className="w-full px-5 py-4 bg-zinc-950/80 border border-zinc-900 rounded-xl text-sm focus:outline-none focus:border-[#FF6B35]/50 font-mono"
                  />
                </div>

                <button
                  onClick={scanForNotes}
                  disabled={!connected || !secret || scanning}
                  className="w-full py-4 bg-[#FF6B35] text-black font-bold rounded-xl hover:bg-[#FF6B35]/90 disabled:opacity-50 flex items-center justify-center gap-2"
                >
                  {scanning ? <><Loader2 className="w-4 h-4 animate-spin" />Scanning...</> : <><Search className="w-4 h-4" />Scan for Notes</>}
                </button>

                {error && (
                  <div className="mt-4 p-4 bg-red-500/10 border border-red-500/30 rounded-xl">
                    <div className="text-sm text-red-400">{error}</div>
                  </div>
                )}

                {notes.length > 0 && (
                  <div className="mt-6">
                    <div className="text-sm text-zinc-400 mb-3">Found {notes.length} Note(s)</div>
                    <div className="space-y-3">
                      {notes.map((note, idx) => (
                        <div key={idx} className="p-4 bg-zinc-950/80 border border-zinc-900 rounded-xl flex justify-between">
                          <div>
                            <div className="text-xs text-zinc-500">Note ID</div>
                            <div className="text-sm font-mono text-zinc-300">{note.id.slice(0, 20)}...</div>
                          </div>
                          <button
                            onClick={() => consumeNote(note.id)}
                            disabled={consuming}
                            className="px-4 py-2 bg-[#FF6B35] text-black font-bold rounded-lg hover:bg-[#FF6B35]/90 disabled:opacity-50"
                          >
                            {consuming ? "Consuming..." : "Consume"}
                          </button>
                        </div>
                      ))}
                    </div>
                  </div>
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
