// Test script to check AccountId conversion
(async () => {
  try {
    const { AccountId, NetworkId } = await import("@demox-labs/miden-sdk");

    // Test with your bech32
    const bech32 = "mtst1arqdc9zj0cr3ayzp4y39vnp4qgg26qhs_qruqqypuyph";
    
    console.log("Testing bech32:", bech32);
    const fromBech32 = AccountId.fromBech32(bech32);
    console.log("From bech32 inner:", fromBech32.inner);
    console.log("From bech32 toString:", fromBech32.toString());
    console.log("From bech32 toHex:", fromBech32.toHex ? fromBech32.toHex() : "toHex not available");
    
    // Test with your hex (the padded one)
    const hex = "0000000000000000000000000000000000c2131bd51cbf8d900327b5cd997ef2";
    
    console.log("\nTesting hex:", hex);
    const fromHex = AccountId.fromHex(hex);
    console.log("From hex inner:", fromHex.inner);
    console.log("From hex toBech32:", fromHex.toBech32(NetworkId.Testnet));
    console.log("From hex toString:", fromHex.toString());
    
    // Do they match?
    console.log("\nMatch?", fromBech32.inner === fromHex.inner);
    console.log("Match (toString)?", fromBech32.toString() === fromHex.toString());
    
    // Try to get the correct hex from bech32
    console.log("\n=== Getting hex from bech32 ===");
    if (fromBech32.inner) {
      console.log("Inner value:", fromBech32.inner);
      console.log("Inner type:", typeof fromBech32.inner);
      console.log("Inner string:", String(fromBech32.inner));
    }
    
    // Check all properties
    console.log("\n=== All properties of fromBech32 ===");
    console.log(Object.keys(fromBech32));
    console.log(Object.getOwnPropertyNames(fromBech32));
    
  } catch (error) {
    console.error("Error:", error);
  }
})();

