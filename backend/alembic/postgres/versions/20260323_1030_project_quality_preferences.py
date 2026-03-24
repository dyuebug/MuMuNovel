"""新增项目默认质量预设字段

Revision ID: 20260323_proj_quality_prefs
Revises: 20260322_proj_gen_defaults
Create Date: 2026-03-23 10:30:00.000000

"""

from typing import Sequence, Union

from alembic import op
import sqlalchemy as sa


# revision identifiers, used by Alembic.
revision: str = '20260323_proj_quality_prefs'
down_revision: Union[str, None] = '20260322_proj_gen_defaults'
branch_labels: Union[str, Sequence[str], None] = None
depends_on: Union[str, Sequence[str], None] = None


def upgrade() -> None:
    op.add_column('projects', sa.Column('default_quality_preset', sa.String(length=50), nullable=True, comment='默认质量预设'))
    op.add_column('projects', sa.Column('default_quality_notes', sa.Text(), nullable=True, comment='默认质量补充偏好'))


def downgrade() -> None:
    op.drop_column('projects', 'default_quality_notes')
    op.drop_column('projects', 'default_quality_preset')
