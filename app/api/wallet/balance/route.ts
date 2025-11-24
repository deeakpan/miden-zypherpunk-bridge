import { NextResponse } from 'next/server';
import { getBalance, parseBalance } from '@/lib/zcash-cli';

export async function GET() {
  try {
    console.log('Balance API: Calling getBalance()...');
    const result = await getBalance();
    console.log('Balance API: Result:', { success: result.success, hasStdout: !!result.stdout, hasStderr: !!result.stderr });
    
    if (!result.success) {
      console.error('Balance API: Command failed:', result.error, result.stderr);
      return NextResponse.json(
        { success: false, error: result.error || 'Failed to get balance', stderr: result.stderr },
        { status: 500 }
      );
    }

    const balance = parseBalance(result.stdout);
    console.log('Balance API: Parsed balance:', balance);
    
    return NextResponse.json({
      success: true,
      balance,
      raw: result.stdout,
    });
  } catch (error: any) {
    console.error('Balance API: Exception:', error);
    return NextResponse.json(
      { success: false, error: error.message || 'Internal server error' },
      { status: 500 }
    );
  }
}

