"""新增低AI连载感写作风格预设

Revision ID: e8b4d6c1f2a7
Revises: c4e9d1b7a2f0
Create Date: 2026-03-01 17:32:00.000000

"""
from typing import Sequence, Union

from alembic import op
import sqlalchemy as sa


# revision identifiers, used by Alembic.
revision: str = "e8b4d6c1f2a7"
down_revision: Union[str, None] = "c4e9d1b7a2f0"
branch_labels: Union[str, Sequence[str], None] = None
depends_on: Union[str, Sequence[str], None] = None


STYLE_NAME = "低AI连载感"
STYLE_PRESET_ID = "low_ai_serial"
STYLE_DESCRIPTION = "低AI感的网文连载风格，强调现场感、自然口语和非工整节奏"
STYLE_PROMPT = """写作风格建议：
1. 叙述要有“现场正在发生”的感觉，少解释，多让动作和反应自己说话
2. 句式长短交替，关键推进处用短句，情绪过渡用中句，别整段同长度
3. 段落允许有轻微粗粝感，不追求句句漂亮，优先保证可读和代入
4. 别把每句话都打磨成金句，保留自然的过渡句和口头连接词
5. 对话贴近日常中文，允许停顿、打断、欲言又止，角色语气要分开
6. 修辞要克制，每段最多一个明显比喻，避免连续堆意象
7. 热梗仅可偶发点缀，优先放在配角台词，必须贴场景且不过量
8. 章末可留轻钩子，但不要生硬反转，保持“下一章想看”的顺滑感"""


def upgrade() -> None:
    """为已有数据库新增低AI连载感预设"""
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
        conn.execute(
            sa.text(
                "UPDATE writing_styles "
                "SET name = :name, style_type = 'preset', description = :description, prompt_content = :prompt_content "
                "WHERE id = :id"
            ),
            {
                "id": existing,
                "name": STYLE_NAME,
                "description": STYLE_DESCRIPTION,
                "prompt_content": STYLE_PROMPT,
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
            "description": STYLE_DESCRIPTION,
            "prompt_content": STYLE_PROMPT,
            "order_index": int(max_order) + 1,
        },
    )


def downgrade() -> None:
    """删除低AI连载感预设"""
    conn = op.get_bind()
    conn.execute(
        sa.text(
            "DELETE FROM writing_styles "
            "WHERE user_id IS NULL AND preset_id = :preset_id"
        ),
        {"preset_id": STYLE_PRESET_ID},
    )
