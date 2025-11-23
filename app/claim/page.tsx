"use client";

import { Lock, Copy, Check, Search, Loader2 } from "lucide-react";
import { useState, useEffect } from "react";
import Link from "next/link";
import Image from "next/image";

export default function ClaimPage() {
  const [secret, setSecret] = useState("");
  const [client, setClient] = useState<any>(null);
  const [connected, setConnected] = useState(false);
  const [accountId, setAccountId] = useState("");
  const [scanning, setScanning] = useState(false);
  const [notes, setNotes] = useState<any[]>([]);
  const [consuming, setConsuming] = useState(false);
  const [copied, setCopied] = useState(false);
  const [error, setError] = useState("");
  const [connecting, setConnecting] = useState(false);

  // Initialize Miden WebClient and automatically create/load wallet
  useEffect(() => {
    if (typeof window === "undefined") return;
    
    async function setupClient() {
      try {
        setConnecting(true);
        setError("");
        
        // Dynamically import WebClient to avoid SSR issues
        const { WebClient, AccountStorageMode, NetworkId } = await import("@demox-labs/miden-sdk");
        
        const client = await WebClient.createClient("https://rpc.testnet.miden.io");
        setClient(client);
        
        // Check localStorage for existing account ID
        const storedAccountId = localStorage.getItem("miden_account_id");
        
        if (storedAccountId) {
          setAccountId(storedAccountId);
          setConnected(true);
          setConnecting(false);
          return;
        }
        
        // No existing account, create a new wallet automatically
        await client.syncState();
        
        const account = await client.newWallet(AccountStorageMode.private(), true, 0);
        
        // Get hex format for backend - toString() returns hex like "0xb455b7fe7496199022fd85dda901b5"
        const accountIdHex = account.id().toString();
        const hexOnly = accountIdHex.startsWith('0x') ? accountIdHex.slice(2) : accountIdHex;
        
        console.log("Hex format:", hexOnly);
        
        // Get bech32 format for display
        const accountIdBech32 = (account.id() as any).toBech32?.(NetworkId.Testnet) || accountIdHex;
        
        console.log("Bech32 format:", accountIdBech32);
        
        // Store both formats
        localStorage.setItem("miden_account_id", accountIdBech32); // Display in UI
        localStorage.setItem("miden_account_id_hex", hexOnly); // Send to backend
        
        setAccountId(accountIdBech32); // Show bech32 in UI
        setConnected(true);
      } catch (err: any) {
        console.error("Failed to setup wallet:", err);
        setError(`Failed to setup wallet: ${err.message || String(err)}`);
      } finally {
        setConnecting(false);
      }
    }
    setupClient();
  }, []);

  const scanForNotes = async () => {
    if (!client || !accountId || !secret) {
      setError("Please connect wallet and enter secret");
      return;
    }

    try {
      setError("");
      setScanning(true);
      
      const { AccountId } = await import("@demox-labs/miden-sdk");
      
      // Parse account ID
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

  const copyToClipboard = async (text: string) => {
    try {
      await navigator.clipboard.writeText(text);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch (err) {
      console.error("Failed to copy:", err);
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

  return (
    <div className="min-h-screen bg-black text-white relative overflow-hidden">
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

      <header className="fixed top-6 left-1/2 -translate-x-1/2 z-50">
        <div className="bg-black/80 backdrop-blur-2xl border border-[#FF6B35]/20 rounded-2xl px-8 py-3.5">
          <nav className="flex items-center gap-8">
            <a href="#" className="text-sm text-zinc-400 hover:text-[#FF6B35] transition-colors">Docs</a>
            <div className="w-px h-5 bg-[#FF6B35]/30" />
            <Link href="/" className="text-sm text-zinc-400 hover:text-[#FF6B35] transition-colors">Bridge</Link>
            <div className="w-px h-5 bg-[#FF6B35]/30" />
            <Link href="/wallet" className="text-sm text-zinc-400 hover:text-[#FF6B35] transition-colors">Wallet</Link>
            <div className="w-px h-5 bg-[#FF6B35]/30" />
            <Link href="/claim" className="text-sm text-[#FF6B35]">Claim</Link>
          </nav>
        </div>
      </header>

      <div className="relative z-10 pt-36 pb-24 px-4">
        <div className="max-w-2xl mx-auto">
          <div className="text-center mb-12">
            <h1 className="text-5xl font-bold mb-4 bg-clip-text text-transparent" style={{ backgroundImage: 'linear-gradient(to right, #ffffff, #FF6B35)' }}>
              Claim Deposit
            </h1>
            <p className="text-zinc-500">Enter your secret to claim your wTAZ tokens</p>
          </div>

          <div className="bg-black/60 backdrop-blur-xl border border-[#FF6B35]/20 rounded-2xl p-6">
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