#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 2 ]]; then
  echo "usage: $0 <metrics_before.txt> <metrics_after.txt>" >&2
  exit 1
fi

BEFORE_FILE="$1"
AFTER_FILE="$2"

for file in "${BEFORE_FILE}" "${AFTER_FILE}"; do
  if [[ ! -f "${file}" ]]; then
    echo "metrics file not found: ${file}" >&2
    exit 1
  fi
done

counter_value() {
  local file="$1"
  local metric="$2"

  awk -v metric="${metric}" '
    BEGIN { found = 0 }
    $0 ~ /^#/ { next }
    {
      split($0, parts, " ")
      if (parts[1] == metric) {
        print parts[2]
        found = 1
        exit
      }
    }
    END {
      if (found == 0) {
        print 0
      }
    }
  ' "${file}"
}

check_delta() {
  local metric="$1"
  local before
  local after
  before="$(counter_value "${BEFORE_FILE}" "${metric}")"
  after="$(counter_value "${AFTER_FILE}" "${metric}")"

  if [[ "${after}" -le "${before}" ]]; then
    echo "[FAIL] ${metric} did not increase: before=${before} after=${after}" >&2
    return 1
  fi

  echo "[OK]   ${metric} increased: before=${before} after=${after}"
}

check_delta 'filament_gateway_events_unknown_received_total{scope="ingress",event_type="unknown_ingress_event"}'
check_delta 'filament_gateway_events_parse_rejected_total{scope="ingress",reason="invalid_envelope"}'
check_delta 'filament_gateway_events_parse_rejected_total{scope="ingress",reason="invalid_subscribe_payload"}'
check_delta 'filament_gateway_events_parse_rejected_total{scope="ingress",reason="invalid_message_create_payload"}'
check_delta 'filament_gateway_events_dropped_total{scope="channel",event_type="message_create",reason="oversized_outbound"}'

echo "[PASS] gateway staging telemetry verification passed"
