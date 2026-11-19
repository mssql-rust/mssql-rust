#!/bin/sh
set -eu

report="${1:-target/tarpaulin-live/lcov.info}"

awk -F '[:,]' '
  /^DA:/ {
    line_total += 1
    if (($3 + 0) > 0) line_covered += 1
  }
  /^FNDA:/ {
    function_total += 1
    if (($2 + 0) > 0) function_covered += 1
  }
  END {
    if (line_total == 0 || function_total == 0) {
      print "coverage: no DA or FNDA records found" > "/dev/stderr"
      exit 1
    }

    line_percentage = line_covered * 100 / line_total
    function_percentage = function_covered * 100 / function_total
    printf "line coverage: %d/%d (%.2f%%)\n", line_covered, line_total, line_percentage
    printf "function coverage: %d/%d (%.2f%%)\n", function_covered, function_total, function_percentage

    # Both gates are strictly greater than 95%, not greater-than-or-equal.
    if (line_covered * 100 <= line_total * 95) exit 1
    if (function_covered * 100 <= function_total * 95) exit 1
  }
' "$report"
