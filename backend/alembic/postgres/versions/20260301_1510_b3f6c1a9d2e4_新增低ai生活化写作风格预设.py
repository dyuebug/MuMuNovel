"""新增低AI生活化写作风格预设

Revision ID: b3f6c1a9d2e4
Revises: 20260222_api_compat
Create Date: 2026-03-01 15:10:00.000000

"""
from typing import Sequence, Union

from alembic import op
import sqlalchemy as sa


# revision identifiers, used by Alembic.
revision: str = "b3f6c1a9d2e4"
down_revision: Union[str, None] = "20260222_api_compat"
branch_labels: Union[str, Sequence[str], None] = None
depends_on: Union[str, Sequence[str], None] = None


STYLE_NAME = "低AI生活化"
STYLE_PRESET_ID = "low_ai_life"
STYLE_DESCRIPTION = "低AI感的生活化叙事，强调中文口语自然度与长短句节奏"
STYLE_PROMPT = """写作风格建议：
1. 叙述像真人在讲亲历故事，口语自然，不要写成说明文
2. 句式长短交替：推进情节用短句提速，情绪段落可适当放长
3. 去掉机械排比和总结腔，少用“总之/事实上/值得注意的是”等套话
4. 对话贴近日常中文，保留人物各自的说话习惯和小毛边
5. 能用动作和细节表达，就别改成抽象解释，让情绪自己落地
6. 网络热梗可少量使用，必须贴场景、贴人物，避免硬塞和连续刷梗"""


def upgrade() -> None:
    """为已有数据库新增低AI生活化预设风格"""
    conn = op.get_bind()

    existing = conn.execute(
        sa.text(
            "SELECT id FROM writing_styles "
            "WHERE user_id IS NULL AND preset_id = :preset_id "
            "LIMIT 1"
        ),
        {"preset_id": STYLE_PRESET_ID},
    ).scalar()

    if existing:
        return

    max_order = conn.execute(
        sa.text(
            "SELECT COALESCE(MAX(order_index), 0) "
            "FROM writing_styles WHERE user_id IS NULL"
        )
    ).scalar() or 0

    conn.execute(
        sa.text(
            "INSERT INTO writing_styles "
            "(user_id, name, style_type, preset_id, description, prompt_content, order_index) "
            "VALUES (NULL, :name, 'preset', :preset_id, :description, :prompt_content, :order_index)"
        ),
        {
            "name": STYLE_NAME,
            "preset_id": STYLE_PRESET_ID,
            "description": STYLE_DESCRIPTION,
            "prompt_content": STYLE_PROMPT,
            "order_index": int(max_order) + 1,
        },
    )


def downgrade() -> None:
    """删除低AI生活化预设风格"""
    conn = op.get_bind()
    conn.execute(
        sa.text(
            "DELETE FROM writing_styles "
            "WHERE user_id IS NULL AND preset_id = :preset_id"
        ),
        {"preset_id": STYLE_PRESET_ID},
    )
