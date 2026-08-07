"""Add durable retry scheduling to novel autopilot runs.

Revision ID: 20260807_autopilot_retry_backoff
Revises: 20260720_audit_actor_id_capacity
Create Date: 2026-08-07 12:00:00
"""

from typing import Sequence, Union

import sqlalchemy as sa
from alembic import op

revision: str = "20260807_autopilot_retry_backoff"
down_revision: Union[str, None] = "20260720_audit_actor_id_capacity"
branch_labels: Union[str, Sequence[str], None] = None
depends_on: Union[str, Sequence[str], None] = None


def upgrade() -> None:
    op.add_column(
        "novel_autopilot_runs",
        sa.Column("next_attempt_at", sa.DateTime(), nullable=True),
    )
    op.create_index(
        "ix_novel_autopilot_runs_status_next_attempt_at",
        "novel_autopilot_runs",
        ["status", "next_attempt_at"],
        unique=False,
    )


def downgrade() -> None:
    op.drop_index(
        "ix_novel_autopilot_runs_status_next_attempt_at",
        table_name="novel_autopilot_runs",
    )
    op.drop_column("novel_autopilot_runs", "next_attempt_at")
