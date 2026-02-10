#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"
COMPOSE_FILE="${COMPOSE_FILE:-${REPO_ROOT}/infra/docker-compose.yml}"
COMPOSE_PROJECT="${COMPOSE_PROJECT:-filament}"
POSTGRES_SERVICE="${POSTGRES_SERVICE:-postgres}"
SERVER_SERVICE="${SERVER_SERVICE:-filament-server}"
POSTGRES_USER="${POSTGRES_USER:-filament}"
POSTGRES_DB="${POSTGRES_DB:-filament}"
ATTACHMENT_ROOT="${FILAMENT_ATTACHMENT_ROOT:-/var/lib/filament/attachments}"

TIMESTAMP="$(date -u +%Y%m%dT%H%M%SZ)"
BACKUP_ROOT="${1:-${REPO_ROOT}/backups}"
BACKUP_DIR="${BACKUP_ROOT}/${TIMESTAMP}"
mkdir -p "${BACKUP_DIR}"

compose() {
  docker compose -p "${COMPOSE_PROJECT}" -f "${COMPOSE_FILE}" "$@"
}

echo "[backup] writing backup to ${BACKUP_DIR}"

compose exec -T "${POSTGRES_SERVICE}" \
  pg_dump -U "${POSTGRES_USER}" -d "${POSTGRES_DB}" -Fc \
  > "${BACKUP_DIR}/postgres.dump"

compose exec -T "${SERVER_SERVICE}" sh -lc \
  "test -d '${ATTACHMENT_ROOT}' && tar -C '${ATTACHMENT_ROOT}' -czf - ." \
  > "${BACKUP_DIR}/attachments.tar.gz"

( cd "${BACKUP_DIR}" && shasum -a 256 postgres.dump attachments.tar.gz > SHA256SUMS )

echo "[backup] complete"
echo "[backup] artifacts:"
echo "  - ${BACKUP_DIR}/postgres.dump"
echo "  - ${BACKUP_DIR}/attachments.tar.gz"
echo "  - ${BACKUP_DIR}/SHA256SUMS"
