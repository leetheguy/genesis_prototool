#!/usr/bin/env bash
# Checks that the built CSS has a rule for every class a style key can give (spec 040, "Tests"):
# every line of crates/genesis-ui/classes.txt, in apps/web/style/genesis.css. Run after trunk build.
set -euo pipefail
cd "$(dirname "$0")/.."
css=apps/web/style/genesis.css
missing=0
while IFS= read -r class; do
  # CSS escapes these characters in a class selector with a backslash.
  selector=".$(printf '%s' "$class" | sed 's|[]:/()[,.%]|\\&|g')"
  if ! grep -qF -- "$selector{" "$css" && ! grep -qF -- "$selector," "$css" && ! grep -qF -- "$selector:" "$css"; then
    echo "check-css: no rule for $class in $css" >&2
    missing=1
  fi
done < crates/genesis-ui/classes.txt
[ "$missing" = 0 ] && echo "check-css: every style class is in the CSS"
exit "$missing"
