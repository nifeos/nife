#!/bin/sh
# Prove helpers/owned-canary-image.sh exits non-zero when the probe prints a FINDING line (even if
# the probe itself exits 0), non-zero when the probe exits non-zero, and zero when it prints none.
# Also requires the workflow's Image step to call the wrapper and not to set a verdict variable
# inside a pipe.
#
# Name: provisional, minted by lane/canary-finding-fails-job.

set -u
cd "$(dirname "$0")/.." || exit 2

dir=$(mktemp -d) || exit 2
trap 'rm -rf "$dir"' EXIT
failed=0

fail() {
    echo "owned-canary-image-selftest: $*" >&2
    failed=1
}

cat >"$dir/finding" <<'PROBE'
#!/bin/sh
echo "FINDING $1 is not on PATH"
exit 0
PROBE
cat >"$dir/finding-exit" <<'PROBE'
#!/bin/sh
echo "FINDING $1 is not on PATH"
exit 1
PROBE
cat >"$dir/silent-exit" <<'PROBE'
#!/bin/sh
exit 1
PROBE
cat >"$dir/clean" <<'PROBE'
#!/bin/sh
echo "$1: ok"
exit 0
PROBE
chmod +x "$dir/finding" "$dir/finding-exit" "$dir/silent-exit" "$dir/clean"

run() {
    OWNED_IMAGE_PROBE="$dir/$1" GITHUB_STEP_SUMMARY="$dir/summary" \
        helpers/owned-canary-image.sh tools toolchain >"$dir/stdout" 2>&1
}

run finding && fail "a FINDING line with probe exit 0 must fail"
run finding-exit && fail "a FINDING line with probe exit 1 must fail"
run silent-exit && fail "a probe exit 1 must fail"
run clean || fail "a clean probe must pass"
grep -q '^FINDING' "$dir/summary" || fail "the summary must carry the FINDING lines"

# The Image step must call the wrapper, and no step may assign bad= or open= inside a tee group.
wf=.github/workflows/owned-runner-canary.yml
grep -q 'helpers/owned-canary-image.sh' "$wf" || fail "the Image step does not call the wrapper"
if awk '/^ *\{ *$/ {grp = 1; buf = ""; next}
        grp && /^ *\} / {
            if ($0 ~ /\| *tee/ && buf ~ /(bad|open)=1/) { print "group ending at line " NR; hit = 1 }
            grp = 0; next }
        grp {buf = buf $0 "\n"}
        END {exit !hit}' "$wf" >"$dir/hits"; then
    fail "a verdict variable is set inside a pipe, where the step cannot see it: $(cat "$dir/hits")"
fi

exit "$failed"
