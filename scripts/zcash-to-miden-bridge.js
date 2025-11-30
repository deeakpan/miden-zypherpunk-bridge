/**
 * Zcash to Miden Bridge Script
 * Uses existing endpoints to bridge 0.3 TAZ from Zcash to Miden
 */

const BACKEND_URL = process.env.BACKEND_URL || "http://127.0.0.1:8001";
const AMOUNT = 0.3; // TAZ amount

// Get account ID from localStorage (if running in browser) or use provided one
function getAccountId() {
  if (typeof window !== "undefined") {
    const storedHex = localStorage.getItem("miden_account_id_hex");
    const storedBech32 = localStorage.getItem("miden_account_id");
    return storedHex || storedBech32 || null;
  }
  // For Node.js, you can set it here or pass as env var
  return process.env.MIDEN_ACCOUNT_ID || null;
}

// Generate random secret (32 bytes = 64 hex chars)
function generateSecret() {
  const array = new Uint8Array(32);
  if (typeof window !== "undefined") {
    crypto.getRandomValues(array);
  } else {
    // Node.js
    const crypto = require('crypto');
    crypto.randomFillSync(array);
  }
  return Array.from(array)
    .map(b => b.toString(16).padStart(2, '0'))
    .join('');
}

// Generate hash using backend endpoint
async function generateHash(accountId, secret) {
  const secretWithPrefix = secret.startsWith("0x") ? secret : `0x${secret}`;
  const url = `${BACKEND_URL}/deposit/hash?account_id=${encodeURIComponent(accountId)}&secret=${encodeURIComponent(secretWithPrefix)}`;
  
  console.log(`[1] Generating hash for account: ${accountId.substring(0, 20)}...`);
  
  const response = await fetch(url, {
    method: "GET",
  });

  if (!response.ok) {
    const error = await response.json();
    throw new Error(error.error || "Failed to generate hash");
  }

  const data = await response.json();
  if (!data.success || !data.recipient_hash) {
    throw new Error(data.error || "Invalid response from server");
  }

  console.log(`[1] ✅ Hash generated: ${data.recipient_hash.substring(0, 30)}...`);
  return data.recipient_hash;
}

// Claim deposit using backend endpoint
async function claimDeposit(accountId, secret, recipientHash) {
  const backendUrl = BACKEND_URL;
  const secretWithPrefix = secret.startsWith("0x") ? secret : `0x${secret}`;
  
  console.log(`[2] Claiming deposit for account: ${accountId.substring(0, 20)}...`);
  console.log(`[2] Amount: ${AMOUNT} TAZ`);
  
  const response = await fetch(`${backendUrl}/deposit/claim`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({
      account_id: accountId,
      secret: secretWithPrefix,
    }),
  });

  if (!response.ok) {
    const error = await response.json();
    throw new Error(error.error || error.message || "Failed to claim deposit");
  }

  const data = await response.json();
  console.log(`[2] ✅ Deposit claimed!`);
  console.log(`    Note ID: ${data.note_id || "N/A"}`);
  console.log(`    Transaction ID: ${data.transaction_id || "N/A"}`);
  console.log(`    Message: ${data.message}`);
  return data;
}

// Main bridge flow
async function bridgeZcashToMiden() {
  try {
    console.log("=".repeat(60));
    console.log("Zcash → Miden Bridge Script");
    console.log("=".repeat(60));
    console.log(`Amount: ${AMOUNT} TAZ`);
    console.log(`Backend: ${BACKEND_URL}`);
    console.log("");

    // Step 1: Get account ID
    const accountId = getAccountId();
    if (!accountId) {
      throw new Error("No Miden account ID found. Please set MIDEN_ACCOUNT_ID env var or connect wallet in browser.");
    }
    console.log(`[0] Using account: ${accountId.substring(0, 30)}...`);

    // Step 2: Generate secret
    const secret = generateSecret();
    console.log(`[0] Generated secret: ${secret.substring(0, 16)}...${secret.slice(-8)}`);
    console.log("");

    // Step 3: Generate hash
    const recipientHash = await generateHash(accountId, secret);
    console.log("");

    // Step 4: Display deposit info
    const depositAddress = process.env.BRIDGE_ZCASH_ADDRESS || 
      "utest1s7vrs7ycxvpu379zvtxt0fnc0efseur2f8g2s8puqls7nk45l6p7wvglu3rph9us9qzsjww44ly3wxlsul0jcpqx8qwvwqz4sq48rjj0cn59956sjsrz5ufuswd5ujy89n3vh264wx3843pxscnrf0ulku4990h65h5ll9r0j3q82mjgm2sx7lfnrkfkuqw9l2m7yfmgc4jvzq6n8j2";
    
    console.log("=".repeat(60));
    console.log("DEPOSIT INSTRUCTIONS:");
    console.log("=".repeat(60));
    console.log(`1. Send ${AMOUNT} TAZ to this address:`);
    console.log(`   ${depositAddress}`);
    console.log("");
    console.log(`2. Use this memo (recipient hash):`);
    console.log(`   ${recipientHash}`);
    console.log("");
    console.log(`3. Save this secret (you'll need it to claim):`);
    console.log(`   ${secret}`);
    console.log("=".repeat(60));
    console.log("");

    // Step 5: Wait for user to send transaction
    console.log("⏳ Waiting for deposit...");
    console.log("   (Send the TAZ transaction, then press Enter to claim)");
    console.log("");
    
    // In browser, we can't wait for Enter, so we'll poll
    if (typeof window !== "undefined") {
      console.log("   Auto-polling for deposit...");
      let attempts = 0;
      const maxAttempts = 60; // 5 minutes (5 second intervals)
      
      while (attempts < maxAttempts) {
        await new Promise(resolve => setTimeout(resolve, 5000)); // Wait 5 seconds
        attempts++;
        
        try {
          const result = await claimDeposit(accountId, secret, recipientHash);
          console.log("");
          console.log("=".repeat(60));
          console.log("✅ BRIDGE SUCCESSFUL!");
          console.log("=".repeat(60));
          return result;
        } catch (error) {
          if (error.message.includes("No deposit found")) {
            process.stdout.write(`\r   Attempt ${attempts}/${maxAttempts}... (no deposit found yet)`);
            continue;
          }
          throw error;
        }
      }
      
      throw new Error("Timeout: No deposit found after 5 minutes");
    } else {
      // Node.js - wait for Enter
      const readline = require('readline');
      const rl = readline.createInterface({
        input: process.stdin,
        output: process.stdout
      });
      
      await new Promise(resolve => {
        rl.question("Press Enter after sending the TAZ transaction to claim...", () => {
          rl.close();
          resolve();
        });
      });
      
      // Step 6: Claim deposit
      const result = await claimDeposit(accountId, secret, recipientHash);
      console.log("");
      console.log("=".repeat(60));
      console.log("✅ BRIDGE SUCCESSFUL!");
      console.log("=".repeat(60));
      return result;
    }

  } catch (error) {
    console.error("");
    console.error("=".repeat(60));
    console.error("❌ BRIDGE FAILED!");
    console.error("=".repeat(60));
    console.error(`Error: ${error.message}`);
    console.error("");
    process.exit(1);
  }
}

// Run the script
if (typeof window !== "undefined") {
  // Browser environment
  window.bridgeZcashToMiden = bridgeZcashToMiden;
  console.log("Bridge script loaded. Call bridgeZcashToMiden() to start.");
} else {
  // Node.js environment
  bridgeZcashToMiden();
}

