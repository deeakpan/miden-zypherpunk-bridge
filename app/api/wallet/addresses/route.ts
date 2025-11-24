import { NextResponse } from 'next/server';
import { listAddresses } from '@/lib/zcash-cli';

export async function GET(request: Request) {
  try {
    const { searchParams } = new URL(request.url);
    const accountId = searchParams.get('accountId') || undefined;

    console.log('Addresses API: Calling listAddresses()...', { accountId });
    const result = await listAddresses(accountId);
    console.log('Addresses API: Result:', { success: result.success, hasStdout: !!result.stdout, hasStderr: !!result.stderr });
    
    if (!result.success) {
      console.error('Addresses API: Command failed:', result.error, result.stderr);
      return NextResponse.json(
        { success: false, error: result.error || 'Failed to list addresses', stderr: result.stderr },
        { status: 500 }
      );
    }

    return NextResponse.json({
      success: true,
      addresses: result.stdout,
      raw: result.stdout,
    });
  } catch (error: any) {
    console.error('Addresses API: Exception:', error);
    return NextResponse.json(
      { success: false, error: error.message || 'Internal server error' },
      { status: 500 }
    );
  }
}

