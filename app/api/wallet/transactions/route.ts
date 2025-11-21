import { NextResponse } from 'next/server';
import { listTransactions } from '@/lib/zcash-cli';

export async function GET(request: Request) {
  try {
    const { searchParams } = new URL(request.url);
    const accountId = searchParams.get('accountId') || undefined;

    const result = await listTransactions(accountId);
    
    if (!result.success) {
      return NextResponse.json(
        { error: result.error || 'Failed to list transactions', stderr: result.stderr },
        { status: 500 }
      );
    }

    return NextResponse.json({
      success: true,
      transactions: result.stdout,
      raw: result.stdout,
    });
  } catch (error: any) {
    return NextResponse.json(
      { error: error.message || 'Internal server error' },
      { status: 500 }
    );
  }
}

