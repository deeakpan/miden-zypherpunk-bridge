import { NextResponse } from 'next/server';
import { listTransactions } from '@/lib/zcash-cli';

export async function GET(request: Request) {
  try {
    const { searchParams } = new URL(request.url);
    const accountId = searchParams.get('accountId') || undefined;

    console.log('Transactions API: Calling listTransactions()...', { accountId });
    const result = await listTransactions(accountId);
    console.log('Transactions API: Result:', { success: result.success, hasStdout: !!result.stdout, hasStderr: !!result.stderr });
    
    if (!result.success) {
      console.error('Transactions API: Command failed:', result.error, result.stderr);
      return NextResponse.json(
        { success: false, error: result.error || 'Failed to list transactions', stderr: result.stderr },
        { status: 500 }
      );
    }

    return NextResponse.json({
      success: true,
      transactions: result.stdout,
      raw: result.stdout,
    });
  } catch (error: any) {
    console.error('Transactions API: Exception:', error);
    return NextResponse.json(
      { success: false, error: error.message || 'Internal server error' },
      { status: 500 }
    );
  }
}

