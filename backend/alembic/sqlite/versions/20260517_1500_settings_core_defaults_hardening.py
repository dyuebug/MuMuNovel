"""Harden settings core defaults.

Revision ID: 20260517_settings_core_defaults
Revises: 20260517_regeneration_task_defaults
Create Date: 2026-05-17 15:00:00.000000
"""

from typing import Sequence, Union

from alembic import op
import sqlalchemy as sa


revision: str = '20260517_settings_core_defaults'
down_revision: Union[str, None] = '20260517_regeneration_task_defaults'
branch_labels: Union[str, Sequence[str], None] = None
depends_on: Union[str, Sequence[str], None] = None


def upgrade() -> None:
    op.execute("UPDATE settings SET api_provider = 'openai' WHERE api_provider IS NULL")
    op.execute("UPDATE settings SET llm_model = 'gpt-4' WHERE llm_model IS NULL")
    op.execute("UPDATE settings SET temperature = 0.7 WHERE temperature IS NULL")
    op.execute("UPDATE settings SET max_tokens = 2000 WHERE max_tokens IS NULL")

    with op.batch_alter_table('settings', schema=None) as batch_op:
        batch_op.alter_column(
            'api_provider',
            existing_type=sa.String(length=50),
            nullable=False,
            server_default=sa.text("'openai'"),
        )
        batch_op.alter_column(
            'llm_model',
            existing_type=sa.String(length=100),
            nullable=False,
            server_default=sa.text("'gpt-4'"),
        )
        batch_op.alter_column(
            'temperature',
            existing_type=sa.Float(),
            nullable=False,
            server_default=sa.text('0.7'),
        )
        batch_op.alter_column(
            'max_tokens',
            existing_type=sa.Integer(),
            nullable=False,
            server_default=sa.text('2000'),
        )


def downgrade() -> None:
    with op.batch_alter_table('settings', schema=None) as batch_op:
        batch_op.alter_column(
            'max_tokens',
            existing_type=sa.Integer(),
            nullable=True,
            server_default=None,
        )
        batch_op.alter_column(
            'temperature',
            existing_type=sa.Float(),
            nullable=True,
            server_default=None,
        )
        batch_op.alter_column(
            'llm_model',
            existing_type=sa.String(length=100),
            nullable=True,
            server_default=None,
        )
        batch_op.alter_column(
            'api_provider',
            existing_type=sa.String(length=50),
            nullable=True,
            server_default=None,
        )
