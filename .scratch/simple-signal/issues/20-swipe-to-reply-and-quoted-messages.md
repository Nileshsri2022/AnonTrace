# 20. Swipe-to-Reply and Quoted Message Bubbles

| Property | Value |
| :--- | :--- |
| **Status** | Completed |
| **Type** | Feature / Messaging UX |
| **Release Target** | v1.0.14 |
| **Blockers** | 19 (Scroll-to-Bottom FAB) |

---

## 1. Context & User Story
In multi-turn conversations, users need to reply directly to a specific previous message so the recipient understands what is being answered, identical to WhatsApp and Signal.

---

## 2. Technical Scope

### 2.1 In-Band Quoted Payload Modeling (`apps/simple_signal_mobile/lib/src/utils/timeline_adapter.dart`)
Implement parser and formatter:
```dart
class ParsedMessageBody {
  final String? quotedSnippet;
  final String mainText;

  const ParsedMessageBody({this.quotedSnippet, required this.mainText});
}

ParsedMessageBody parseMessageBody(String rawText);
String buildQuotedPayload(String quotedSnippet, String replyText);
```
Payload format:
`[QUOTE:quoted_text_here]\nactual_reply_text`

### 2.2 ChatScreen Reply Mode (`apps/simple_signal_mobile/lib/src/screens/chat_screen.dart`)
- Add `MessageSummary? _replyingTo;` state in `_ChatScreenState`.
- Action Sheet long-press: Add "Reply" option which sets `_replyingTo = msg`.
- Horizontal swipe on bubble sets `_replyingTo = msg`.
- When `_replyingTo != null`:
  - Show a WhatsApp-style quote bar above the message input text field:
    - Vertical accent bar
    - "Replying to [Anonymous Peer / You]"
    - Quoted text preview (1 line, ellipsis)
    - Close button `(X)` to cancel reply.
- When sending message:
  - If `_replyingTo != null`, call `buildQuotedPayload(_replyingTo!.bodyText, text)` and reset `_replyingTo = null`.
- When rendering message bubble:
  - If `parseMessageBody(msg.bodyText).quotedSnippet != null`:
    - Render a stylish quote container at the top of the bubble with vertical accent line, rounded corners, and subdued background.
    - Render `mainText` below the quote container.

---

## 3. Verification Criteria
- [x] Unit tests for `parseMessageBody` and `buildQuotedPayload` pass.
- [x] Selecting "Reply" or swiping activates reply banner above input field.
- [x] Sending with reply renders quote container inside new message bubble.
- [x] Tapping cancel `(X)` clears reply state.
- [x] All Flutter tests (`flutter test`) pass.
