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
    op.execute("UPDATE batch_generation_tasks SET enable_analysis = 0 WHERE enable_analysis IS NULL")
    op.execute("UPDATE batch_generation_tasks SET status = 'pending' WHERE status IS NULL")
    op.execute("UPDATE batch_generation_tasks SET total_chapters = 0 WHERE total_chapters IS NULL")
    op.execute("UPDATE batch_generation_tasks SET completed_chapters = 0 WHERE completed_chapters IS NULL")
    op.execute("UPDATE batch_generation_tasks SET failed_chapters = '[]' WHERE failed_chapters IS NULL")
    op.execute("UPDATE batch_generation_tasks SET current_retry_count = 0 WHERE current_retry_count IS NULL")
    op.execute("UPDATE batch_generation_tasks SET max_retries = 3 WHERE max_retries IS NULL")

    with op.batch_alter_table('batch_generation_tasks', schema=None) as batch_op:
        batch_op.alter_column(
            'target_word_count',
            existing_type=sa.Integer(),
            nullable=False,
            server_default=sa.text('3000'),
        )
        batch_op.alter_column(
            'enable_analysis',
            existing_type=sa.Boolean(),
            nullable=False,
            server_default=sa.text('0'),
        )
        batch_op.alter_column(
            'status',
            existing_type=sa.String(length=20),
            nullable=False,
            server_default=sa.text("'pending'"),
        )
        batch_op.alter_column(
            'total_chapters',
            existing_type=sa.Integer(),
            nullable=False,
            server_default=sa.text('0'),
        )
        batch_op.alter_column(
            'completed_chapters',
            existing_type=sa.Integer(),
            nullable=False,
            server_default=sa.text('0'),
        )
        batch_op.alter_column(
            'failed_chapters',
            existing_type=sa.JSON(),
            nullable=False,
            server_default=sa.text("'[]'"),
        )
        batch_op.alter_column(
            'current_retry_count',
            existing_type=sa.Integer(),
            nullable=False,
            server_default=sa.text('0'),
        )
        batch_op.alter_column(
            'max_retries',
            existing_type=sa.Integer(),
            nullable=False,
            server_default=sa.text('3'),
        )


def downgrade() -> None:
    with op.batch_alter_table('batch_generation_tasks', schema=None) as batch_op:
        batch_op.alter_column(
            'max_retries',
            existing_type=sa.Integer(),
            nullable=True,
            server_default=None,
        )
        batch_op.alter_column(
            'current_retry_count',
            existing_type=sa.Integer(),
            nullable=True,
            server_default=None,
        )
        batch_op.alter_column(
            'completed_chapters',
            existing_type=sa.Integer(),
            nullable=True,
            server_default=None,
        )
        batch_op.alter_column(
            'failed_chapters',
            existing_type=sa.JSON(),
            nullable=True,
            server_default=None,
        )
        batch_op.alter_column(
            'total_chapters',
            existing_type=sa.Integer(),
            nullable=True,
            server_default=None,
        )
        batch_op.alter_column(
            'status',
            existing_type=sa.String(length=20),
            nullable=True,
            server_default=None,
        )
        batch_op.alter_column(
            'enable_analysis',
            existing_type=sa.Boolean(),
            nullable=True,
            server_default=None,
        )
        batch_op.alter_column(
            'target_word_count',
            existing_type=sa.Integer(),
            nullable=True,
            server_default=None,
        )
