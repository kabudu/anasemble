#!/bin/sh
set -eu

project_dir=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
output=${1:-"$project_dir/target/site"}

case "$output" in
  "$project_dir/target/"*) ;;
  *) echo "site output must be below target/: $output" >&2; exit 1 ;;
esac

test -f "$project_dir/website/index.html"
test -f "$project_dir/website/input.css"
test -f "$project_dir/website/package-lock.json"
test -f "$project_dir/assets/brand/exports/anasemble-symbol.svg"

(
  cd "$project_dir/website"
  npm ci --ignore-scripts
  npm run build
)

mkdir -p "$output/assets"
cp "$project_dir/website/index.html" "$output/index.html"
cp "$project_dir/website/styles.css" "$output/styles.css"
cp "$project_dir/assets/brand/exports/anasemble-symbol.svg" "$output/assets/anasemble-symbol.svg"
cp "$project_dir/assets/brand/exports/anasemble-architecture.svg" "$output/assets/anasemble-architecture.svg"
printf '%s\n' > "$output/.nojekyll"

test ! -L "$output/index.html"
test ! -L "$output/styles.css"
test ! -L "$output/assets/anasemble-symbol.svg"
printf 'site built at %s\n' "$output"
