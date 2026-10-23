Analyzed SKILL.md:L1-356

**REA (Reverse-Engineer-Anything)** ka primary kaam documentation se aage badhkar **actual production apps (Signal & SimpleX) ke binaries, APKs, databases aur network protocols ko deconstruct karna** hai. 

Is hybrid project (**Signal Cryptography + SimpleX Queue Routing + Embedded Tor**) mein REA aapke liye ek **"X-Ray Tool"** ki tarah kaam karega taaki aapko edge cases scratch se guess na karne padein.

---

### Step-by-Step Flow: REA Aapke Project Me Kaise Help Karega

```
┌────────────────────────────────────────────────────────────────────────┐
│  Phase 1: SimpleX Reverse Engineering (Queue & Anonymous Identity)     │
│  Phase 2: Signal Protocol & Local Storage Reverse Engineering (E2EE)   │
│  Phase 3: Push Notification & Network Traffic Interception Flow        │
│  Phase 4: Arti (Rust-Tor) Binary Footprint & Mobile Memory Analysis   │
│  Phase 5: Reconstruction Blueprint (Rust Core + Flutter FFI)           │
└────────────────────────────────────────────────────────────────────────┘
```

---

#### **Phase 1: SimpleX Chat ko Reverse-Engineer karna (Routing & Queues)**
SimpleX ka documentation high-level hai, lekin exact SMP (Simple Messaging Protocol) packet framing aur client-side queue lifecycle production code me chhipi hoti hai.

* **Kya karenge:** SimpleX Android APK ya CLI binary ko REA me load karenge.
* **REA Tools:** 
  * `inspect_android_package` & `search_android_classes`: SimpleX APK ke SMP client modules, queue rotation logic, aur invitation link parser ko trace karna.
  * `inspect_rust_binary` / `inspect_go_binary`: Agar SimpleX ke native core library components use ho rahe hain, to unke dependency tree aur exported symbols verify karna.
* **Output:**
  * Exact handshake format: Queue generation ($Q_1 \rightarrow Q_2$) ke actual JSON/Binary payloads.
  * Onion address ke through Tor socket connection establish karne ka lifecycle.

---

#### **Phase 2: Signal ko Deconstruct karna (Double Ratchet & SQLCipher)**
Signal protocol standalone library (`libsignal`) me exist karta hai, lekin Android app me session storage, pre-key bundles aur database key derivation kaise wired hai, yeh dekhna zaroori hai.

* **Kya karenge:** Signal Android APK aur Signal Desktop (Electron ASAR) ko analyze karenge.
* **REA Tools:**
  * `analyze_javascript_application`: Signal Desktop ke extracted ASAR se state-machine, message queue handling aur SQLite migrations nikalna.
  * `inspect_sqlite_database`: Signal ke encrypted local SQLite schema ko study karna — ratcheting keys, pre-keys, aur session states disk par kaise store hoti hain.
  * `inspect_android_method`: `libsignal-client` ke JNI/FFI boundaries ko inspect karna (Rust to Java communication).
* **Output:**
  * Clean SQLite database schema aapke naye app ke liye.
  * Rust FFI boundary design pattern jo mobile UI ke sath smoothly sync kare.

---

#### **Phase 3: Ephemeral Push Notifications & Network Interception**
Sabse bada challenge hai: **Bina kisi phone number ya account ID ke mobile push notification kaise receive ho?**

* **Kya karenge:** SimpleX ke push notification gateway aur client communication ko trace karna.
* **REA Tools:**
  * `inspect_web_network_capture` / `inspect_packet_capture`: SimpleX app aur push gateway ke beech capture kiye gaye network flows (HAR/PCAP) ko decode karna.
  * `trace_application_feature`: Push token generation aur queue ID blinding logic ko app ke codebase me map karna.
* **Output:**
  * Blinding token ka exact cryptographic flow (Queue ID $\rightarrow$ Blinded Token $\rightarrow$ Apple APNs/Google FCM $\rightarrow$ Client Queue Check).

---

#### **Phase 4: Arti (Embedded Tor) Binary Footprint & Battery Optimization**
Arti ko mobile par embed karte waqt binary size aur background battery drain sabse badi problem banti hai.

* **Kya karenge:** Rust se compile kiye gaye Arti native binaries (`.so` / `.a`) ko inspect karna.
* **REA Tools:**
  * `inspect_binary_layout`: Section sizes, unnecessary debug symbols, aur relocations check karna.
  * `inspect_rust_binary`: Cargo auditable dependencies scan karna taaki heavy cryptographic crates ko strip karke minimum binary footprint banaya ja sake.
* **Output:**
  * Strip configuration aur minimal feature-flags list jisse mobile build 15–20MB se kam rahe.

---

#### **Phase 5: Reconstruction Ledger (Code Implementation)**
Reverse engineering ke baad REA ka use final code scaffold verify karne ke liye hota hai:

* **REA Tools:**
  * `build_reconstruction_obligation_ledger` & `verify_reconstruction`: Yeh check karna ki jo features humne SimpleX aur Signal me observe kiye the, kya hamare Rust core implementation me unka 100% equivalent exist karta hai.
* **Output:**
  * A clear checklist for developing the unified Rust crate (`simple-signal-core`) with `flutter_rust_bridge`.

---

### Aap Abhi Kya Shuru Karna Chahte Hain?

1. **Option 1 (Reverse Engineering Route):** SimpleX ya Signal ka koi APK/binary download karke REA ke through uska queue management aur database schema inspect karein.
2. **Option 2 (Direct Development Route):** Is workspace (`d:\vscode_programs\simple+signal`) me direct **Rust Core CLI project** initialize karein (`libsignal` + basic in-memory SMP queue prototype).