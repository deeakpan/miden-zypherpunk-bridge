"use client";

import { Loader2 } from "lucide-react";

interface LoadingModalProps {
  isOpen: boolean;
  message?: string;
}

export default function LoadingModal({ isOpen, message = "Generating Hash & Secret..." }: LoadingModalProps) {
  if (!isOpen) return null;

  return (
    <div className="fixed inset-0 bg-black/80 backdrop-blur-sm z-50 flex items-center justify-center">
      <div className="bg-black/90 border border-[#FF6B35]/30 rounded-2xl p-8 flex flex-col items-center gap-4">
        <div className="relative">
          <Loader2 className="w-12 h-12 text-[#FF6B35] animate-spin" />
          <div className="absolute inset-0 flex items-center justify-center">
            <div className="w-8 h-8 border-2 border-[#FF6B35]/30 rounded-full animate-ping" />
          </div>
        </div>
        <div className="text-[#FF6B35] font-semibold">{message}</div>
      </div>
    </div>
  );
}

