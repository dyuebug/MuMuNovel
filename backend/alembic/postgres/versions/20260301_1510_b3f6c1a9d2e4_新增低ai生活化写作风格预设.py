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
STYLE_DESCRIPTION = "低AI感的生活化叙事，强调口语自然、节奏起伏与去工整感"
STYLE_PROMPT = """写作风格建议：
1. 叙述像真人在讲亲历故事，口语自然，不要写成说明文
2. 句式长短交替，快节奏处用短句，情绪段落允许稍慢一点
3. 控制修辞密度，每段最多一个明显比喻，别连环堆意象
4. 别把每句话都写成“金句”，保留普通过渡句和生活化连接词
5. 对话允许停顿、打断和半句收口，贴近中国人真实聊天节奏
6. 去掉模板化总结和口号腔，少用“总之/事实上/值得注意的是”
7. 保留少量不完美表达和情绪毛边，让人物声音有区分度
8. 网络热梗仅可偶发点缀，优先给配角使用，必须贴场景且不过量"""


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
