"""Harden batch generation task defaults.

Revision ID: 20260517_batch_task_defaults
Revises: 20260517_analysis_task_hardening
Create Date: 2026-05-17 13:00:00.000000
"""

from typing import Sequence, Union

from alembic import op
import sqlalchemy as sa


revision: str = '20260517_batch_task_defaults'
down_revision: Union[str, None] = '20260517_analysis_task_hardening'
branch_labels: Union[str, Sequence[str], None] = None
depends_on: Union[str, Sequence[str], None] = None


def upgrade() -> None:
    op.execute("UPDATE batch_generation_tasks SET target_word_count = 3000 WHERE target_word_count IS NULL")
    op.execute("UPDATE batch_generation_tasks SET enable_analysis = false WHERE enable_analysis IS NULL")
    op.execute("UPDATE batch_generation_tasks SET status = 'pending' WHERE status IS NULL")
    op.execute("UPDATE batch_generation_tasks SET total_chapters = 0 WHERE total_chapters IS NULL")
    op.execute("UPDATE batch_generation_tasks SET completed_chapters = 0 WHERE completed_chapters IS NULL")
    op.execute("UPDATE batch_generation_tasks SET failed_chapters = '[]'::json WHERE failed_chapters IS NULL")
    op.execute("UPDATE batch_generation_tasks SET current_retry_count = 0 WHERE current_retry_count IS NULL")
    op.execute("UPDATE batch_generation_tasks SET max_retries = 3 WHERE max_retries IS NULL")

    op.alter_column(
        'batch_generation_tasks',
        'target_word_count',
        existing_type=sa.Integer(),
        nullable=False,
        server_default=sa.text('3000'),
    )
    op.alter_column(
        'batch_generation_tasks',
        'enable_analysis',
        existing_type=sa.Boolean(),
        nullable=False,
        server_default=sa.text('false'),
    )
    op.alter_column(
        'batch_generation_tasks',
        'status',
        existing_type=sa.String(length=20),
        nullable=False,
        server_default=sa.text("'pending'"),
    )
    op.alter_column(
        'batch_generation_tasks',
        'total_chapters',
        existing_type=sa.Integer(),
        nullable=False,
        server_default=sa.text('0'),
    )
    op.alter_column(
        'batch_generation_tasks',
        'completed_chapters',
        existing_type=sa.Integer(),
        nullable=False,
        server_default=sa.text('0'),
    )
    op.alter_column(
        'batch_generation_tasks',
        'failed_chapters',
        existing_type=sa.JSON(),
        nullable=False,
        server_default=sa.text("'[]'::json"),
    )
    op.alter_column(
        'batch_generation_tasks',
        'current_retry_count',
        existing_type=sa.Integer(),
        nullable=False,
        server_default=sa.text('0'),
    )
    op.alter_column(
        'batch_generation_tasks',
        'max_retries',
        existing_type=sa.Integer(),
        nullable=False,
        server_default=sa.text('3'),
    )


def downgrade() -> None:
    op.alter_column(
        'batch_generation_tasks',
        'max_retries',
        existing_type=sa.Integer(),
        nullable=True,
        server_default=None,
    )
    op.alter_column(
        'batch_generation_tasks',
        'current_retry_count',
        existing_type=sa.Integer(),
        nullable=True,
        server_default=None,
    )
    op.alter_column(
        'batch_generation_tasks',
        'completed_chapters',
        existing_type=sa.Integer(),
        nullable=True,
        server_default=None,
    )
    op.alter_column(
        'batch_generation_tasks',
        'failed_chapters',
        existing_type=sa.JSON(),
        nullable=True,
        server_default=None,
    )
    op.alter_column(
        'batch_generation_tasks',
        'total_chapters',
        existing_type=sa.Integer(),
        nullable=True,
        server_default=None,
    )
    op.alter_column(
        'batch_generation_tasks',
        'status',
        existing_type=sa.String(length=20),
        nullable=True,
        server_default=None,
    )
    op.alter_column(
        'batch_generation_tasks',
        'enable_analysis',
        existing_type=sa.Boolean(),
        nullable=True,
        server_default=None,
    )
    op.alter_column(
        'batch_generation_tasks',
        'target_word_count',
        existing_type=sa.Integer(),
        nullable=True,
        server_default=None,
    )
