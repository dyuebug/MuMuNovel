"""Expand autopilot invocation audit actor user id capacity.

Revision ID: 20260720_audit_actor_id_capacity
Revises: 20260719_autopilot_user_id_capacity
Create Date: 2026-07-20 09:00:00
"""

from typing import Sequence, Union

import sqlalchemy as sa
from alembic import op

revision: str = "20260720_audit_actor_id_capacity"
down_revision: Union[str, None] = "20260719_autopilot_user_id_capacity"
branch_labels: Union[str, Sequence[str], None] = None
depends_on: Union[str, Sequence[str], None] = None


def upgrade() -> None:
    op.alter_column(
        "autopilot_invocation_audits",
        "actor_user_id",
        existing_type=sa.String(length=36),
        type_=sa.String(length=100),
        existing_nullable=False,
    )


def downgrade() -> None:
    op.alter_column(
        "autopilot_invocation_audits",
        "actor_user_id",
        existing_type=sa.String(length=100),
        type_=sa.String(length=36),
        existing_nullable=False,
    )
