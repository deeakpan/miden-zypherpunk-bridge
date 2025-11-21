"use client";

import { Lock, ArrowUpDown, Sparkles, Copy, Check } from "lucide-react";
import { useState } from "react";
import Image from "next/image";
import Link from "next/link";

export default function App() {
  const [fromChain, setFromChain] = useState("Zcash");
  const [toChain, setToChain] = useState("Miden");
  const [amount, setAmount] = useState("");
  const [address, setAddress] = useState("");
  const [copied, setCopied] = useState(false);

  const midenDepositAddress = "utest1s7vrs7ycxvpu379zvtxt0fnc0efseur2f8g2s8puqls7nk45l6p7wvglu3rph9us9qzsjww44ly3wxlsul0jcpqx8qwvwqz4sq48rjj0cn59956sjsrz5ufuswd5ujy89n3vh264wx3843pxscnrf0ulku4990h65h5ll9r0j3q82mjgm2sx7lfnrkfkuqw9l2m7yfmgc4jvzq6n8j2";

  const copyToClipboard = async () => {
    try {
      await navigator.clipboard.writeText(midenDepositAddress);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch (err) {
      console.error("Failed to copy:", err);
    }
  };

  const formatAddress = (addr: string) => {
    if (addr.length <= 20) return addr;
    return `${addr.slice(0, 10)}...${addr.slice(-10)}`;
  };

  const swapChains = () => {
    const temp = fromChain;
    setFromChain(toChain);
    setToChain(temp);
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
        
        {/* Animated scanning lines */}
        <div className="absolute inset-0">
          <div className="absolute top-0 left-0 w-full h-[2px] bg-[#FF6B35] opacity-30 animate-[scan_8s_linear_infinite]" />
          <div className="absolute top-1/3 left-0 w-full h-[1px] bg-[#FF6B35] opacity-20 animate-[scan_12s_linear_infinite]" />
          <div className="absolute top-2/3 left-0 w-full h-[1px] bg-[#FF6B35] opacity-20 animate-[scan_15s_linear_infinite]" />
        </div>

        {/* Glowing orbs */}
        <div className="absolute top-1/4 left-1/4 w-96 h-96 bg-[#FF6B35] rounded-full blur-[120px] opacity-[0.08] animate-[pulse_4s_ease-in-out_infinite]" />
        <div className="absolute bottom-1/4 right-1/4 w-96 h-96 bg-[#FF6B35] rounded-full blur-[120px] opacity-[0.06] animate-[pulse_5s_ease-in-out_infinite]" />
      </div>

      {/* Floating Header */}
      <header className="fixed top-6 left-1/2 -translate-x-1/2 z-50">
        <div className="bg-black/80 backdrop-blur-2xl border border-[#FF6B35]/20 rounded-2xl px-8 py-3.5 shadow-[0_0_30px_rgba(255,107,53,0.1)]">
          <nav className="flex items-center gap-8">
            <a href="#" className="text-sm text-zinc-400 hover:text-[#FF6B35] transition-colors font-medium tracking-wide">Docs</a>
            <div className="w-px h-5 bg-[#FF6B35]/30" />
            <Link href="/" className="text-sm text-[#FF6B35] font-medium tracking-wide">Bridge</Link>
            <div className="w-px h-5 bg-[#FF6B35]/30" />
            <Link href="/wallet" className="text-sm text-zinc-400 hover:text-[#FF6B35] transition-colors font-medium tracking-wide">Wallet</Link>
            <div className="w-px h-5 bg-[#FF6B35]/30" />
            <a href="#" className="text-sm text-zinc-400 hover:text-[#FF6B35] transition-colors font-medium tracking-wide">Transactions</a>
          </nav>
        </div>
      </header>

      {/* Main Content */}
      <div className="relative z-10 pt-36 pb-24 px-4">
        <div className="max-w-xl mx-auto">
          {/* Title */}
          <div className="text-center mb-16">
            <div className="inline-flex items-center gap-3 mb-4">
              <div className="w-1.5 h-1.5 bg-[#FF6B35] rounded-full animate-pulse" />
              <h1 className="text-6xl font-bold tracking-tight bg-clip-text text-transparent" style={{ backgroundImage: 'linear-gradient(to right, #ffffff, #FF6B35)' }}>
                RAVEN
              </h1>
              <div className="w-1.5 h-1.5 bg-[#FF6B35] rounded-full animate-pulse" />
            </div>
            <p className="text-zinc-500 text-base font-light tracking-wider">Private Cross-Chain Bridge</p>
          </div>

          {/* Bridge Card */}
          <div className="bg-black/60 backdrop-blur-xl border border-[#FF6B35]/20 rounded-2xl p-6 shadow-[0_0_60px_rgba(255,107,53,0.15)]">
            {/* Chains with Swap */}
            <div className="mb-6">
              <div className="flex items-center gap-3">
                <div className="flex-1 group relative bg-zinc-950/80 border border-zinc-900 rounded-xl p-4 hover:border-[#FF6B35]/30 transition-all duration-300 cursor-pointer">
                  <div className="absolute inset-0 bg-[#FF6B35]/5 rounded-xl opacity-0 group-hover:opacity-100 transition-opacity" />
                  <div className="relative">
                    <div className="text-xs text-zinc-500 mb-3 uppercase tracking-widest font-semibold">From</div>
                    <div className="flex items-center gap-3">
                      <div className="w-10 h-10 bg-zinc-900 border border-zinc-800 rounded-lg flex items-center justify-center group-hover:border-[#FF6B35]/50 transition-colors overflow-hidden">
                        <Image 
                          src="/zcash-logo.jpg" 
                          alt="Zcash" 
                          width={40} 
                          height={40} 
                          className="object-contain"
                        />
                      </div>
                      <div>
                        <div className="font-bold text-lg text-white">{fromChain}</div>
                        <div className="text-xs text-zinc-500 font-medium">Testnet</div>
                      </div>
                    </div>
                  </div>
                </div>

                {/* Swap Button */}
                <button
                  onClick={swapChains}
                  className="flex-shrink-0 w-10 h-10 bg-black border-2 border-[#FF6B35]/40 rounded-lg flex items-center justify-center hover:bg-[#FF6B35]/10 hover:border-[#FF6B35] transition-all shadow-[0_0_20px_rgba(255,107,53,0.3)] group"
                >
                  <ArrowUpDown className="w-4 h-4 text-[#FF6B35] group-hover:rotate-180 transition-transform duration-300" />
                </button>

                <div className="flex-1 group relative bg-zinc-950/80 border border-zinc-900 rounded-xl p-4 hover:border-[#FF6B35]/30 transition-all duration-300 cursor-pointer">
                  <div className="absolute inset-0 bg-[#FF6B35]/5 rounded-xl opacity-0 group-hover:opacity-100 transition-opacity" />
                  <div className="relative">
                    <div className="text-xs text-zinc-500 mb-3 uppercase tracking-widest font-semibold">To</div>
                    <div className="flex items-center gap-3">
                      <div className="w-10 h-10 bg-[#FF6B35]/10 border border-[#FF6B35]/30 rounded-lg flex items-center justify-center overflow-hidden">
                        <Image 
                          src="/miden-logo.jpg" 
                          alt="Miden" 
                          width={40} 
                          height={40} 
                          className="object-contain"
                        />
                      </div>
                      <div>
                        <div className="font-bold text-lg text-white">{toChain}</div>
                        <div className="text-xs text-zinc-500 font-medium">Testnet</div>
                      </div>
                    </div>
                  </div>
                </div>
              </div>
            </div>

            {/* Amount */}
            <div className="mb-5">
              <label className="block text-xs text-zinc-400 mb-2 uppercase tracking-widest font-semibold">Amount</label>
              <div className="relative group">
                <input
                  type="text"
                  value={amount}
                  onChange={(e) => setAmount(e.target.value)}
                  placeholder="0.00"
                  className="w-full px-5 py-4 bg-zinc-950/80 border border-zinc-900 rounded-xl text-2xl font-bold focus:outline-none focus:border-[#FF6B35]/50 focus:ring-2 focus:ring-[#FF6B35]/20 transition-all placeholder-zinc-700"
                />
                <span className="absolute right-5 top-1/2 -translate-y-1/2 text-sm text-zinc-500 font-semibold">
                  {fromChain === "Zcash" ? "TAZ" : "wTAZ"}
                </span>
                <div className="absolute inset-0 border-2 border-[#FF6B35]/0 rounded-xl group-focus-within:border-[#FF6B35]/30 transition-all pointer-events-none" />
              </div>
            </div>

            {/* Deposit Address Display (Zcash to Miden) */}
            {fromChain === "Zcash" && toChain === "Miden" && (
              <div className="mb-6">
                <label className="block text-xs text-zinc-400 mb-2 uppercase tracking-widest font-semibold">Deposit Address</label>
                <div className="relative group">
                  <div className="w-full px-5 py-4 pr-14 bg-zinc-950/80 border border-[#FF6B35]/30 rounded-xl text-sm font-mono text-zinc-300">
                    {formatAddress(midenDepositAddress)}
                  </div>
                  <button
                    onClick={copyToClipboard}
                    className="absolute right-3 top-1/2 -translate-y-1/2 p-2 bg-[#FF6B35]/10 hover:bg-[#FF6B35]/20 border border-[#FF6B35]/30 hover:border-[#FF6B35]/50 rounded-lg transition-all group/btn"
                    title="Copy address"
                  >
                    {copied ? (
                      <Check className="w-4 h-4 text-[#FF6B35] group-hover/btn:scale-110 transition-transform" />
                    ) : (
                      <Copy className="w-4 h-4 text-[#FF6B35] group-hover/btn:scale-110 transition-transform" />
                    )}
                  </button>
                </div>
                <p className="text-xs text-zinc-500 mt-2 font-medium">Send your TAZ deposit to this address</p>
              </div>
            )}

            {/* Address (for other directions) */}
            {!(fromChain === "Zcash" && toChain === "Miden") && (
              <div className="mb-6">
                <label className="block text-xs text-zinc-400 mb-2 uppercase tracking-widest font-semibold">Miden Address</label>
                <div className="relative group">
                  <input
                    type="text"
                    value={address}
                    onChange={(e) => setAddress(e.target.value)}
                    placeholder="Enter your Miden address"
                    className="w-full px-5 py-4 bg-zinc-950/80 border border-zinc-900 rounded-xl text-sm focus:outline-none focus:border-[#FF6B35]/50 focus:ring-2 focus:ring-[#FF6B35]/20 transition-all placeholder-zinc-700"
                  />
                  <div className="absolute inset-0 border-2 border-[#FF6B35]/0 rounded-xl group-focus-within:border-[#FF6B35]/30 transition-all pointer-events-none" />
                </div>
              </div>
            )}

            {/* Button */}
            <button className="relative w-full py-4 bg-[#FF6B35] text-black font-bold text-base rounded-xl hover:bg-[#FF6B35]/90 active:scale-[0.98] transition-all shadow-[0_0_40px_rgba(255,107,53,0.4)] hover:shadow-[0_0_60px_rgba(255,107,53,0.6)] overflow-hidden group">
              <span className="relative z-10 flex items-center justify-center gap-2">
                <Sparkles className="w-4 h-4" />
                Generate Deposit Address
              </span>
              <div className="absolute inset-0 bg-white/20 translate-y-full group-hover:translate-y-0 transition-transform duration-300" />
            </button>

            {/* Footer Note */}
            <div className="flex items-center justify-center gap-2.5 text-xs text-zinc-500 pt-6 mt-6 border-t border-zinc-900">
              <Lock className="w-3.5 h-3.5 text-[#FF6B35]/60" />
              <span className="font-medium tracking-wide">Private • Zero-knowledge • Non-custodial</span>
            </div>
          </div>
        </div>
      </div>

      <style jsx>{`
        @keyframes gridMove {
          0% { transform: translate(0, 0); }
          100% { transform: translate(60px, 60px); }
        }
        @keyframes scan {
          0% { transform: translateY(0); opacity: 0.3; }
          50% { opacity: 0.6; }
          100% { transform: translateY(100vh); opacity: 0.3; }
        }
        @keyframes pulse {
          0%, 100% { opacity: 0.08; transform: scale(1); }
          50% { opacity: 0.12; transform: scale(1.1); }
        }
      `}</style>
    </div>
  );
}