"""更新低AI生活化风格文案V2

Revision ID: c4e9d1b7a2f0
Revises: b3f6c1a9d2e4
Create Date: 2026-03-01 17:00:00.000000

"""
from typing import Sequence, Union

from alembic import op
import sqlalchemy as sa


# revision identifiers, used by Alembic.
revision: str = "c4e9d1b7a2f0"
down_revision: Union[str, None] = "b3f6c1a9d2e4"
branch_labels: Union[str, Sequence[str], None] = None
depends_on: Union[str, Sequence[str], None] = None


STYLE_NAME = "低AI生活化"
STYLE_PRESET_ID = "low_ai_life"

OLD_STYLE_DESCRIPTION = "低AI感的生活化叙事，强调中文口语自然度与长短句节奏"
OLD_STYLE_PROMPT = """写作风格建议：
1. 叙述像真人在讲亲历故事，口语自然，不要写成说明文
2. 句式长短交替：推进情节用短句提速，情绪段落可适当放长
3. 去掉机械排比和总结腔，少用“总之/事实上/值得注意的是”等套话
4. 对话贴近日常中文，保留人物各自的说话习惯和小毛边
5. 能用动作和细节表达，就别改成抽象解释，让情绪自己落地
6. 网络热梗可少量使用，必须贴场景、贴人物，避免硬塞和连续刷梗"""

NEW_STYLE_DESCRIPTION = "低AI感的生活化叙事，强调口语自然、节奏起伏与去工整感"
NEW_STYLE_PROMPT = """写作风格建议：
1. 叙述像真人在讲亲历故事，口语自然，不要写成说明文
2. 句式长短交替，快节奏处用短句，情绪段落允许稍慢一点
3. 控制修辞密度，每段最多一个明显比喻，别连环堆意象
4. 别把每句话都写成“金句”，保留普通过渡句和生活化连接词
5. 对话允许停顿、打断和半句收口，贴近中国人真实聊天节奏
6. 去掉模板化总结和口号腔，少用“总之/事实上/值得注意的是”
7. 保留少量不完美表达和情绪毛边，让人物声音有区分度
8. 网络热梗仅可偶发点缀，优先给配角使用，必须贴场景且不过量"""


def _upsert_style(conn, description: str, prompt: str) -> None:
    existing = conn.execute(
        sa.text(
            "SELECT id FROM writing_styles "
            "WHERE user_id IS NULL AND preset_id = :preset_id "
            "LIMIT 1"
        ),
        {"preset_id": STYLE_PRESET_ID},
    ).scalar()

    if existing:
        conn.execute(
            sa.text(
                "UPDATE writing_styles "
                "SET name = :name, style_type = 'preset', description = :description, prompt_content = :prompt_content "
                "WHERE id = :id"
            ),
            {
                "id": existing,
                "name": STYLE_NAME,
                "description": description,
                "prompt_content": prompt,
            },
        )
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
            "description": description,
            "prompt_content": prompt,
            "order_index": int(max_order) + 1,
        },
    )


def upgrade() -> None:
    """升级低AI生活化文案到V2"""
    conn = op.get_bind()
    _upsert_style(conn, NEW_STYLE_DESCRIPTION, NEW_STYLE_PROMPT)


def downgrade() -> None:
    """回退低AI生活化文案到V1"""
    conn = op.get_bind()
    _upsert_style(conn, OLD_STYLE_DESCRIPTION, OLD_STYLE_PROMPT)
