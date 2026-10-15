#!/usr/bin/env bash
# Reports the scheduled run's state on the gate you DO read.
#
# A scheduled run answers "is what already landed still good against the
# world as it is now", and its whole value is firing while nobody is
# looking — which is exactly when its RESULT arrives. Four seats found
# reds this week they had never opened; one had nine of twelve. The
# sensor worked perfectly and reported to nobody.
#
# A NOTICE, NEVER A REFUSAL, and it exits 0 always. A scheduled red is
# about what already landed; the push in front of you may be its fix, and
# refusing unrelated work would be wrong. It reappears on every run until
# the schedule is green, so unlike a notification it cannot be missed by
# being away.
#
# Three distinct failures, because from inside the sensor they look
# identical:
#
#   not green           a red nobody read
#   declared, no runs   a cron that never fires looks exactly like one passing
#   fired, then died    a schedule that stopped leaves a GREEN last run,
#                       and that happens to a repo that has gone quiet —
#                       precisely when nobody is looking
#
# `--event schedule` is filtered SERVER-side, so `--limit` applies to the
# filtered set and a push flood cannot empty the window. With a bare
# `--limit` a busy repo reports zero scheduled runs, which reads as "no
# cron" — a count is a proxy for time, and it degrades precisely when
# there is most to look at.
#
# The staleness bound is 48h rather than 24h because the fleet measures
# crons landing four to five hours behind their stated time on a shared
# queue. An alarm that fires on healthy lag gets muted, and that is how
# the next real one reaches nobody.

set -uo pipefail
cd "$(dirname "$0")/.."

command -v gh >/dev/null 2>&1 || exit 0

sched=$(gh run list --event schedule --limit 1 \
  --json status,conclusion,createdAt,headSha \
  --jq '.[] | "\(.status) \(.conclusion // "-") \(.createdAt[0:16]) \(.headSha[0:8])"' 2>/dev/null || true)

if [[ -z $sched ]]; then
  if grep -lq 'cron:' .github/workflows/*.yml 2>/dev/null; then
    printf '\n\033[33m▸ notice\033[0m: a cron is declared and NO scheduled run exists yet.\n'
    printf '  A schedule that never fires looks exactly like one that passes.\n'
  fi
  exit 0
fi

created=$(gh run list --event schedule --limit 1 --json createdAt \
  --jq '.[0].createdAt' 2>/dev/null || true)
age_hours=$(AGE_ISO="$created" python3 -c '
import datetime, os
raw = os.environ.get("AGE_ISO", "").strip()
if raw:
    t = datetime.datetime.fromisoformat(raw.replace("Z", "+00:00"))
    now = datetime.datetime.now(datetime.timezone.utc)
    print(int((now - t).total_seconds() // 3600))
' 2>/dev/null || true)

if [[ -n ${age_hours:-} ]] && [[ $age_hours =~ ^[0-9]+$ ]] && ((age_hours > 48)); then
  printf '\n\033[33m▸ notice\033[0m: the last scheduled run is %sh old.\n' "$age_hours"
  printf '  A cron that stopped firing leaves a green last run and looks healthy.\n'
fi

# Four outcomes, not two. A run that was CANCELLED or is still QUEUED is
# not a failure and must not be reported as one: nothing broke, the
# sample was simply never taken. Saying "not green" sends the reader
# hunting a break that does not exist, and the true problem — that the
# daily measurement has silently stopped happening — goes unnamed.
case $sched in
  completed\ success*) ;;
  completed\ cancelled*)
    printf '\n\033[33m▸ notice\033[0m: the last SCHEDULED run was CANCELLED, not failed:\n'
    printf '  %s\n' "$sched"
    printf '  Nothing broke; the sample was never taken. A push to the same ref\n'
    printf '  sharing its concurrency group is the usual cause.\n'
    ;;
  queued*|in_progress*|waiting*|pending*|requested*)
    printf '\n\033[33m▸ notice\033[0m: the last SCHEDULED run has NOT RUN:\n'
    printf '  %s\n' "$sched"
    printf '  A run stuck queued looks alive — it is recent, so no staleness\n'
    printf '  check fires — while producing no measurement at all. If it is\n'
    printf '  hours old, no runner is serving this label.\n'
    ;;
  *)
    printf '\n\033[33m▸ notice\033[0m: the last SCHEDULED run was not green:\n'
    printf '  %s\n' "$sched"
    printf '  It asks whether what already landed still builds against a toolchain\n'
    printf '  that moved overnight, and it reports to nobody.\n'
    printf '  gh run list --event schedule --limit 10\n'
    ;;
esac

exit 0
