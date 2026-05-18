"""Harden project core defaults.

Revision ID: 20260517_project_core_defaults
Revises: 20260517_settings_core_defaults
Create Date: 2026-05-17 16:00:00.000000
"""

from typing import Sequence, Union

from alembic import op
import sqlalchemy as sa


revision: str = '20260517_project_core_defaults'
down_revision: Union[str, None] = '20260517_settings_core_defaults'
branch_labels: Union[str, Sequence[str], None] = None
depends_on: Union[str, Sequence[str], None] = None


def upgrade() -> None:
    op.execute("UPDATE projects SET target_words = 0 WHERE target_words IS NULL")
    op.execute("UPDATE projects SET current_words = 0 WHERE current_words IS NULL")
    op.execute("UPDATE projects SET status = 'planning' WHERE status IS NULL")
    op.execute("UPDATE projects SET wizard_status = 'incomplete' WHERE wizard_status IS NULL")
    op.execute("UPDATE projects SET wizard_step = 0 WHERE wizard_step IS NULL")
    op.execute("UPDATE projects SET character_count = 5 WHERE character_count IS NULL")

    with op.batch_alter_table('projects', schema=None) as batch_op:
        batch_op.alter_column(
            'target_words',
            existing_type=sa.Integer(),
            nullable=False,
            server_default=sa.text('0'),
        )
        batch_op.alter_column(
            'current_words',
            existing_type=sa.Integer(),
            nullable=False,
            server_default=sa.text('0'),
        )
        batch_op.alter_column(
            'status',
            existing_type=sa.String(length=20),
            nullable=False,
            server_default=sa.text("'planning'"),
        )
        batch_op.alter_column(
            'wizard_status',
            existing_type=sa.String(length=20),
            nullable=False,
            server_default=sa.text("'incomplete'"),
        )
        batch_op.alter_column(
            'wizard_step',
            existing_type=sa.Integer(),
            nullable=False,
            server_default=sa.text('0'),
        )
        batch_op.alter_column(
            'outline_mode',
            existing_type=sa.String(length=20),
            nullable=False,
            server_default=sa.text("'one-to-many'"),
        )
        batch_op.alter_column(
            'character_count',
            existing_type=sa.Integer(),
            nullable=False,
            server_default=sa.text('5'),
        )


def downgrade() -> None:
    with op.batch_alter_table('projects', schema=None) as batch_op:
        batch_op.alter_column(
            'character_count',
            existing_type=sa.Integer(),
            nullable=True,
            server_default=None,
        )
        batch_op.alter_column(
            'outline_mode',
            existing_type=sa.String(length=20),
            nullable=False,
            server_default=None,
        )
        batch_op.alter_column(
            'wizard_step',
            existing_type=sa.Integer(),
            nullable=True,
            server_default=None,
        )
        batch_op.alter_column(
            'wizard_status',
            existing_type=sa.String(length=20),
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
            'current_words',
            existing_type=sa.Integer(),
            nullable=True,
            server_default=None,
        )
        batch_op.alter_column(
            'target_words',
            existing_type=sa.Integer(),
            nullable=True,
            server_default=None,
        )
