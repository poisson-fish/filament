#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 1 ]]; then
  echo "usage: $0 <backup_dir>" >&2
  exit 1
fi

BACKUP_DIR="$1"
if [[ ! -d "${BACKUP_DIR}" ]]; then
  echo "backup directory not found: ${BACKUP_DIR}" >&2
  exit 1
fi

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"
COMPOSE_FILE="${COMPOSE_FILE:-${REPO_ROOT}/infra/docker-compose.yml}"
COMPOSE_PROJECT="${COMPOSE_PROJECT:-filament}"
POSTGRES_SERVICE="${POSTGRES_SERVICE:-postgres}"
SERVER_SERVICE="${SERVER_SERVICE:-filament-server}"
POSTGRES_USER="${POSTGRES_USER:-filament}"
POSTGRES_DB="${POSTGRES_DB:-filament}"
ATTACHMENT_ROOT="${FILAMENT_ATTACHMENT_ROOT:-/var/lib/filament/attachments}"

for artifact in postgres.dump attachments.tar.gz SHA256SUMS; do
  if [[ ! -f "${BACKUP_DIR}/${artifact}" ]]; then
    echo "missing artifact: ${BACKUP_DIR}/${artifact}" >&2
    exit 1
  fi
done

compose() {
  docker compose -p "${COMPOSE_PROJECT}" -f "${COMPOSE_FILE}" "$@"
}

echo "[restore] verifying backup checksums"
( cd "${BACKUP_DIR}" && shasum -a 256 -c SHA256SUMS )

echo "[restore] pausing write-path services"
compose stop "${SERVER_SERVICE}"

echo "[restore] restoring postgres"
compose exec -T "${POSTGRES_SERVICE}" \
  pg_restore --clean --if-exists --no-owner --no-privileges \
  -U "${POSTGRES_USER}" -d "${POSTGRES_DB}" \
  < "${BACKUP_DIR}/postgres.dump"

echo "[restore] restoring attachments"
compose exec -T "${SERVER_SERVICE}" sh -lc \
  "mkdir -p '${ATTACHMENT_ROOT}' && rm -rf '${ATTACHMENT_ROOT:?}'/*"
compose exec -T "${SERVER_SERVICE}" sh -lc \
  "tar -xzf - -C '${ATTACHMENT_ROOT}'" \
  < "${BACKUP_DIR}/attachments.tar.gz"

echo "[restore] restarting services"
compose up -d "${SERVER_SERVICE}"

echo "[restore] complete"
echo "[restore] next: run search rebuild and reconcile per guild"
