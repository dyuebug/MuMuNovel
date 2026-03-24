"""新增项目默认生成偏好字段

Revision ID: 20260322_proj_gen_defaults
Revises: e8b4d6c1f2a7
Create Date: 2026-03-22 12:00:00.000000

"""

from typing import Sequence, Union

from alembic import op
import sqlalchemy as sa


# revision identifiers, used by Alembic.
revision: str = '20260322_proj_gen_defaults'
down_revision: Union[str, None] = 'e8b4d6c1f2a7'
branch_labels: Union[str, Sequence[str], None] = None
depends_on: Union[str, Sequence[str], None] = None


def upgrade() -> None:
    op.add_column('projects', sa.Column('default_creative_mode', sa.String(length=50), nullable=True, comment='默认创作模式'))
    op.add_column('projects', sa.Column('default_story_focus', sa.String(length=50), nullable=True, comment='默认结构侧重点'))
    op.add_column('projects', sa.Column('default_plot_stage', sa.String(length=20), nullable=True, comment='默认剧情阶段'))
    op.add_column('projects', sa.Column('default_story_creation_brief', sa.Text(), nullable=True, comment='默认创作总控摘要'))


def downgrade() -> None:
    op.drop_column('projects', 'default_story_creation_brief')
    op.drop_column('projects', 'default_plot_stage')
    op.drop_column('projects', 'default_story_focus')
    op.drop_column('projects', 'default_creative_mode')
