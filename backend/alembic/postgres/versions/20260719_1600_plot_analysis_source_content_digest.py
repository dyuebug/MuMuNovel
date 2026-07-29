"""bind plot analysis to source chapter content digest

Revision ID: 20260719_analysis_content_digest
Revises: 20260719_durable_novel_autopilot
Create Date: 2026-07-19 16:00:00
"""

from typing import Sequence, Union

import sqlalchemy as sa
from alembic import op

revision: str = "20260719_analysis_content_digest"
down_revision: Union[str, None] = "20260719_durable_novel_autopilot"
branch_labels: Union[str, Sequence[str], None] = None
depends_on: Union[str, Sequence[str], None] = None


def upgrade() -> None:
    op.add_column(
        "plot_analysis",
        sa.Column("source_content_digest", sa.String(length=80), nullable=True),
    )


def downgrade() -> None:
    op.drop_column("plot_analysis", "source_content_digest")
