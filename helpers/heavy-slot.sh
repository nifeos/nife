# helpers/heavy-slot.sh: take one of the machine's heavy-job slots for the rest of this script.
#
#     . "$(dirname "$0")/../helpers/heavy-slot.sh"
#
# Sourced, so ShellCheck is told the dialect.
# shellcheck shell=sh
#
# Sourced as the first thing a heavy script does, before it changes directory, because it
# re-executes "$0" and a relative "$0" only resolves from the directory the caller started in.
#
# When no ancestor holds a slot, the script is re-run under helpers/heavy_slot.py, which waits for a
# slot, holds it on an inherited descriptor and execs the script again with the same pid and
# arguments. On that second pass the script's own pid is the holder, so the check below passes and
# it goes on. A nested heavy script passes the same check through its parent and runs in place,
# never re-executed: `script/lint --no-cargo` sources its own preamble again inside `sh -c`, where
# "$0" is not the file being run. The job a waiter sees named is the script's own file name.
#
# Name: provisional, minted 2026-10-19 (UTC) by lane/heavy-job-slots, like heavy_slot.py beside it,
# which says why the slots exist. NIFE_HEAVY_SLOTS=<n> sets how many there are.

# Under LINT_NO_CARGO (`script/lint --no-cargo`'s sections) a heavy script runs against a stub cargo
# and takes no slot, for the reason script/lint's preamble gives for that mode taking none: a wait
# there is idle time on the push connection git holds open.
if [ -z "${LINT_NO_CARGO:-}" ] && ! python3 "$(dirname "$0")/../helpers/heavy_slot.py" --held; then
    exec python3 "$(dirname "$0")/../helpers/heavy_slot.py" "$(basename "$0")" -- "$0" "$@"
fi
