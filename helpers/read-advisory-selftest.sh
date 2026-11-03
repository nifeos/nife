#!/bin/sh
#
# helpers/read-advisory-selftest.sh: prove script/read-advisory against a stub `gh` and a fake
# advisory fixture. No network, no credential, no real finding.
#
#     helpers/read-advisory-selftest.sh
#
# Cases, each a way the script would silently stop being safe:
#   1. no token on the channel: refused, names the channel, `gh` never called, even with GH_TOKEN
#      and GITHUB_TOKEN set (it never searches).
#   2. a malformed identifier, and two identifiers: refused, `gh` never called.
#   3. a token: reads the right path, hands the token to `gh` as GH_TOKEN, clears GITHUB_TOKEN,
#      prints the fixture's summary and description, never prints the token.
#   4. `gh` fails: the script fails, and its message holds no token.
#   5. standard output a file inside a repository: refused; a file it cannot place: refused; a file
#      outside: accepted; a pipe: accepted.
#   6. nothing is written to the working directory by a read.

set -e
cd "$(dirname "$0")/.."
script="$(pwd)/script/read-advisory"
dir="$(mktemp -d "${TMPDIR:-/tmp}/read-advisory-selftest.XXXXXX")"
trap 'rm -rf "$dir"' EXIT
fail() { echo "read-advisory-selftest: $1" >&2; exit 1; }

mkdir "$dir/bin" "$dir/repo" "$dir/scratch"
git init -q "$dir/repo"

# The fixture is invented text. It holds no seed, input or replay line.
cat >"$dir/fixture.json" <<'JSON'
{"ghsa_id": "GHSA-aaaa-bbbb-cccc", "state": "draft", "severity": "high",
 "summary": "fixture: a invented finding class",
 "description": "FIXTURE-DESCRIPTION line one\nFIXTURE-REPRODUCER-PLACEHOLDER",
 "vulnerabilities": [{"package": {"name": "fixture_crate"}, "vulnerable_version_range": "< 1.0"}]}
JSON

# The stub records how it was called, then answers with the fixture, or fails when told to.
cat >"$dir/bin/gh" <<'STUB'
#!/bin/sh
{
    echo "args: $*"
    echo "GH_TOKEN=${GH_TOKEN-unset}"
    echo "GITHUB_TOKEN=${GITHUB_TOKEN-unset}"
} >>"$STUB_LOG"
if [ -n "${STUB_FAIL-}" ]; then
    echo "stub failure with $GH_TOKEN" >&2
    exit 1
fi
cat "$STUB_FIXTURE"
STUB
chmod +x "$dir/bin/gh"

# A python3 that cannot place standard output, for the unplaceable-file case.
real_python="$(command -v python3)"
mkdir "$dir/nopath"
cat >"$dir/nopath/python3" <<STUB
#!/bin/sh
case "\$*" in *S_ISREG*) exit 1 ;; esac
exec "$real_python" "\$@"
STUB
chmod +x "$dir/nopath/python3"

export STUB_LOG="$dir/gh.log" STUB_FIXTURE="$dir/fixture.json"
export PATH="$dir/bin:$PATH"
: >"$STUB_LOG"

# run <extra env words...> -- <args...>: runs the script from the scratch repo, output in $dir/out
# and $dir/err, exit status in $rc.
run() {
    envs=""
    while [ "$1" != "--" ]; do envs="$envs $1"; shift; done
    shift
    : >"$STUB_LOG"
    # shellcheck disable=SC2086
    (cd "$dir/repo" && env -u NIFE_ADVISORY_READER_TOKEN -u GH_TOKEN -u GITHUB_TOKEN $envs \
        "$script" "$@") >"$dir/out" 2>"$dir/err" && rc=0 || rc=$?
}
gh_calls() { grep -c '^args:' "$STUB_LOG" || true; }

id=GHSA-aaaa-bbbb-cccc
secret=ghs_fixturetoken1234567890

# 1. no token
run -- "$id"
[ "$rc" -ne 0 ] || fail "no token: wanted a refusal"
grep -q NIFE_ADVISORY_READER_TOKEN "$dir/err" || fail "no token: the refusal does not name the channel"
[ "$(gh_calls)" -eq 0 ] || fail "no token: gh was called"
run GH_TOKEN=ghp_other GITHUB_TOKEN=ghp_another -- "$id"
[ "$rc" -ne 0 ] || fail "other credentials: wanted a refusal, the script searched"
[ "$(gh_calls)" -eq 0 ] || fail "other credentials: gh was called"
run NIFE_ADVISORY_READER_TOKEN= GH_TOKEN=ghp_other -- "$id"
[ "$rc" -ne 0 ] || fail "empty token: wanted a refusal"

# 2. identifiers
run NIFE_ADVISORY_READER_TOKEN=$secret -- notanid
[ "$rc" -ne 0 ] && [ "$(gh_calls)" -eq 0 ] || fail "bad identifier: wanted a refusal before gh"
run NIFE_ADVISORY_READER_TOKEN=$secret -- GHSA-aaaa-bbbb-cc/c
[ "$rc" -ne 0 ] && [ "$(gh_calls)" -eq 0 ] || fail "path in identifier: wanted a refusal before gh"
run NIFE_ADVISORY_READER_TOKEN=$secret -- "$id" GHSA-dddd-eeee-ffff
[ "$rc" -ne 0 ] && [ "$(gh_calls)" -eq 0 ] || fail "two identifiers: wanted a refusal before gh"

# 3. a read
run NIFE_ADVISORY_READER_TOKEN=$secret GITHUB_TOKEN=ghp_another -- "$id"
[ "$rc" -eq 0 ] || fail "read: wanted success, got $rc: $(cat "$dir/err")"
grep -q "/repos/nifeos/nife/security-advisories/$id" "$STUB_LOG" || fail "read: wrong path"
grep -q "^GH_TOKEN=$secret\$" "$STUB_LOG" || fail "read: the token did not reach gh as GH_TOKEN"
grep -q '^GITHUB_TOKEN=unset$' "$STUB_LOG" || fail "read: GITHUB_TOKEN was left set for gh"
grep -q 'FIXTURE-DESCRIPTION' "$dir/out" || fail "read: the description was not printed"
grep -q 'summary: fixture' "$dir/out" || fail "read: the summary was not printed"
grep -q 'affected: fixture_crate' "$dir/out" || fail "read: the affected package was not printed"
! grep -q "$secret" "$dir/out" "$dir/err" || fail "read: the token was printed"

# 4. gh fails
run NIFE_ADVISORY_READER_TOKEN=$secret STUB_FAIL=1 -- "$id"
[ "$rc" -ne 0 ] || fail "gh failure: wanted a failure"
! grep -q "$secret" "$dir/out" "$dir/err" || fail "gh failure: the token was printed"

# 5. where standard output points
(cd "$dir/repo" && env -u GH_TOKEN NIFE_ADVISORY_READER_TOKEN=$secret "$script" "$id" >"$dir/repo/advisory.txt" 2>"$dir/err") \
    && fail "a file in the repository: wanted a refusal" || true
grep -q 'inside' "$dir/err" || fail "a file in the repository: the refusal does not say why"
[ ! -s "$dir/repo/advisory.txt" ] || fail "a file in the repository: text was written"
(cd "$dir/repo" && env -u GH_TOKEN NIFE_ADVISORY_READER_TOKEN=$secret "$script" "$id" >"$dir/scratch/advisory.txt" 2>"$dir/err") \
    || fail "a file outside every repository: wanted success"
grep -q 'FIXTURE-DESCRIPTION' "$dir/scratch/advisory.txt" || fail "a file outside: no text"
(cd "$dir/repo" && env -u GH_TOKEN NIFE_ADVISORY_READER_TOKEN=$secret "$script" "$id" 2>/dev/null | grep -q FIXTURE-DESCRIPTION) \
    || fail "a pipe: wanted success"

(cd "$dir/repo" && env -u GH_TOKEN PATH="$dir/nopath:$PATH" NIFE_ADVISORY_READER_TOKEN=$secret "$script" "$id" >"$dir/scratch/unplaced.txt" 2>"$dir/err") \
    && fail "an unplaceable file: wanted a refusal" || true
grep -q 'cannot place' "$dir/err" || fail "an unplaceable file: the refusal does not say why"
[ ! -s "$dir/scratch/unplaced.txt" ] || fail "an unplaceable file: text was written"

# 6. nothing written to the working directory
run NIFE_ADVISORY_READER_TOKEN=$secret -- "$id"
[ -z "$(ls -A "$dir/repo" | grep -v -e '^advisory.txt$' -e '^.git$')" ] || fail "a read left files in the working directory"
[ -z "$(git -C "$dir/repo" status --porcelain | grep -v advisory.txt)" ] || fail "a read dirtied the repository"

echo "read-advisory-selftest: ok"
