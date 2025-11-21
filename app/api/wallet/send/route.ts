import { NextResponse } from 'next/server';
import { sendTransaction } from '@/lib/zcash-cli';

export async function POST(request: Request) {
  try {
    const body = await request.json();
    const { address, amount, memo, accountId } = body;

    if (!address || !amount) {
      return NextResponse.json(
        { error: 'Address and amount are required' },
        { status: 400 }
      );
    }

    const result = await sendTransaction(address, amount, memo, accountId);
    
    if (!result.success) {
      return NextResponse.json(
        { 
          error: result.error || 'Failed to send transaction', 
          stderr: result.stderr,
          stdout: result.stdout,
        },
        { status: 500 }
      );
    }

    return NextResponse.json({
      success: true,
      txid: result.txid,
      message: 'Transaction sent successfully',
      output: result.stdout,
    });
  } catch (error: any) {
    return NextResponse.json(
      { error: error.message || 'Internal server error' },
      { status: 500 }
    );
  }
}

