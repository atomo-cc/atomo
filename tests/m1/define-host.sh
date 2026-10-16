#!/bin/sh
set -eu
set -a
. tests/m1/.env
set +a
M1_DATABASE="m1_define_${1:-full}_$(date +%s)"
export M1_DATABASE
printf '%s\n' "$M1_DATABASE" >> tests/m1/define-databases.txt
docker compose -f tests/m1/compose.yaml exec -T -e M1_PASSWORD -e M1_DATABASE db sh < tests/m1/init-db.sh > tests/m1/define-init.log 2>&1
docker compose -f tests/m1/compose.yaml run --rm rust bash tests/m1/define-checks.sh "${1:-full}"
