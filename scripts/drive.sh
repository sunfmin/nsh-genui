#!/bin/zsh
# scripts/drive.sh <log> <command>...: runs the app with the development controls (src/dev.rs), sends each
# command (or "wait <seconds>") on its stdin, then quits it and prints the results.
# OPEN=1 starts target/GenUI.app through LaunchServices (the login session: the keychain works), else the
# binary directly. GENUI_REPLAY=<file> answers generations with the file instead of DeepSeek.
set -u
cd "${0:A:h}/.."
log=$1; shift
fifo=$(mktemp -u -t genui-fifo)
mkfifo $fifo
if [[ -n ${OPEN:-} ]]; then
  : > $log
  envs=(--env GENUI_DEV=1)
  [[ -n ${GENUI_REPLAY:-} ]] && envs+=(--env GENUI_REPLAY=$GENUI_REPLAY)
  open -n -W $envs --stdin $fifo --stdout $log --stderr $log target/GenUI.app &
else
  GENUI_DEV=1 ./target/release/nsh-genui < $fifo > $log 2>&1 &
fi
pid=$!
exec 3<> $fifo
sleep 3
for c in "$@"; do
  if [[ $c == wait\ * ]]; then sleep ${c#wait }; else print -r -- "$c" >&3; sleep 1; fi
done
exec 3>&-
sleep 1; pkill -x nsh-genui; wait $pid 2>/dev/null
rm -f $fifo
grep -E '^(ok|err) ' $log
