#!/bin/sh
set -eu

report="${1:-target/tarpaulin-live/lcov.info}"

awk -F '[:,]' '
  /^FNDA:/ {
    total += 1
    if (($2 + 0) > 0) covered += 1
  }
  END {
    if (total == 0) {
      print "function coverage: no FNDA records found" > "/dev/stderr"
      exit 1
    }

    percentage = covered * 100 / total
    printf "function coverage: %d/%d (%.2f%%)\n", covered, total, percentage

    # The project gate is strictly greater than 95%, not greater-than-or-equal.
    if (covered * 100 <= total * 95) exit 1
  }
' "$report"
