# 19. Scroll-to-Bottom Floating Action Button with Unread Badge

| Property | Value |
| :--- | :--- |
| **Status** | Completed |
| **Type** | Feature / UX Enhancement |
| **Release Target** | v1.0.14 |
| **Blockers** | 18 (Long-Press Actions) |

---

## 1. Context & User Story
When reviewing older messages or reading long conversation histories, users need an effortless way to return to the newest messages, and should be visually notified if new messages have arrived while they were reading history.

---

## 2. Technical Scope

### 2.1 Viewport State (`apps/simple_signal_mobile/lib/src/screens/chat_screen.dart`)
- Attach a `ScrollController` to the conversation `ListView`.
- Monitor scroll offset with listener:
  ```dart
  bool get _isScrolledAway => _scrollController.hasClients &&
      (_scrollController.position.maxScrollExtent - _scrollController.offset) > 120;
  ```
- Track `int _unreadWhileScrolled = 0`.
- In `messageStream.listen`: If `_isScrolledAway` is true, increment `_unreadWhileScrolled`.
- When user taps FAB or scrolls back to bottom, reset `_unreadWhileScrolled = 0`.

### 2.2 Floating UI Component
- Render a positioned circular button in the bottom right corner above the message input field:
  - Background: `#1E2630` with border `#2C6BED` or subtle glow.
  - Icon: `Icons.keyboard_arrow_down_rounded`.
  - Badge: If `_unreadWhileScrolled > 0`, display a green pill badge on the top right of the FAB containing the count (e.g. `1`, `2`).
  - Action: `_scrollController.animateTo(_scrollController.position.maxScrollExtent, duration: Duration(milliseconds: 300), curve: Curves.easeOut)`.

---

## 3. Verification Criteria
- [x] ScrollController attached to chat `ListView`.
- [x] FAB appears when user scrolls up away from bottom.
- [x] Incoming messages while scrolled up increment unread counter.
- [x] Tapping FAB scrolls smoothly to bottom and clears unread counter.
- [x] Widget test passes in `chat_screen_test.dart`.
