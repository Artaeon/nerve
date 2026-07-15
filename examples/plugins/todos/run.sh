#!/usr/bin/env bash
# Nerve plugin: /todos [path]
#
# Lists TODO/FIXME/HACK/XXX markers in the current repo, grouped by file.
#
# Why this is a plugin and not a model turn: the model would need several
# search/read tool calls (and the tokens for every result) to answer the same
# question. This runs locally, deterministically, and hands back one compact
# list — the plugin costs ZERO model tokens until its output is used.
#
# Contract (see docs/PLUGINS.md):
#   - args arrive as $1..$n and in $NERVE_ARGS
#   - $NERVE_PLUGIN_DIR points at this plugin's directory
#   - whatever we print on stdout becomes the plugin's result
#   - nerve strips ANSI/control chars and caps output at 1 MB
#   - a NON-ZERO exit makes nerve report the plugin as failed, so every normal
#     path here ends in `exit 0` (finding no markers is a result, not an error).
#     Note: deliberately NOT `set -e` — `git grep` exits non-zero on "no match".
set -uo pipefail

MARKERS='TODO|FIXME|HACK|XXX'
SCOPE="${1:-.}"
MAX=200

# Prefer `git grep` so ignored files (node_modules, target, build output) are
# skipped for free; fall back to plain grep outside a git repo.
if git rev-parse --is-inside-work-tree >/dev/null 2>&1; then
  hits=$(git grep -nIE "$MARKERS" -- "$SCOPE" 2>/dev/null)
else
  hits=$(grep -rnIE "$MARKERS" "$SCOPE" 2>/dev/null)
fi

if [ -z "$hits" ]; then
  echo "No TODO/FIXME/HACK/XXX markers found in ${SCOPE}."
  exit 0
fi

total=$(printf '%s\n' "$hits" | wc -l | tr -d ' ')

# Group by file, keeping it compact: "path (n)" then indented "line: text".
printf '%s\n' "$hits" \
  | head -n "$MAX" \
  | awk -F: '
      {
        file = $1; line = $2;
        # Re-join the rest (the matched text may itself contain colons).
        text = "";
        for (i = 3; i <= NF; i++) text = text (i > 3 ? ":" : "") $i;
        gsub(/^[ \t]*/, "", text);          # trim leading whitespace
        gsub(/^(\/\/|#|\/\*|\*)[ \t]*/, "", text);  # trim comment leaders
        if (file != prev) { if (prev != "") print ""; print file; prev = file }
        printf "  %s: %s\n", line, substr(text, 1, 120);
      }
    '

echo ""
if [ "$total" -gt "$MAX" ]; then
  echo "— showing first ${MAX} of ${total} markers (narrow it: /todos src/)"
else
  echo "— ${total} marker(s)"
fi

exit 0
