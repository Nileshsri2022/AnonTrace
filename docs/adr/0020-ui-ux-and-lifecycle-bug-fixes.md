# ADR-0020: UI/UX Behavioral Realignment and Privacy Lifecycle Hardening

## Status
Proposed (Awaiting User Confirmation)

## Context
AnonTrace is designed as a metadata-free, post-quantum messaging client combining Signal Double Ratchet with SimpleX unidirectional queues over embedded Tor.
Following rapid implementation of chat timeline features, a thorough diagnostic audit using the `diagnosing-bugs` framework revealed 12 distinct UI/UX discrepancies, behavioral flaws, and missing lifecycle handlers across `chat_screen.dart`, `main.dart`, `lock_screen.dart`, and `qr_screen.dart`.

## Proposed Decisions & Architecture

1. **Chat Bubble Text Alignment**:
   - Change message text column alignment inside bubbles to always be `CrossAxisAlignment.start` (left-aligned) for reading comfort, while keeping the container right-aligned for outbound messages.
2. **Protocol Encapsulation & Text Sanitation**:
   - When copying message text, strip internal `[QUOTE:...]` tags and copy only the user's utterance.
   - In reply preview banner, parse `parsedBody.mainText` rather than displaying raw `bodyText`.
3. **Disappearing Messages Plumbing**:
   - Pass `onSetDisappearingTimer: (sec) => _bridge.setDisappearingTimer(conversationId: conv.id, durationSeconds: sec)` from `MainNavigationScreen`.
4. **Send Button Dynamics**:
   - Listen to `_msgController` text changes; dynamically enable/dim the send button.
5. **Keyboard & Scroll Synchronization**:
   - Detect keyboard visibility changes or scroll listener to maintain message visibility above the on-screen keyboard.
6. **Main Conversation List Rich Metadata**:
   - Provide friendly peer alias display, last message excerpt, and timestamp.
   - Add long-press to delete conversation with cascading delete confirmation dialog.
7. **Security Lifecycle Auto-Lock**:
   - Implement `WidgetsBindingObserver` in `MainNavigationScreen` to automatically engage `_isUnlocked = false` when `AppLifecycleState.paused` or `AppLifecycleState.inactive` occurs.
8. **QR Scanner Stabilization**:
   - Disable swipe gesture collision on TabBarView (`NeverScrollableScrollPhysics` when camera is active).
   - Add flashlight/torch toggle button over camera preview.

## Consequences
- Elevates AnonTrace to WhatsApp-level UX polish while strictly upholding Zero-Knowledge and Tor routing guarantees.
- Prevents security regressions where leaving the app leaves unencrypted views accessible.
