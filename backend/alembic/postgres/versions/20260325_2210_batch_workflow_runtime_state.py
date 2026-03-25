"""add workflow runtime state to batch snapshots

Revision ID: 20260325_batch_workflow_state
Revises: 20260325_batch_runtime_store
Create Date: 2026-03-25 22:10:00.000000
"""

from typing import Sequence, Union

from alembic import op
import sqlalchemy as sa


revision: str = '20260325_batch_workflow_state'
down_revision: Union[str, None] = '20260325_batch_runtime_store'
branch_labels: Union[str, Sequence[str], None] = None
depends_on: Union[str, Sequence[str], None] = None


def upgrade() -> None:
    op.add_column(
        'batch_generation_snapshots',
        sa.Column('workflow_runtime_state', sa.JSON(), nullable=True, comment='Persisted workflow runtime state'),
    )


def downgrade() -> None:
    op.drop_column('batch_generation_snapshots', 'workflow_runtime_state')
