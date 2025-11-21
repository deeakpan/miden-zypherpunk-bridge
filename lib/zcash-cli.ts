import { exec } from 'child_process';
import { promisify } from 'util';
import path from 'path';

const execAsync = promisify(exec);

const WALLET_DIR = path.join(process.cwd(), 'wallet', 'personal_wallet');
const IDENTITY_FILE = path.join(WALLET_DIR, 'key.txt');
const ZCASH_DEVTOOL_DIR = path.join(process.cwd(), 'wallet', 'zcash-devtool');

export interface ZcashCommandResult {
  stdout: string;
  stderr: string;
  success: boolean;
  txid?: string;
  error?: string;
}

export async function execZcashCommand(args: string[]): Promise<ZcashCommandResult> {
  try {
    const command = `cargo run --release --all-features -- ${args.join(' ')}`;
    
    const { stdout, stderr } = await execAsync(command, {
      cwd: ZCASH_DEVTOOL_DIR,
      maxBuffer: 10 * 1024 * 1024, // 10MB buffer
      timeout: 300000, // 5 minute timeout
    });

    // Parse transaction ID from output (64 char hex string)
    const txidMatch = stdout.match(/\b([a-f0-9]{64})\b/i);
    const txid = txidMatch ? txidMatch[1] : undefined;

    return {
      stdout,
      stderr,
      success: true,
      txid,
    };
  } catch (error: any) {
    return {
      stdout: error.stdout || '',
      stderr: error.stderr || error.message || 'Unknown error',
      success: false,
      error: error.message || 'Command execution failed',
    };
  }
}

export async function getBalance(): Promise<ZcashCommandResult> {
  return execZcashCommand(['wallet', '-w', WALLET_DIR, 'balance']);
}

export async function syncWallet(): Promise<ZcashCommandResult> {
  return execZcashCommand(['wallet', '-w', WALLET_DIR, 'sync', '-s', 'zecrocks']);
}

export async function listAccounts(): Promise<ZcashCommandResult> {
  return execZcashCommand(['wallet', '-w', WALLET_DIR, 'list-accounts']);
}

export async function listAddresses(accountId?: string): Promise<ZcashCommandResult> {
  const args = ['wallet', '-w', WALLET_DIR, 'list-addresses'];
  if (accountId) {
    args.push('--account-id', accountId);
  }
  return execZcashCommand(args);
}

export async function listTransactions(accountId?: string): Promise<ZcashCommandResult> {
  const args = ['wallet', '-w', WALLET_DIR, 'list-tx'];
  if (accountId) {
    args.push('--account-id', accountId);
  }
  return execZcashCommand(args);
}

export async function sendTransaction(
  address: string,
  amount: string,
  memo?: string,
  accountId?: string
): Promise<ZcashCommandResult> {
  const args = [
    'wallet',
    '-w', WALLET_DIR,
    'send',
    '--identity', IDENTITY_FILE,
    '--address', address,
    '--value', amount,
    '--target-note-count', '1',
    '-s', 'zecrocks',
  ];

  if (accountId) {
    args.push('--account-id', accountId);
  }

  if (memo) {
    args.push('--memo', memo);
  }

  return execZcashCommand(args);
}

// Helper to parse balance from CLI output
export function parseBalance(output: string): {
  total: string;
  spendable: string;
  pending: string;
} {
  const lines = output.split('\n');
  let total = '0';
  let spendable = '0';
  let pending = '0';

  for (const line of lines) {
    // Parse main Balance line: "Balance:   0.19990000 ZEC"
    if (line.trim().startsWith('Balance:')) {
      const match = line.match(/Balance:\s+(\d+\.\d+)/);
      if (match) total = match[1];
    }
    // Parse Sapling Spendable
    if (line.includes('Sapling Spendable:')) {
      const match = line.match(/Sapling Spendable:\s+(\d+\.\d+)/);
      if (match) spendable = match[1];
    }
    // Parse Orchard Spendable
    if (line.includes('Orchard Spendable:')) {
      const match = line.match(/Orchard Spendable:\s+(\d+\.\d+)/);
      if (match) {
        const orchard = parseFloat(match[1]);
        const sapling = parseFloat(spendable);
        spendable = (orchard + sapling).toFixed(8);
      }
    }
  }

  return { total, spendable, pending };
}

