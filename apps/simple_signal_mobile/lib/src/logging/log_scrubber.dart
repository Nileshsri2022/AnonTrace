class LogScrubber {
  static final RegExp _pinPattern = RegExp(
    r'(masterPin:\s*|pin[:=]\s*)\d+',
    caseSensitive: false,
  );

  static final RegExp _uriParamPattern = RegExp(
    r'([?&](?:auth|ik|idhk|spk|sig|otpk|kpk)=)[0-9a-fA-F]+',
    caseSensitive: false,
  );

  /// Sanitizes text to remove sensitive cryptographic keys, tokens, and master PINs
  /// prior to in-memory persistence or user display.
  static String scrub(String input) {
    if (input.isEmpty) return input;

    // 1. Redact PIN patterns
    var sanitized = input.replaceAllMapped(_pinPattern, (match) {
      final prefix = match.group(1) ?? 'pin: ';
      return '${prefix}[REDACTED_PIN]';
    });

    // 2. Redact cryptographic prekeys and auth parameters in URIs
    sanitized = sanitized.replaceAllMapped(_uriParamPattern, (match) {
      final paramPrefix = match.group(1) ?? '';
      return '${paramPrefix}[REDACTED]';
    });

    return sanitized;
  }
}
