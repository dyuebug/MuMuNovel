"""Add durable novel autopilot run and step records.

Revision ID: 20260719_durable_novel_autopilot
Revises: 20260716_autopilot_invocation_audit
Create Date: 2026-07-19 12:00:00.000000
"""

from typing import Sequence, Union

from alembic import op
import sqlalchemy as sa


revision: str = "20260719_durable_novel_autopilot"
down_revision: Union[str, None] = "20260716_autopilot_invocation_audit"
branch_labels: Union[str, Sequence[str], None] = None
depends_on: Union[str, Sequence[str], None] = None


def upgrade() -> None:
    op.create_table(
        "novel_autopilot_runs",
        sa.Column("id", sa.String(length=36), nullable=False),
        sa.Column("project_id", sa.String(length=36), nullable=False),
        sa.Column("user_id", sa.String(length=36), nullable=False),
        sa.Column("schema_version", sa.String(length=64), nullable=False),
        sa.Column("status", sa.String(length=32), nullable=False),
        sa.Column("current_phase", sa.String(length=64), nullable=False),
        sa.Column("current_step", sa.String(length=128), nullable=True),
        sa.Column("active_scope_key", sa.String(length=36), nullable=True),
        sa.Column("current_chapter_id", sa.String(length=36), nullable=True),
        sa.Column("current_chapter_number", sa.Integer(), nullable=True),
        sa.Column("total_chapters", sa.Integer(), nullable=False),
        sa.Column("completed_chapters", sa.Integer(), nullable=False),
        sa.Column("failed_chapters", sa.JSON(), nullable=False),
        sa.Column("pending_rewrites", sa.JSON(), nullable=False),
        sa.Column("total_word_count", sa.BigInteger(), nullable=False),
        sa.Column("execution_scope", sa.String(length=64), nullable=False),
        sa.Column("human_gate_mode", sa.String(length=64), nullable=False),
        sa.Column("gate_interval", sa.Integer(), nullable=True),
        sa.Column("config_snapshot", sa.JSON(), nullable=False),
        sa.Column("max_chapters", sa.Integer(), nullable=True),
        sa.Column("max_tokens", sa.BigInteger(), nullable=True),
        sa.Column("max_estimated_cost", sa.Float(), nullable=True),
        sa.Column("max_runtime_seconds", sa.BigInteger(), nullable=True),
        sa.Column("used_tokens", sa.BigInteger(), nullable=False),
        sa.Column("estimated_cost", sa.Float(), nullable=False),
        sa.Column("epoch", sa.BigInteger(), nullable=False),
        sa.Column("version", sa.BigInteger(), nullable=False),
        sa.Column("consecutive_provider_failures", sa.Integer(), nullable=False),
        sa.Column("consecutive_quality_failures", sa.Integer(), nullable=False),
        sa.Column("last_error_code", sa.String(length=128), nullable=True),
        sa.Column("guidance_digest", sa.String(length=80), nullable=True),
        sa.Column("active_background_task_id", sa.String(length=36), nullable=True),
        sa.Column("final_export_ref", sa.Text(), nullable=True),
        sa.Column("created_at", sa.DateTime(), nullable=False),
        sa.Column("updated_at", sa.DateTime(), nullable=False),
        sa.Column("started_at", sa.DateTime(), nullable=True),
        sa.Column("paused_at", sa.DateTime(), nullable=True),
        sa.Column("completed_at", sa.DateTime(), nullable=True),
        sa.ForeignKeyConstraint(["project_id"], ["projects.id"], ondelete="CASCADE"),
        sa.ForeignKeyConstraint(
            ["current_chapter_id"], ["chapters.id"], ondelete="SET NULL"
        ),
        sa.PrimaryKeyConstraint("id"),
    )
    op.create_index(
        "uq_novel_autopilot_runs_active_scope_key",
        "novel_autopilot_runs",
        ["active_scope_key"],
        unique=True,
    )
    op.create_index(
        "ix_novel_autopilot_runs_project_created_at",
        "novel_autopilot_runs",
        ["project_id", "created_at"],
        unique=False,
    )

    op.create_table(
        "novel_autopilot_step_runs",
        sa.Column("id", sa.String(length=36), nullable=False),
        sa.Column("run_id", sa.String(length=36), nullable=False),
        sa.Column("step_key", sa.String(length=160), nullable=False),
        sa.Column("step_type", sa.String(length=64), nullable=False),
        sa.Column("phase", sa.String(length=64), nullable=False),
        sa.Column("chapter_id", sa.String(length=36), nullable=True),
        sa.Column("chapter_number", sa.Integer(), nullable=True),
        sa.Column("attempt", sa.Integer(), nullable=False),
        sa.Column("run_epoch", sa.BigInteger(), nullable=False),
        sa.Column("status", sa.String(length=32), nullable=False),
        sa.Column("background_task_id", sa.String(length=36), nullable=True),
        sa.Column("input_digest", sa.String(length=80), nullable=False),
        sa.Column("result_digest", sa.String(length=80), nullable=True),
        sa.Column("quality_decision", sa.String(length=32), nullable=True),
        sa.Column("error_code", sa.String(length=128), nullable=True),
        sa.Column("started_at", sa.DateTime(), nullable=True),
        sa.Column("completed_at", sa.DateTime(), nullable=True),
        sa.Column("created_at", sa.DateTime(), nullable=False),
        sa.Column("updated_at", sa.DateTime(), nullable=False),
        sa.ForeignKeyConstraint(
            ["run_id"], ["novel_autopilot_runs.id"], ondelete="CASCADE"
        ),
        sa.ForeignKeyConstraint(["chapter_id"], ["chapters.id"], ondelete="SET NULL"),
        sa.PrimaryKeyConstraint("id"),
        sa.UniqueConstraint(
            "run_id",
            "step_key",
            "attempt",
            name="uq_novel_autopilot_step_runs_run_step_attempt",
        ),
    )
    op.create_index(
        "ix_novel_autopilot_step_runs_run_status_created_at",
        "novel_autopilot_step_runs",
        ["run_id", "status", "created_at"],
        unique=False,
    )


def downgrade() -> None:
    op.drop_index(
        "ix_novel_autopilot_step_runs_run_status_created_at",
        table_name="novel_autopilot_step_runs",
    )
    op.drop_table("novel_autopilot_step_runs")
    op.drop_index(
        "ix_novel_autopilot_runs_project_created_at",
        table_name="novel_autopilot_runs",
    )
    op.drop_index(
        "uq_novel_autopilot_runs_active_scope_key",
        table_name="novel_autopilot_runs",
    )
    op.drop_table("novel_autopilot_runs")
