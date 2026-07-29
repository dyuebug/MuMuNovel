"""align durable novel autopilot owner id capacity

Revision ID: 20260719_autopilot_user_id_capacity
Revises: 20260719_analysis_content_digest
Create Date: 2026-07-19 17:00:00
"""

from typing import Sequence, Union

import sqlalchemy as sa
from alembic import op

revision: str = "20260719_autopilot_user_id_capacity"
down_revision: Union[str, None] = "20260719_analysis_content_digest"
branch_labels: Union[str, Sequence[str], None] = None
depends_on: Union[str, Sequence[str], None] = None


def upgrade() -> None:
    op.alter_column(
        "novel_autopilot_runs",
        "user_id",
        existing_type=sa.String(length=36),
        type_=sa.String(length=100),
        existing_nullable=False,
    )


def downgrade() -> None:
    op.alter_column(
        "novel_autopilot_runs",
        "user_id",
        existing_type=sa.String(length=100),
        type_=sa.String(length=36),
        existing_nullable=False,
    )
