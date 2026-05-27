#!/usr/bin/env bash
# Usage: ./scripts/release.sh [patch|minor|major] [--build] [--universal]
#
# Bumps version in package.json, Cargo.toml, and tauri.conf.json.
# Pass --build to also run `npm run tauri build` after bumping.
# Pass --universal to build a universal binary (Apple Silicon + Intel).
#   Requires both Rust targets installed:
#     rustup target add aarch64-apple-darwin x86_64-apple-darwin

set -e

BUMP="${1:-patch}"
BUILD=false
UNIVERSAL=false
for arg in "$@"; do
  [[ "$arg" == "--build" ]]     && BUILD=true
  [[ "$arg" == "--universal" ]] && UNIVERSAL=true
done

if [[ "$BUMP" != "patch" && "$BUMP" != "minor" && "$BUMP" != "major" ]]; then
  echo "Usage: $0 [patch|minor|major] [--build] [--universal]"
  exit 1
fi

check_universal_targets() {
  for t in x86_64-apple-darwin aarch64-apple-darwin; do
    if ! rustup target list --installed 2>/dev/null | grep -q "$t"; then
      echo "Warning: Rust target $t not installed. Run: rustup target add $t"
    fi
  done
}

CURRENT=$(node -p "require('./package.json').version")
IFS='.' read -r MAJ MIN PAT <<< "$CURRENT"

case "$BUMP" in
  major) MAJ=$((MAJ + 1)); MIN=0; PAT=0 ;;
  minor) MIN=$((MIN + 1)); PAT=0 ;;
  patch) PAT=$((PAT + 1)) ;;
esac

NEW="$MAJ.$MIN.$PAT"
echo "Version: $CURRENT → $NEW"

# package.json
node -e "
  const fs = require('fs');
  const p = JSON.parse(fs.readFileSync('package.json', 'utf8'));
  p.version = '$NEW';
  fs.writeFileSync('package.json', JSON.stringify(p, null, 2) + '\n');
"

# Cargo.toml — only the [package] section
sed -i '' "/^\[package\]/,/^\[/ s/^version = \"$CURRENT\"/version = \"$NEW\"/" src-tauri/Cargo.toml

# tauri.conf.json
node -e "
  const fs = require('fs');
  const p = JSON.parse(fs.readFileSync('src-tauri/tauri.conf.json', 'utf8'));
  p.version = '$NEW';
  fs.writeFileSync('src-tauri/tauri.conf.json', JSON.stringify(p, null, 2) + '\n');
"

echo "Updated: package.json, src-tauri/Cargo.toml, src-tauri/tauri.conf.json"

if $BUILD; then
  echo ""

  if $UNIVERSAL; then
    check_universal_targets
    TARGET_ARGS=(--target universal-apple-darwin)
    DMG_PATH="src-tauri/target/universal-apple-darwin/release/bundle/dmg/Buckethead_${NEW}_universal.dmg"
    echo "Building universal release (aarch64 + x86_64)..."
  else
    TARGET_ARGS=()
    DMG_PATH="src-tauri/target/release/bundle/dmg/Buckethead_${NEW}_aarch64.dmg"
    echo "Building release..."
  fi

  npm run tauri build -- "${TARGET_ARGS[@]}"
  echo ""
  echo "DMG: $DMG_PATH"
fi
