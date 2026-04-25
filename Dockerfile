# 多阶段构建 Dockerfile for AI Story Creator
# 支持多架构构建: linux/amd64, linux/arm64

# 构建参数
ARG USE_CN_MIRROR=false
ARG SKIP_FRONTEND_BUILD=false
ARG HTTP_PROXY
ARG HTTPS_PROXY
ARG NO_PROXY
ARG http_proxy
ARG https_proxy
ARG no_proxy

# 阶段1: 构建前端
FROM node:22-alpine AS frontend-builder

ARG USE_CN_MIRROR
ARG SKIP_FRONTEND_BUILD
ARG HTTP_PROXY
ARG HTTPS_PROXY
ARG NO_PROXY
ARG http_proxy
ARG https_proxy
ARG no_proxy

WORKDIR /frontend

# 复制前端依赖文件
COPY frontend/package*.json ./

# 根据参数决定是否使用国内npm镜像，并增强网络容错
RUN if [ "$USE_CN_MIRROR" = "true" ]; then \
        npm config set registry https://registry.npmmirror.com; \
    fi && \
    npm config set fetch-retries 5 && \
    npm config set fetch-retry-mintimeout 20000 && \
    npm config set fetch-retry-maxtimeout 120000

# 安装依赖（可跳过）
RUN if [ "$SKIP_FRONTEND_BUILD" != "true" ]; then npm ci && npm rebuild esbuild; fi

# 复制前端源代码
COPY frontend/ ./

# 提供后端静态资源作为跳过前端构建时的兜底
COPY backend/static/ /frontend/dist/

# 通过环境变量指定输出目录，避免在构建时修改源码配置
RUN if [ "$SKIP_FRONTEND_BUILD" != "true" ]; then \
        VITE_OUT_DIR=dist npm run build; \
    fi

# 阶段2: 构建最终镜像
FROM python:3.11-slim

ARG USE_CN_MIRROR
ARG TARGETPLATFORM
ARG TARGETARCH
ARG HTTP_PROXY
ARG HTTPS_PROXY
ARG NO_PROXY
ARG http_proxy
ARG https_proxy
ARG no_proxy

# 设置工作目录
WORKDIR /app

# ?? Debian ?? HTTPS?????????????
RUN set -eux; \
    sed -i 's|http://deb.debian.org|https://deb.debian.org|g' /etc/apt/sources.list.d/debian.sources; \
    sed -i 's|http://security.debian.org|https://security.debian.org|g' /etc/apt/sources.list.d/debian.sources; \
    if [ "$USE_CN_MIRROR" = "true" ]; then \
        sed -i 's|deb.debian.org|mirrors.aliyun.com|g' /etc/apt/sources.list.d/debian.sources; \
        sed -i 's|security.debian.org|mirrors.aliyun.com|g' /etc/apt/sources.list.d/debian.sources; \
    fi

# ????????????????????????
RUN set -eux; \
    apt_install_cmd='apt-get update -o Acquire::Retries=5 -o Acquire::http::Timeout=30 -o Acquire::https::Timeout=30 && apt-get install -y --no-install-recommends gcc postgresql-client netcat-traditional'; \
    if ! sh -c "$apt_install_cmd"; then \
        echo 'Primary Debian mirror failed, switching to Aliyun mirror and retrying...' >&2; \
        sed -i 's|https://deb.debian.org|https://mirrors.aliyun.com|g' /etc/apt/sources.list.d/debian.sources; \
        sed -i 's|https://security.debian.org|https://mirrors.aliyun.com|g' /etc/apt/sources.list.d/debian.sources; \
        sed -i 's|deb.debian.org|mirrors.aliyun.com|g' /etc/apt/sources.list.d/debian.sources; \
        sed -i 's|security.debian.org|mirrors.aliyun.com|g' /etc/apt/sources.list.d/debian.sources; \
        sh -c "$apt_install_cmd"; \
    fi; \
    rm -rf /var/lib/apt/lists/*

# 复制后端依赖文件
COPY backend/requirements.txt ./

# ???? PyTorch CPU ???????????
RUN set -eux; \
    torch_cpu_cmd='pip install --no-cache-dir --default-timeout=180 --retries 5 torch --index-url https://download.pytorch.org/whl/cpu'; \
    torch_default_cmd='pip install --no-cache-dir --default-timeout=180 --retries 5 torch'; \
    if [ "$TARGETARCH" = "arm64" ]; then \
        sh -c "$torch_cpu_cmd" || sh -c "$torch_default_cmd"; \
    else \
        sh -c "$torch_cpu_cmd" || sh -c "$torch_default_cmd"; \
    fi

# ?? Python ???????????????? PyPI
RUN set -eux; \
    pip_install_base='pip install --no-cache-dir --default-timeout=180 --retries 5 -r requirements.txt'; \
    if [ "$USE_CN_MIRROR" = "true" ]; then \
        sh -c "$pip_install_base -i https://mirrors.aliyun.com/pypi/simple/" || { \
            echo 'Aliyun PyPI mirror failed, retrying with default PyPI...' >&2; \
            sh -c "$pip_install_base"; \
        }; \
    else \
        sh -c "$pip_install_base"; \
    fi

# 创建 embedding 目录并预置本地模型（源码构建场景）
RUN mkdir -p /app/embedding

# 设置 Sentence-Transformers 缓存目录
ENV SENTENCE_TRANSFORMERS_HOME=/app/embedding

# 若本地已提供 embedding 模型，则直接复用；README 已要求源码构建前先准备该目录
COPY backend/embedding/ ./embedding/

# 复制后端代码
COPY backend/ ./

# 优先使用本地 embedding，缺失时再联网下载
RUN set -eux; \
    export SENTENCE_TRANSFORMERS_HOME=/app/embedding; \
    if [ "$USE_CN_MIRROR" = "true" ]; then \
        export HF_ENDPOINT=https://hf-mirror.com; \
        echo "Using HF mirror: $HF_ENDPOINT"; \
    fi; \
    python - <<'PY'
from sentence_transformers import SentenceTransformer
import os
import time

model_name = 'sentence-transformers/paraphrase-multilingual-MiniLM-L12-v2'
cache_dir = '/app/embedding'
model_dir = os.path.join(cache_dir, 'models--sentence-transformers--paraphrase-multilingual-MiniLM-L12-v2')

if os.path.isdir(model_dir):
    print(f'Using preloaded embedding model from {model_dir}')
    raise SystemExit(0)

print('Preloaded embedding model not found, downloading from HuggingFace...')
last_error = None
for attempt in range(1, 4):
    try:
        print(f'Downloading {model_name} (attempt {attempt}/3)...')
        SentenceTransformer(
            model_name,
            cache_folder=cache_dir,
            device='cpu',
            trust_remote_code=True,
            local_files_only=False,
        )
        print('Model downloaded successfully!')
        last_error = None
        break
    except Exception as exc:
        last_error = exc
        print(f'Download attempt {attempt} failed: {exc!r}')
        if attempt < 3:
            time.sleep(5)

if last_error is not None:
    raise SystemExit(
        'Embedding model download failed after 3 attempts. '
        'Please place the model under backend/embedding/ before redeploy.'
    ) from last_error
PY

# 从前端构建阶段复制构建好的静态文件
COPY --from=frontend-builder /frontend/dist ./static

# 复制 Alembic 迁移配置和脚本（PostgreSQL）
COPY backend/alembic-postgres.ini ./alembic.ini
COPY backend/alembic/postgres ./alembic
COPY backend/scripts/entrypoint.sh /app/entrypoint.sh
COPY backend/scripts/migrate.py ./scripts/migrate.py

# 赋予执行权限
RUN chmod +x /app/entrypoint.sh

# 创建必要的目录
RUN mkdir -p /app/data /app/logs

# 暴露端口
EXPOSE 8000

# 设置环境变量
ENV PYTHONUNBUFFERED=1
ENV APP_HOST=0.0.0.0
ENV APP_PORT=8000

# 设置运行时为离线模式（模型已在构建时下载）
ENV TRANSFORMERS_OFFLINE=1
ENV HF_DATASETS_OFFLINE=1
ENV HF_HUB_OFFLINE=1

# 健康检查
HEALTHCHECK --interval=30s --timeout=10s --start-period=5s --retries=3 \
    CMD python -c "import urllib.request; urllib.request.urlopen('http://localhost:8000/readyz')" || exit 1

# 使用 entrypoint 脚本启动（自动执行迁移）
ENTRYPOINT ["/app/entrypoint.sh"]
