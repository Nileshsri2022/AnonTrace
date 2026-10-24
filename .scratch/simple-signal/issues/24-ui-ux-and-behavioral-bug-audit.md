# 24. Comprehensive UI/UX and Behavioral Bug Audit (WhatsApp-Parity & Privacy)

| Property | Value |
| :--- | :--- |
| **Status** | Completed (Verified with Rust & Flutter Test Suites) |
| **Type** | UI/UX Bug Audit & Diagnostics |
| **Target** | apps/simple_signal_mobile |
| **Framework Skill** | `diagnosing-bugs` |

---

## 1. Executive Summary
Following a comprehensive audit of all mobile presentation layers (`chat_screen.dart`, `main.dart`, `lock_screen.dart`, `qr_screen.dart`), multiple UI defects, WhatsApp behavioral mismatches, missing callbacks, and security lifecycle oversights were identified.

---

## 2. Comprehensive Inventory of Identified Bugs

### 📱 A. ChatScreen Bugs & WhatsApp Behavioral Flaws

1. **Bug 1: Message Text Right-Alignment Inside Outbound Bubbles (Visual Defect)**
   - **Symptom**: In outbound bubbles, `CrossAxisAlignment.end` is applied to the message text Column. Multi-line English/Hindi text gets right-aligned (ragged left), which is unnatural and deviates from WhatsApp.
   - **WhatsApp Standard**: Message text is always left-aligned (ragged right) for LTR languages; only the bubble container aligns to the right edge of the screen.

2. **Bug 2: Unwired Disappearing Messages Callback (Functional Bug)**
   - **Symptom**: `MainNavigationScreen` opens `ChatScreen` without providing `onSetDisappearingTimer`. When the user changes the timer via the AppBar menu, it only updates widget state and is permanently lost upon navigating away.
   - **WhatsApp Standard**: Ephemeral timers are persisted to the conversation record and remain active across sessions.

3. **Bug 3: Long-Press "Copy Text" Copies Raw Internal `[QUOTE:...]` Headers (Protocol Leak to User)**
   - **Symptom**: Long-pressing a reply and tapping "Copy Text" copies `[QUOTE:snippet]\nActual message` to the clipboard instead of clean user text.
   - **WhatsApp Standard**: Copying a quoted reply copies only the reply utterance.

4. **Bug 4: Quoted Reply Preview Shows Raw Protocol Tag (Visual Bug)**
   - **Symptom**: When replying to a message that already has a quote, the reply preview banner displays `[QUOTE:snippet] text`, exposing raw in-band protocol framing.
   - **WhatsApp Standard**: Strips previous quote headers and shows only the clean message excerpt.

5. **Bug 5: Inflexible Bubble Layout for Short Messages (Layout Bug)**
   - **Symptom**: Short 1-word messages ("Ok", "Hi") force a vertical Column with timestamp stacked below, resulting in tall, awkwardly stretched bubbles.
   - **WhatsApp Standard**: Short messages render timestamp and ticks inline on the same row next to the text.

6. **Bug 6: Send Button Does Not Reflect Input State (UX Defect)**
   - **Symptom**: The send icon button is always bright blue, even when `TextField` is empty. Tapping it does nothing silently without visual feedback.
   - **WhatsApp Standard**: Send button is disabled or dimmed when the text field contains only whitespace.

7. **Bug 7: Keyboard Opens Without Auto-Scrolling Chat to Frontier (UX Flaw)**
   - **Symptom**: When the on-screen keyboard appears, `ListView` shrinks but does not automatically animate to `maxScrollExtent`, obscuring the latest incoming messages.
   - **WhatsApp Standard**: Opening the keyboard automatically bumps the message frontier above the keyboard.

---

### 📋 B. Main Screen & Conversation List Bugs

8. **Bug 8: Chat List Displays Cryptic Raw Hex IDs and Hardcoded Subtitles**
   - **Symptom**: Every conversation in the list shows a raw 32-character hex ID (e.g. `conv_3a8f1...`) and a static subtitle `'End-to-end encrypted (PQXDH + Double Ratchet)'`. There is zero indication of the last message sent, message timestamp, or unread counter.
   - **WhatsApp Standard**: Displays human-readable peer alias, last message preview snippet, formatted time, and unread badge.

9. **Bug 9: Absence of Conversation Deletion from Main List (Missing Core Action)**
   - **Symptom**: Users cannot delete conversations from the main screen. If a conversation is no longer needed, there is no swipe-to-delete or long-press delete option.
   - **WhatsApp Standard**: Long-pressing a conversation item exposes Delete / Archive options.

10. **Bug 10: Missing Auto-Lock on App Backgrounding (Security/Privacy Flaw)**
    - **Symptom**: Once unlocked with the Master PIN, leaving the app, switching tasks, or locking the device leaves the app permanently unlocked in memory.
    - **Signal/AnonTrace Standard**: Backgrounding the app should automatically engage the LockScreen after a grace period or immediately, protecting data from unauthorized device access.

---

### 🔒 C. LockScreen & QR Scanner Bugs

11. **Bug 11: TabBarView Horizontal Swipe Intercepts Camera Scanner**
    - **Symptom**: In `QrContactScreen`, Tab 2 camera preview allows horizontal page swiping. Moving the phone to frame a QR code accidentally triggers tab swipe back to Tab 1.
    - **WhatsApp Standard**: Camera viewports disable horizontal swipe gestures (`NeverScrollableScrollPhysics`).

12. **Bug 12: Missing Flashlight/Torch Toggle in Camera Scanner**
    - **Symptom**: Scanning partner QR codes in low-light environments is unsupported because no torch toggle button is provided, despite `MobileScannerController` supporting it.
    - **WhatsApp Standard**: Prominent flashlight toggle button over the camera preview.
