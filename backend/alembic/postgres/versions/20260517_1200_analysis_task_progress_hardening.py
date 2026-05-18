"""Harden analysis task status/progress defaults.

Revision ID: 20260517_analysis_task_hardening
Revises: 20260325_batch_workflow_state
Create Date: 2026-05-17 12:00:00.000000
"""

from typing import Sequence, Union

from alembic import op
import sqlalchemy as sa


revision: str = '20260517_analysis_task_hardening'
down_revision: Union[str, None] = '20260325_batch_workflow_state'
branch_labels: Union[str, Sequence[str], None] = None
depends_on: Union[str, Sequence[str], None] = None


def upgrade() -> None:
    op.execute("UPDATE analysis_tasks SET progress = 0 WHERE progress IS NULL")
    op.execute("UPDATE analysis_tasks SET status = 'pending' WHERE status IS NULL")

    op.alter_column(
        'analysis_tasks',
        'progress',
        existing_type=sa.Integer(),
        nullable=False,
        server_default=sa.text('0'),
    )
    op.alter_column(
        'analysis_tasks',
        'status',
        existing_type=sa.String(length=20),
        nullable=False,
        server_default=sa.text("'pending'"),
    )


def downgrade() -> None:
    op.alter_column(
        'analysis_tasks',
        'status',
        existing_type=sa.String(length=20),
        nullable=False,
        server_default=None,
    )
    op.alter_column(
        'analysis_tasks',
        'progress',
        existing_type=sa.Integer(),
        nullable=True,
        server_default=None,
    )
