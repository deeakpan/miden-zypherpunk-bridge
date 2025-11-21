"use client";

import { useState, useEffect } from "react";
import { Wallet, Send, RefreshCw, Copy, Check, ExternalLink } from "lucide-react";
import Link from "next/link";
import { parseTransactions, parseAddresses, zatoshisToZec, zecToZatoshis, ParsedTransaction, ParsedAddress } from "@/lib/parse-wallet";

export default function WalletPage() {
  const [balance, setBalance] = useState<string>("0");
  const [loading, setLoading] = useState(false);
  const [syncing, setSyncing] = useState(false);
  const [addresses, setAddresses] = useState<ParsedAddress[]>([]);
  const [transactions, setTransactions] = useState<ParsedTransaction[]>([]);
  const [copied, setCopied] = useState<string | null>(null);

  // Send form state
  const [sendAddress, setSendAddress] = useState("");
  const [sendAmount, setSendAmount] = useState(""); // Now in ZEC
  const [sendMemo, setSendMemo] = useState("");
  const [sending, setSending] = useState(false);
  const [sendResult, setSendResult] = useState<{ success: boolean; message: string } | null>(null);

  useEffect(() => {
    loadBalance();
    loadAddresses();
    loadTransactions();
  }, []);

  const loadBalance = async () => {
    setLoading(true);
    try {
      const res = await fetch("/api/wallet/balance");
      const data = await res.json();
      if (data.success && data.balance) {
        setBalance(data.balance.total || "0");
      }
    } catch (error) {
      console.error("Failed to load balance:", error);
    } finally {
      setLoading(false);
    }
  };

  const loadAddresses = async () => {
    try {
      const res = await fetch("/api/wallet/addresses");
      const data = await res.json();
      if (data.success) {
        const parsed = parseAddresses(data.raw);
        console.log("Parsed addresses:", parsed);
        console.log("Raw output:", data.raw);
        setAddresses(parsed);
      } else {
        console.error("Failed to load addresses:", data.error);
      }
    } catch (error) {
      console.error("Failed to load addresses:", error);
    }
  };

  const loadTransactions = async () => {
    try {
      const res = await fetch("/api/wallet/transactions");
      const data = await res.json();
      if (data.success) {
        const parsed = parseTransactions(data.raw);
        setTransactions(parsed);
      }
    } catch (error) {
      console.error("Failed to load transactions:", error);
    }
  };

  const handleSync = async () => {
    setSyncing(true);
    try {
      const res = await fetch("/api/wallet/sync", { method: "POST" });
      const data = await res.json();
      if (data.success) {
        // Reload balance after sync
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
      // Convert ZEC to Zatoshis
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
        // Reload balance and transactions
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
            <div className="w-px h-5 bg-[#FF6B35]/30" />
            <a href="#" className="text-sm text-zinc-400 hover:text-[#FF6B35] transition-colors font-medium tracking-wide">Transactions</a>
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
            <p className="text-zinc-500 text-base font-light tracking-wider">Personal Wallet Dashboard</p>
          </div>

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

