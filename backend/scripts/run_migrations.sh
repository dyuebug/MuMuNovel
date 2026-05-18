#!/bin/bash
# Explicit migration entrypoint for shared-db strangler deployments.

set -e

DB_HOST="${DB_HOST:-postgres}"
DB_PORT="${DB_PORT:-5432}"
DB_USER="${POSTGRES_USER:-mumuai}"
DB_NAME="${POSTGRES_DB:-mumuai_novel}"

echo "================================================"
echo "🔄 Running explicit database migration step..."
echo "================================================"

echo "Waiting for database startup..."
MAX_RETRIES=30
RETRY_COUNT=0
DB_READY_RETRIES="${DB_READY_RETRIES:-45}"
DB_READY_INTERVAL="${DB_READY_INTERVAL:-2}"

wait_for_database_ready() {
    local retries=0
    while ! PGPASSWORD="${POSTGRES_PASSWORD}" psql -h "$DB_HOST" -U "$DB_USER" -d "$DB_NAME" -c "SELECT 1;" > /dev/null 2>&1; do
        retries=$((retries + 1))
        if [ "$retries" -ge "$DB_READY_RETRIES" ]; then
            echo "ERROR: database is still not fully ready after ${DB_READY_RETRIES} probes"
            return 1
        fi
        echo "   Database is not fully ready yet... ($retries/$DB_READY_RETRIES)"
        sleep "$DB_READY_INTERVAL"
    done
    return 0
}

while ! nc -z "$DB_HOST" "$DB_PORT" 2>/dev/null; do
    RETRY_COUNT=$((RETRY_COUNT + 1))
    if [ $RETRY_COUNT -ge $MAX_RETRIES ]; then
        echo "ERROR: database port check timed out (${MAX_RETRIES}s)"
        exit 1
    fi
    echo "   Waiting for database port... ($RETRY_COUNT/$MAX_RETRIES)"
    sleep 1
done

echo "Database port is reachable"
echo "Checking whether the database accepts queries..."
if ! wait_for_database_ready; then
    exit 1
fi

echo "Database is ready"

cd /app

echo "Checking Alembic revision health..."
python tools/check_alembic_revision_health.py

echo "Ensuring Alembic version table capacity..."
python tools/ensure_alembic_version_table_capacity.py

echo "Upgrading database to latest revision..."
if python scripts/migrate.py upgrade head; then
    echo "Database migration completed successfully"
else
    echo "Database migration failed"
    exit 1
fi
