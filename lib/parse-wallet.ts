// Helper functions to parse wallet CLI output

export interface ParsedTransaction {
  txid: string;
  status: 'mined' | 'unmined' | 'expired';
  height?: number;
  date?: string;
  expiryHeight?: number;
  amount: string;
  fee: string;
  sentNotes: number;
  receivedNotes: number;
  memoCount: number;
  outputs: {
    pool: string;
    value: string;
    isChange: boolean;
    fromAccount?: string;
    toAccount?: string;
    toAddress?: string;
    memo?: string;
  }[];
}

export interface ParsedAddress {
  address: string;
  account?: string;
  pool?: string;
}

// Parse transaction list output
export function parseTransactions(output: string): ParsedTransaction[] {
  const transactions: ParsedTransaction[] = [];
  const lines = output.split('\n');
  
  let currentTx: Partial<ParsedTransaction> | null = null;
  let currentOutput: any = null;
  let inOutput = false;

  for (let i = 0; i < lines.length; i++) {
    const line = lines[i].trim();
    
    // Transaction ID (64 char hex)
    if (/^[a-f0-9]{64}$/i.test(line)) {
      if (currentTx) {
        if (currentOutput) currentTx.outputs?.push(currentOutput);
        transactions.push(currentTx as ParsedTransaction);
      }
      currentTx = {
        txid: line,
        outputs: [],
        status: 'mined',
      };
      currentOutput = null;
      inOutput = false;
      continue;
    }

    if (!currentTx) continue;

    // Mined status
    if (line.startsWith('Mined:')) {
      const match = line.match(/Mined:\s+(\d+)\s+\(([^)]+)\)/);
      if (match) {
        currentTx.status = 'mined';
        currentTx.height = parseInt(match[1]);
        currentTx.date = match[2];
      }
    }

    // Unmined status
    if (line.startsWith('Unmined') || line.startsWith('Expired')) {
      const match = line.match(/expiry height:\s+(\d+)/);
      if (match) {
        currentTx.status = line.startsWith('Expired') ? 'expired' : 'unmined';
        currentTx.expiryHeight = parseInt(match[1]);
      }
    }

    // Amount
    if (line.startsWith('Amount:')) {
      const match = line.match(/Amount:\s+([\d.]+)\s+ZEC/);
      if (match) currentTx.amount = match[1];
    }

    // Fee
    if (line.startsWith('Fee paid:')) {
      const match = line.match(/Fee paid:\s+([\d.]+)\s+ZEC/);
      if (match) {
        currentTx.fee = match[1];
      } else if (line.includes('Unknown')) {
        currentTx.fee = 'Unknown';
      }
    }

    // Notes and memos
    if (line.startsWith('Sent') && line.includes('notes')) {
      const match = line.match(/Sent\s+(\d+)\s+notes,\s+received\s+(\d+)\s+notes,\s+(\d+)\s+memos/);
      if (match) {
        currentTx.sentNotes = parseInt(match[1]);
        currentTx.receivedNotes = parseInt(match[2]);
        currentTx.memoCount = parseInt(match[3]);
      }
    }

    // Output
    if (line.startsWith('Output') && line.includes('(')) {
      if (currentOutput && currentTx.outputs) {
        currentTx.outputs.push(currentOutput);
      }
      const match = line.match(/Output\s+(\d+)\s+\((\w+)\)/);
      currentOutput = {
        index: match ? parseInt(match[1]) : 0,
        pool: match ? match[2] : 'Unknown',
        isChange: false,
      };
      inOutput = true;
    }

    if (inOutput && currentOutput) {
      // Value
      if (line.startsWith('Value:')) {
        const match = line.match(/Value:\s+([\d.]+)\s+ZEC/);
        if (match) currentOutput.value = match[1];
        const changeMatch = line.match(/\(Change\)/);
        if (changeMatch) currentOutput.isChange = true;
      }

      // From account
      if (line.startsWith('Sent from account:')) {
        const match = line.match(/Sent from account:\s+([^\(]+)\s+\(([^)]+)\)/);
        if (match) {
          currentOutput.fromAccount = match[2].trim();
        }
      }

      // To account
      if (line.startsWith('Received by account:')) {
        const match = line.match(/Received by account:\s+([^\(]+)\s+\(([^)]+)\)/);
        if (match) {
          currentOutput.toAccount = match[2].trim();
        }
      }

      // To address
      if (line.startsWith('To:')) {
        const match = line.match(/To:\s+(.+)/);
        if (match) currentOutput.toAddress = match[1].trim();
      }

      // Memo
      if (line.startsWith('Memo:')) {
        const match = line.match(/Memo:\s+(.+)/);
        if (match) {
          let memo = match[1].trim();
          // Parse Memo::Text("...")
          const textMatch = memo.match(/Memo::Text\("([^"]+)"\)/);
          if (textMatch) {
            currentOutput.memo = textMatch[1];
          } else if (memo === 'Memo::Empty') {
            currentOutput.memo = '';
          } else {
            currentOutput.memo = memo;
          }
        }
      }
    }
  }

  // Push last transaction
  if (currentTx) {
    if (currentOutput) currentTx.outputs?.push(currentOutput);
    transactions.push(currentTx as ParsedTransaction);
  }

  return transactions;
}

// Parse addresses output
export function parseAddresses(output: string): ParsedAddress[] {
  const addresses: ParsedAddress[] = [];
  const lines = output.split('\n');
  
  let currentAccount: string | undefined;
  
  for (let i = 0; i < lines.length; i++) {
    const line = lines[i];
    const trimmed = line.trim();
    
    // Look for Account line: "Account <uuid>" or "Account <uuid> (Name)"
    if (trimmed.startsWith('Account')) {
      const accountMatch = trimmed.match(/Account\s+([^\s\(]+)/);
      if (accountMatch) {
        currentAccount = accountMatch[1];
      }
    }
    
    // Look for "Default Address:" or "Address:" line (can have leading spaces)
    if (line.includes('Default Address:') || line.includes('Address:')) {
      // Match with or without leading spaces
      const match = line.match(/(?:Default\s+)?Address:\s+(.+)/);
      if (match) {
        const address = match[1].trim();
        // Unified addresses start with utest1 (testnet) or u1 (mainnet) and are long
        if (/^u(test1|1)[a-z0-9]{50,}$/i.test(address)) {
          addresses.push({ 
            address: address,
            account: currentAccount 
          });
        }
      }
    }
    
    // Also check for standalone addresses on their own line (unified addresses are very long)
    if (/^utest1[a-z0-9]{100,}$/i.test(trimmed)) {
      // Make sure we haven't already added this address
      if (!addresses.some(a => a.address === trimmed)) {
        addresses.push({ address: trimmed, account: currentAccount });
      }
    }
  }

  return addresses;
}

// Convert Zatoshis to ZEC
export function zatoshisToZec(zatoshis: string | number): string {
  const zats = typeof zatoshis === 'string' ? parseFloat(zatoshis) : zatoshis;
  return (zats / 100000000).toFixed(8);
}

// Convert ZEC to Zatoshis
export function zecToZatoshis(zec: string | number): string {
  const zecAmount = typeof zec === 'string' ? parseFloat(zec) : zec;
  return Math.floor(zecAmount * 100000000).toString();
}

