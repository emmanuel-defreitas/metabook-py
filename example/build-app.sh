#!/bin/bash
set -euo pipefail
example_dir="$(cd "$(dirname "$0")" && pwd)"
cd "$example_dir"
mkdir -p Metabook.app/Contents/Resources target
xcrun actool "$example_dir/assets/branding/metabook.icon" \
    --compile "$example_dir/Metabook.app/Contents/Resources" \
    --app-icon metabook --platform macosx --minimum-deployment-target 11.0 \
    --output-partial-info-plist "$example_dir/target/metabook-icon-info.plist"
cargo build --locked
mkdir -p Metabook.app/Contents/MacOS
cp -f target/debug/metabook-example Metabook.app/Contents/MacOS/metabook-example
chmod +x Metabook.app/Contents/MacOS/Metabook
echo "Built $example_dir/Metabook.app"
