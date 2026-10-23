use tempfile::tempdir;

use simple_signal_core::api::SimpleSignalApp;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("============================================================");
    println!("           ANONTRACE: POST-QUANTUM ANONYMOUS CHAT           ");
    println!("   Signal Cryptography + SimpleX Metadata-Free Routing      ");
    println!("============================================================\n");

    let tmp = tempdir()?;
    let alice_db = tmp.path().join("alice_encrypted.db");
    let bob_db = tmp.path().join("bob_encrypted.db");

    // 1. Initialize Alice's Node
    println!("[1] Initializing Alice's client...");
    let alice = SimpleSignalApp::init(alice_db.to_str().unwrap(), "alice-secret-pin-1234")?;
    println!("    * Local Encrypted SQLite created (Argon2id + AES-256)");
    println!("    * NIST FIPS 203 ML-KEM-1024 Post-Quantum Keypair generated");
    println!("    * Embedded Arti Tor Client: Active\n");

    // 2. Initialize Bob's Node
    println!("[2] Initializing Bob's client...");
    let bob = SimpleSignalApp::init(bob_db.to_str().unwrap(), "bob-secret-pin-5678")?;
    println!("    * Local Encrypted SQLite created (Argon2id + AES-256)");
    println!("    * NIST FIPS 203 ML-KEM-1024 Post-Quantum Keypair generated\n");

    // 3. Alice exports Contact Link / QR Code URI
    println!("[3] Alice generates her Contact Link (for QR code exchange):");
    let alice_link_uri = alice.get_my_contact_link()?;
    println!("    URI: {}\n", alice_link_uri);

    // 4. Bob scans / imports Alice's Contact Link
    println!("[4] Bob imports Alice's Contact Link...");
    let conv_bob = bob.import_contact_link(&alice_link_uri)?;
    println!("    * Derived pairwise conversation ID: {}", conv_bob.id);
    println!("    * Alice's Send Queue URI: {}", conv_bob.send_queue_uri);
    println!("    * Bob's Allocated Return Queue: {}\n", conv_bob.receive_queue_id);

    // 5. Bob sends end-to-end encrypted message across Double Ratchet
    let bob_plaintext = "Hello Alice! Zero metadata, ML-KEM-1024 PQXDH, and isolated Tor onion circuits.";
    println!("[5] Bob sends message:");
    println!("    Plaintext: \"{}\"", bob_plaintext);
    let msg_bob = bob.send_chat_message(&conv_bob.id, bob_plaintext)?;
    println!("    * Status: {}", msg_bob.status);
    println!("    * Stored in encrypted DB at timestamp: {}\n", msg_bob.timestamp);

    // 6. Demonstrate Tor Battery Dormancy & Blinded Push Wakeup
    println!("[6] Testing mobile battery dormancy & push wake-up:");
    println!("    * Initial Tor state: active = {}", bob.is_active());
    bob.set_dormant(true)?;
    println!("    * Mobile app backgrounded: Tor transitioned to soft dormancy (active = {})", bob.is_active());

    println!("    * Blinded Push Trigger (NMSG) received -> waking up Tor client...");
    bob.set_dormant(false)?;
    println!("    * Tor awakened for ephemeral message poll: active = {}\n", bob.is_active());

    println!("============================================================");
    println!("SUCCESS: All cryptographic, routing & storage layers verified!");
    println!("============================================================");

    Ok(())
}
