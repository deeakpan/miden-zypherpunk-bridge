import { NextResponse } from 'next/server';
import { getBalance, parseBalance } from '@/lib/zcash-cli';

export async function GET() {
  try {
    const result = await getBalance();
    
    if (!result.success) {
      return NextResponse.json(
        { error: result.error || 'Failed to get balance', stderr: result.stderr },
        { status: 500 }
      );
    }

    const balance = parseBalance(result.stdout);
    
    return NextResponse.json({
      success: true,
      balance,
      raw: result.stdout,
    });
  } catch (error: any) {
    return NextResponse.json(
      { error: error.message || 'Internal server error' },
      { status: 500 }
    );
  }
}

