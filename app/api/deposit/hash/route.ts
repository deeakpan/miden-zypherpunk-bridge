import { NextRequest, NextResponse } from 'next/server';

/**
 * Call Rust backend to generate hash (server-to-server, faster than client calls)
 * This keeps the API on the same Next.js server from the frontend's perspective
 */
async function generateHashFromBackend(accountId: string, secret: string): Promise<string> {
  const backendUrl = process.env.RUST_BACKEND_URL || 'http://127.0.0.1:8001';
  const url = `${backendUrl}/deposit/hash?account_id=${encodeURIComponent(accountId)}&secret=${encodeURIComponent(secret)}`;
  
  const response = await fetch(url, {
    method: 'GET',
    headers: {
      'Content-Type': 'application/json',
    },
  });
  
  // Read response body once
  let data: any;
  try {
    data = await response.json();
  } catch (jsonError) {
    // If JSON parsing fails, try to get text
    const text = await response.text();
    throw new Error(`Backend returned invalid JSON: ${text || response.statusText}`);
  }
  
  if (!response.ok) {
    throw new Error(data.error || `Backend error: ${response.statusText}`);
  }
  
  if (!data.success || !data.recipient_hash) {
    throw new Error(data.error || 'Invalid response from backend');
  }
  
  return data.recipient_hash;
}

export async function GET(request: NextRequest) {
  try {
    const searchParams = request.nextUrl.searchParams;
    const accountIdStr = searchParams.get('account_id');
    const secretStr = searchParams.get('secret');

    if (!accountIdStr || !secretStr) {
      return NextResponse.json(
        { success: false, error: 'Missing account_id or secret parameter' },
        { status: 400 }
      );
    }

    const trimmedAccountId = accountIdStr.trim();
    const trimmedSecret = secretStr.trim();
    
    // Ensure secret has 0x prefix
    const secretWithPrefix = trimmedSecret.startsWith('0x') ? trimmedSecret : `0x${trimmedSecret}`;
    
    // Call Rust backend to generate hash (server-to-server call)
    const recipientHash = await generateHashFromBackend(trimmedAccountId, secretWithPrefix);

    return NextResponse.json({
      success: true,
      recipient_hash: recipientHash,
    });
  } catch (error: any) {
    console.error('Hash generation error:', error);
    return NextResponse.json(
      { 
        success: false, 
        error: error.message || 'Failed to generate hash' 
      },
      { status: 500 }
    );
  }
}

export async function POST(request: NextRequest) {
  try {
    const body = await request.json();
    const { account_id, secret } = body;

    if (!account_id || !secret) {
      return NextResponse.json(
        { success: false, error: 'Missing account_id or secret in request body' },
        { status: 400 }
      );
    }

    // Reuse the same logic as GET
    const url = new URL('http://localhost/api/deposit/hash');
    url.searchParams.set('account_id', account_id);
    url.searchParams.set('secret', secret);
    
    const getRequest = new NextRequest(url);
    return GET(getRequest);
  } catch (error: any) {
    console.error('Hash generation error:', error);
    return NextResponse.json(
      { 
        success: false, 
        error: error.message || 'Failed to generate hash' 
      },
      { status: 500 }
    );
  }
}

