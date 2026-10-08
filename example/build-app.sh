#!/bin/bash
set -euo pipefail
example_dir="$(cd "$(dirname "$0")" && pwd)"
cd "$example_dir"
swift scripts/build-icons.swift assets/branding Metabook.app/Contents/Resources
cargo build --locked
mkdir -p Metabook.app/Contents/MacOS
cp -f target/debug/metabook-example Metabook.app/Contents/MacOS/metabook-example
chmod +x Metabook.app/Contents/MacOS/Metabook
echo "Built $example_dir/Metabook.app"
