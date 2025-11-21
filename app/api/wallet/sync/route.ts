import { NextResponse } from 'next/server';
import { syncWallet } from '@/lib/zcash-cli';

export async function POST() {
  try {
    const result = await syncWallet();
    
    if (!result.success) {
      return NextResponse.json(
        { error: result.error || 'Failed to sync wallet', stderr: result.stderr },
        { status: 500 }
      );
    }

    return NextResponse.json({
      success: true,
      message: 'Wallet synced successfully',
      output: result.stdout,
    });
  } catch (error: any) {
    return NextResponse.json(
      { error: error.message || 'Internal server error' },
      { status: 500 }
    );
  }
}

