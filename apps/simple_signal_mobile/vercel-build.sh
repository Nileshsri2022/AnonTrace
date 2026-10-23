#!/bin/bash
set -e

echo ">>> [Simple+Signal] Installing Flutter SDK on Vercel build container..."
if [ ! -d "flutter" ]; then
  git clone https://github.com/flutter/flutter.git --depth 1 -b stable flutter
fi

export PATH="$PATH:$(pwd)/flutter/bin"
flutter config --no-analytics
flutter --version

echo ">>> [Simple+Signal] Installing dependencies and building Flutter Web..."
flutter pub get
flutter build web --release

echo ">>> [Simple+Signal] Flutter Web build successful!"
