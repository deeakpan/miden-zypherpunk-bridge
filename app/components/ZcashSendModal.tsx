"use client";

import { useState, useEffect } from "react";
import { X, Send, Download, Copy, Check, Loader2, Wallet } from "lucide-react";

interface ZcashSendModalProps {
  isOpen: boolean;
  onClose: () => void;
  bridgeAddress: string;
  memo: string;
  secret: string;
}

export default function ZcashSendModal({ isOpen, onClose, bridgeAddress, memo, secret }: ZcashSendModalProps) {
  const [amount, setAmount] = useState("");
  const [balance, setBalance] = useState<string>("0");
  const [loadingBalance, setLoadingBalance] = useState(false);
  const [sending, setSending] = useState(false);
  const [sendResult, setSendResult] = useState<{ success: boolean; message: string } | null>(null);
  const [copied, setCopied] = useState(false);

  useEffect(() => {
    if (isOpen) {
      loadBalance();
    }
  }, [isOpen]);

  const loadBalance = async () => {
    setLoadingBalance(true);
    try {
      const res = await fetch("/api/wallet/balance");
      const data = await res.json();
      if (data.success && data.balance) {
        setBalance(data.balance.total || "0");
      }
    } catch (error) {
      console.error("Failed to load balance:", error);
    } finally {
      setLoadingBalance(false);
    }
  };

  const handleAmountChange = (value: string) => {
    // Only allow numbers and one decimal point
    const cleaned = value.replace(/[^0-9.]/g, '');
    // Ensure only one decimal point
    const parts = cleaned.split('.');
    if (parts.length > 2) {
      setAmount(parts[0] + '.' + parts.slice(1).join(''));
    } else {
      setAmount(cleaned);
    }
  };

  const handleSend = async () => {
    if (!amount || amount.trim() === '') {
      alert("Please enter an amount");
      return;
    }

    // Clean and validate amount
    const cleanedAmount = amount.trim();
    const amountNum = parseFloat(cleanedAmount);
    const balanceNum = parseFloat(balance);
    
    if (isNaN(amountNum) || amountNum <= 0) {
      alert("Please enter a valid amount (must be a positive number)");
      setAmount('');
      return;
    }

    if (amountNum > balanceNum) {
      alert(`Insufficient balance. You have ${balance} TAZ`);
      return;
    }

    // Update amount to cleaned version
    setAmount(cleanedAmount);

    setSending(true);
    setSendResult(null);
    try {
      const res = await fetch("/api/wallet/send", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          address: bridgeAddress,
          amount: amount,
          memo: memo,
        }),
      });

      const data = await res.json();
      if (data.success) {
        setSendResult({ success: true, message: `Transaction sent! TXID: ${data.txid || "pending"}` });
      } else {
        setSendResult({ success: false, message: data.error || "Failed to send transaction" });
      }
    } catch (error: any) {
      setSendResult({ success: false, message: error.message || "Failed to send transaction" });
    } finally {
      setSending(false);
    }
  };

  const downloadSecret = () => {
    const blob = new Blob([secret], { type: 'text/plain' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = 'secret.txt';
    a.click();
    URL.revokeObjectURL(url);
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

  if (!isOpen) return null;

  return (
    <div className="fixed inset-0 bg-black/80 backdrop-blur-sm z-50 flex items-center justify-center p-4 overflow-y-auto">
      <div className="bg-black/90 border border-[#FF6B35]/30 rounded-2xl p-6 max-w-lg w-full max-h-[90vh] overflow-y-auto my-auto scrollbar-hide">
        {/* Header */}
        <div className="flex items-center justify-between mb-4">
          <h2 className="text-xl font-bold text-[#FF6B35]">Send Deposit</h2>
          <button
            onClick={onClose}
            className="p-1.5 hover:bg-[#FF6B35]/10 rounded-lg transition-colors"
          >
            <X className="w-4 h-4 text-zinc-400" />
          </button>
        </div>

        {/* Balance Display */}
        <div className="mb-4 p-3 bg-zinc-950/80 border border-zinc-900 rounded-xl">
          <div className="flex items-center justify-between">
            <div className="flex items-center gap-2">
              <Wallet className="w-4 h-4 text-zinc-400" />
              <span className="text-xs text-zinc-400 uppercase tracking-widest">Balance</span>
            </div>
            {loadingBalance ? (
              <Loader2 className="w-4 h-4 animate-spin text-zinc-400" />
            ) : (
              <span className="text-lg font-bold text-white">{balance} TAZ</span>
            )}
          </div>
        </div>

        {/* Secret Download - Compact */}
        <div className="mb-4 p-3 bg-zinc-950/80 border border-[#FF6B35]/20 rounded-xl">
          <button
            onClick={downloadSecret}
            className="w-full flex items-center justify-center gap-2 px-3 py-2 bg-[#FF6B35] text-black font-semibold text-sm rounded-lg hover:bg-[#FF6B35]/90 transition-colors"
          >
            <Download className="w-4 h-4" />
            Download Secret
          </button>
        </div>

        {/* Bridge Address - Compact */}
        <div className="mb-3">
          <label className="block text-xs text-zinc-400 mb-1.5 uppercase tracking-widest font-semibold">
            Bridge Address
          </label>
          <div className="relative">
            <input
              type="text"
              value={bridgeAddress.length > 30 ? `${bridgeAddress.slice(0, 15)}...${bridgeAddress.slice(-15)}` : bridgeAddress}
              readOnly
              className="w-full px-3 py-2.5 pr-10 bg-zinc-950/80 border border-[#FF6B35]/30 rounded-lg text-xs font-mono text-zinc-300"
              title={bridgeAddress}
            />
            <button
              onClick={() => copyToClipboard(bridgeAddress)}
              className="absolute right-2 top-1/2 -translate-y-1/2 p-1.5 hover:bg-[#FF6B35]/10 rounded transition-colors"
            >
              {copied ? (
                <Check className="w-3 h-3 text-[#FF6B35]" />
              ) : (
                <Copy className="w-3 h-3 text-zinc-400" />
              )}
            </button>
          </div>
        </div>

        {/* Memo - Compact */}
        <div className="mb-3">
          <label className="block text-xs text-zinc-400 mb-1.5 uppercase tracking-widest font-semibold">
            Memo
          </label>
          <div className="relative">
            <input
              type="text"
              value={memo.length > 30 ? `${memo.slice(0, 15)}...${memo.slice(-15)}` : memo}
              readOnly
              className="w-full px-3 py-2.5 pr-10 bg-zinc-950/80 border border-[#FF6B35]/30 rounded-lg text-xs font-mono text-zinc-300"
              title={memo}
            />
            <button
              onClick={() => copyToClipboard(memo)}
              className="absolute right-2 top-1/2 -translate-y-1/2 p-1.5 hover:bg-[#FF6B35]/10 rounded transition-colors"
            >
              {copied ? (
                <Check className="w-3 h-3 text-[#FF6B35]" />
              ) : (
                <Copy className="w-3 h-3 text-zinc-400" />
              )}
            </button>
          </div>
        </div>

        {/* Amount */}
        <div className="mb-4">
          <label className="block text-xs text-zinc-400 mb-1.5 uppercase tracking-widest font-semibold">
            Amount (TAZ)
          </label>
          <input
            type="text"
            inputMode="decimal"
            value={amount}
            onChange={(e) => handleAmountChange(e.target.value)}
            placeholder="0.00"
            className={`w-full px-4 py-3 bg-zinc-950/80 border rounded-xl text-xl font-bold focus:outline-none focus:ring-2 transition-all placeholder-zinc-700 ${
              amount && parseFloat(amount) > parseFloat(balance) 
                ? "border-red-500/50 focus:border-red-500/70 focus:ring-red-500/20" 
                : "border-zinc-900 focus:border-[#FF6B35]/50 focus:ring-[#FF6B35]/20"
            }`}
          />
          {amount && parseFloat(amount) > parseFloat(balance) && (
            <p className="mt-1.5 text-xs text-red-400">Amount exceeds balance</p>
          )}
        </div>

        {/* Send Button */}
        <button
          onClick={handleSend}
          disabled={sending || !amount || (amount && parseFloat(amount) > parseFloat(balance))}
          className="w-full py-3 bg-[#FF6B35] text-black font-bold text-sm rounded-xl hover:bg-[#FF6B35]/90 active:scale-[0.98] transition-all shadow-[0_0_40px_rgba(255,107,53,0.4)] disabled:opacity-50 disabled:cursor-not-allowed flex items-center justify-center gap-2"
        >
          {sending ? (
            <>
              <Loader2 className="w-4 h-4 animate-spin" />
              Sending...
            </>
          ) : (
            <>
              <Send className="w-4 h-4" />
              Send Transaction
            </>
          )}
        </button>

        {/* Result Message */}
        {sendResult && (
          <div className={`mt-3 p-3 rounded-lg ${sendResult.success ? 'bg-green-500/10 border border-green-500/30' : 'bg-red-500/10 border border-red-500/30'}`}>
            <p className={`text-xs ${sendResult.success ? 'text-green-400' : 'text-red-400'}`}>
              {sendResult.message}
            </p>
          </div>
        )}
      </div>
    </div>
  );
}

