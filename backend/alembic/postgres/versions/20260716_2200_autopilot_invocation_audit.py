"""Add durable autopilot invocation audit records.

Revision ID: 20260716_autopilot_invocation_audit
Revises: 20260712_password_hash_phc_text
Create Date: 2026-07-16 22:00:00.000000
"""

from typing import Sequence, Union

from alembic import op
import sqlalchemy as sa


revision: str = "20260716_autopilot_invocation_audit"
down_revision: Union[str, None] = "20260712_password_hash_phc_text"
branch_labels: Union[str, Sequence[str], None] = None
depends_on: Union[str, Sequence[str], None] = None


def upgrade() -> None:
    op.create_table(
        "autopilot_invocation_audits",
        sa.Column("id", sa.String(length=36), nullable=False),
        sa.Column("task_id", sa.String(length=36), nullable=False),
        sa.Column("project_id", sa.String(length=36), nullable=False),
        sa.Column("actor_user_id", sa.String(length=36), nullable=False),
        sa.Column("schema_version", sa.String(length=64), nullable=False),
        sa.Column("tool_name", sa.String(length=128), nullable=False),
        sa.Column("tool_schema_version", sa.String(length=64), nullable=False),
        sa.Column("confirmed_by_user", sa.Boolean(), nullable=False),
        sa.Column("execution_mode", sa.String(length=64), nullable=False),
        sa.Column("provider_name", sa.Text(), nullable=True),
        sa.Column("model_name", sa.Text(), nullable=True),
        sa.Column("prompt_digest", sa.String(length=80), nullable=True),
        sa.Column("input_digest", sa.String(length=80), nullable=False),
        sa.Column("input_summary", sa.Text(), nullable=False),
        sa.Column("status", sa.String(length=32), nullable=False),
        sa.Column("result_summary", sa.Text(), nullable=True),
        sa.Column("error_code", sa.String(length=128), nullable=True),
        sa.Column("created_at", sa.DateTime(), nullable=False),
        sa.Column("started_at", sa.DateTime(), nullable=True),
        sa.Column("completed_at", sa.DateTime(), nullable=True),
        sa.ForeignKeyConstraint(["project_id"], ["projects.id"], ondelete="CASCADE"),
        sa.PrimaryKeyConstraint("id"),
        sa.UniqueConstraint("task_id", name="uq_autopilot_invocation_audits_task_id"),
    )
    op.create_index(
        "ix_autopilot_invocation_audits_project_created_at",
        "autopilot_invocation_audits",
        ["project_id", "created_at"],
        unique=False,
    )


def downgrade() -> None:
    op.drop_index(
        "ix_autopilot_invocation_audits_project_created_at",
        table_name="autopilot_invocation_audits",
    )
    op.drop_table("autopilot_invocation_audits")