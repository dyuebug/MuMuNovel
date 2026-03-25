#!/bin/bash
# Docker 容器启动入口脚本
# 功能：等待数据库就绪，执行迁移，启动应用

set -e  # 遇到错误立即退出

# 获取版本信息(从 .env.example 文件)
# 如果环境变量未设置，则从 .env.example 读取
if [ -z "$APP_VERSION" ]; then
    if [ -f "/app/.env.example" ]; then
        APP_VERSION=$(grep "^APP_VERSION=" /app/.env.example | cut -d '=' -f2)
    fi
    APP_VERSION="${APP_VERSION:-1.0.0}"
fi

if [ -z "$APP_NAME" ]; then
    if [ -f "/app/.env.example" ]; then
        APP_NAME=$(grep "^APP_NAME=" /app/.env.example | cut -d '=' -f2)
    fi
    APP_NAME="${APP_NAME:-MuMuNovel}"
fi

BUILD_TIME=$(date '+%Y-%m-%d %H:%M:%S')

echo "================================================"
echo "🚀 ${APP_NAME} 启动中..."
echo "📦 版本: v${APP_VERSION}"
echo "🕐 启动时间: ${BUILD_TIME}"
echo "================================================"

# 数据库配置（从环境变量读取）
DB_HOST="${DB_HOST:-postgres}"
DB_PORT="${DB_PORT:-5432}"
DB_USER="${POSTGRES_USER:-mumuai}"
DB_NAME="${POSTGRES_DB:-mumuai_novel}"

# Wait for database readiness
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

# 运行数据库迁移
echo "================================================"
echo "🔄 执行数据库迁移..."
echo "================================================"

cd /app

echo "Checking Alembic revision health..."
python tools/check_alembic_revision_health.py

echo "Ensuring Alembic version table capacity..."
python tools/ensure_alembic_version_table_capacity.py

# Use alembic upgrade head for both bootstrap and incremental migrations
# Alembic handles initial deployment and incremental upgrades automatically
echo "Upgrading database to latest revision..."
if python scripts/migrate.py upgrade head; then
    echo "Database migration completed successfully"
else
    echo "Database migration failed"
    exit 1
fi

echo "================================================"
echo "🎉 启动应用服务..."
echo "================================================"

# 启动应用（使用 exec 替换当前进程，确保信号正确传递）
cd /app
exec uvicorn app.main:app \
    --host "${APP_HOST:-0.0.0.0}" \
    --port "${APP_PORT:-8000}" \
    --log-level info \
    --access-log \
    --use-colors
