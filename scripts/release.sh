#!/usr/bin/env bash
# Usage: ./scripts/release.sh [patch|minor|major] [--build]
# Bumps version in package.json, Cargo.toml, and tauri.conf.json.
# Pass --build to also run `npm run tauri build` after bumping.

set -e

BUMP="${1:-patch}"
BUILD=false
for arg in "$@"; do [[ "$arg" == "--build" ]] && BUILD=true; done

if [[ "$BUMP" != "patch" && "$BUMP" != "minor" && "$BUMP" != "major" ]]; then
  echo "Usage: $0 [patch|minor|major] [--build]"
  exit 1
fi

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
  echo "Building release..."
  npm run tauri build
  echo ""
  echo "DMG: src-tauri/target/release/bundle/dmg/AWS Thathoo_${NEW}_aarch64.dmg"
fi
