#!/bin/sh
# Runs every test with the ris compiler named by $RIS (default: ris).
set -eu
cd "$(dirname "$0")"
RIS="${RIS:-ris}"
export IYI_PATH="$PWD:$("$RIS" env IYI_PATH)"
"$RIS" fmt --check htmlparse/*.ri
"$RIS" test
