"""Harden regeneration task defaults.

Revision ID: 20260517_regeneration_task_defaults
Revises: 20260517_batch_task_defaults
Create Date: 2026-05-17 14:00:00.000000
"""

from typing import Sequence, Union

from alembic import op
import sqlalchemy as sa


revision: str = '20260517_regeneration_task_defaults'
down_revision: Union[str, None] = '20260517_batch_task_defaults'
branch_labels: Union[str, Sequence[str], None] = None
depends_on: Union[str, Sequence[str], None] = None


def upgrade() -> None:
    op.execute("UPDATE regeneration_tasks SET target_word_count = 3000 WHERE target_word_count IS NULL")
    op.execute("UPDATE regeneration_tasks SET status = 'pending' WHERE status IS NULL")
    op.execute("UPDATE regeneration_tasks SET progress = 0 WHERE progress IS NULL")
    op.execute("UPDATE regeneration_tasks SET version_number = 1 WHERE version_number IS NULL")

    with op.batch_alter_table('regeneration_tasks', schema=None) as batch_op:
        batch_op.alter_column(
            'target_word_count',
            existing_type=sa.Integer(),
            nullable=False,
            server_default=sa.text('3000'),
        )
        batch_op.alter_column(
            'status',
            existing_type=sa.String(length=20),
            nullable=False,
            server_default=sa.text("'pending'"),
        )
        batch_op.alter_column(
            'progress',
            existing_type=sa.Integer(),
            nullable=False,
            server_default=sa.text('0'),
        )
        batch_op.alter_column(
            'version_number',
            existing_type=sa.Integer(),
            nullable=False,
            server_default=sa.text('1'),
        )


def downgrade() -> None:
    with op.batch_alter_table('regeneration_tasks', schema=None) as batch_op:
        batch_op.alter_column(
            'version_number',
            existing_type=sa.Integer(),
            nullable=True,
            server_default=None,
        )
        batch_op.alter_column(
            'progress',
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
            'target_word_count',
            existing_type=sa.Integer(),
            nullable=True,
            server_default=None,
        )
