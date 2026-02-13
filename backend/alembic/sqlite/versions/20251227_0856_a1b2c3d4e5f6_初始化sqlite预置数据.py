"""初始化SQLite预置数据

Revision ID: a1b2c3d4e5f6
Revises: fbeb1038c728
Create Date: 2025-12-27 08:56:00.000000

"""
from typing import Sequence, Union

from alembic import op
import sqlalchemy as sa
from sqlalchemy import table, column, String, Integer, Text


# revision identifiers, used by Alembic.
revision: str = 'a1b2c3d4e5f6'
down_revision: Union[str, None] = 'fbeb1038c728'
branch_labels: Union[str, Sequence[str], None] = None
depends_on: Union[str, Sequence[str], None] = None


def upgrade() -> None:
    """插入预置数据"""
    
    # ==================== 1. 插入关系类型数据 ====================
    relationship_types_table = table(
        'relationship_types',
        column('name', String),
        column('category', String),
        column('reverse_name', String),
        column('intimacy_range', String),
        column('icon', String),
        column('description', Text),
    )
    
    relationship_types_data = [
        # 家庭关系
        {"name": "父亲", "category": "family", "reverse_name": "子女", "intimacy_range": "high", "icon": "👨", "description": "父子/父女关系"},
        {"name": "母亲", "category": "family", "reverse_name": "子女", "intimacy_range": "high", "icon": "👩", "description": "母子/母女关系"},
        {"name": "兄弟", "category": "family", "reverse_name": "兄弟", "intimacy_range": "high", "icon": "👬", "description": "兄弟关系"},
        {"name": "姐妹", "category": "family", "reverse_name": "姐妹", "intimacy_range": "high", "icon": "👭", "description": "姐妹关系"},
        {"name": "子女", "category": "family", "reverse_name": "父母", "intimacy_range": "high", "icon": "👶", "description": "子女关系"},
        {"name": "配偶", "category": "family", "reverse_name": "配偶", "intimacy_range": "high", "icon": "💑", "description": "夫妻关系"},
        {"name": "恋人", "category": "family", "reverse_name": "恋人", "intimacy_range": "high", "icon": "💕", "description": "恋爱关系"},
        
        # 社交关系
        {"name": "师父", "category": "social", "reverse_name": "徒弟", "intimacy_range": "high", "icon": "🎓", "description": "师徒关系（师父视角）"},
        {"name": "徒弟", "category": "social", "reverse_name": "师父", "intimacy_range": "high", "icon": "📚", "description": "师徒关系（徒弟视角）"},
        {"name": "朋友", "category": "social", "reverse_name": "朋友", "intimacy_range": "medium", "icon": "🤝", "description": "朋友关系"},
        {"name": "同学", "category": "social", "reverse_name": "同学", "intimacy_range": "medium", "icon": "🎒", "description": "同学关系"},
        {"name": "邻居", "category": "social", "reverse_name": "邻居", "intimacy_range": "low", "icon": "🏘️", "description": "邻居关系"},
        {"name": "知己", "category": "social", "reverse_name": "知己", "intimacy_range": "high", "icon": "💙", "description": "知心好友"},
        
        # 职业关系
        {"name": "上司", "category": "professional", "reverse_name": "下属", "intimacy_range": "low", "icon": "👔", "description": "上下级关系（上司视角）"},
        {"name": "下属", "category": "professional", "reverse_name": "上司", "intimacy_range": "low", "icon": "💼", "description": "上下级关系（下属视角）"},
        {"name": "同事", "category": "professional", "reverse_name": "同事", "intimacy_range": "medium", "icon": "🤵", "description": "同事关系"},
        {"name": "合作伙伴", "category": "professional", "reverse_name": "合作伙伴", "intimacy_range": "medium", "icon": "🤜🤛", "description": "合作关系"},
        
        # 敌对关系
        {"name": "敌人", "category": "hostile", "reverse_name": "敌人", "intimacy_range": "low", "icon": "⚔️", "description": "敌对关系"},
        {"name": "仇人", "category": "hostile", "reverse_name": "仇人", "intimacy_range": "low", "icon": "💢", "description": "仇恨关系"},
        {"name": "竞争对手", "category": "hostile", "reverse_name": "竞争对手", "intimacy_range": "low", "icon": "🎯", "description": "竞争关系"},
        {"name": "宿敌", "category": "hostile", "reverse_name": "宿敌", "intimacy_range": "low", "icon": "⚡", "description": "宿命之敌"},
    ]
    
    op.bulk_insert(relationship_types_table, relationship_types_data)
    print(f"✅ SQLite: 已插入 {len(relationship_types_data)} 条关系类型数据")
    
    
    # ==================== 2. 插入全局写作风格预设 ====================
    writing_styles_table = table(
        'writing_styles',
        column('user_id', String),
        column('name', String),
        column('style_type', String),
        column('preset_id', String),
        column('description', Text),
        column('prompt_content', Text),
        column('order_index', Integer),
    )
    
    writing_styles_data = [
        {
            "user_id": None,  # NULL 表示全局预设
            "name": "自然流畅",
            "style_type": "preset",
            "preset_id": "natural",
            "description": "自然流畅的叙事风格，适合现代都市、现实题材",
            "prompt_content": """写作风格建议：
1. 句子干净利落，读起来顺口
2. 多用短句推进，节奏别拖
3. 情绪通过动作和细节自然带出
4. 少堆形容词，避免华而不实""",
            "order_index": 1
        },
        {
            "user_id": None,
            "name": "古典优雅",
            "style_type": "preset",
            "preset_id": "classical",
            "description": "古典文雅的写作风格，适合古装、仙侠题材",
            "prompt_content": """写作风格建议：
1. 用典雅白话为主，适度点到文言
2. 可借古典意象，但不过度卖弄
3. 重在意境和韵味，文字要通顺
4. 对话符合时代气息，别像现代口语""",
            "order_index": 2
        },
        {
            "user_id": None,
            "name": "现代简约",
            "style_type": "preset",
            "preset_id": "modern",
            "description": "现代简约风格，适合轻小说、网文快节奏叙事",
            "prompt_content": """写作风格建议：
1. 语言直接明快，信息给到点上
2. 多用对话和行动推剧情
3. 少写无效描写，突出关键动作
4. 章节节奏偏快，适合连读""",
            "order_index": 3
        },
        {
            "user_id": None,
            "name": "文艺细腻",
            "style_type": "preset",
            "preset_id": "literary",
            "description": "文艺细腻风格，注重心理描写和氛围营造",
            "prompt_content": """写作风格建议：
1. 重点写人物心绪变化和细小反应
2. 环境描写服务情绪，不喧宾夺主
3. 语言细腻但不端着，避免空泛抒情
4. 修辞点到为止，以可读性优先""",
            "order_index": 4
        },
        {
            "user_id": None,
            "name": "紧张悬疑",
            "style_type": "preset",
            "preset_id": "suspense",
            "description": "紧张悬疑风格，适合推理、惊悚题材",
            "prompt_content": """写作风格建议：
1. 氛围要有压迫感，但信息要清楚
2. 用短句和断点拉高紧张感
3. 悬念要可回收，伏笔要有落点
4. 细节要服务推理，不做无效铺陈""",
            "order_index": 5
        },
        {
            "user_id": None,
            "name": "幽默诙谐",
            "style_type": "preset",
            "preset_id": "humorous",
            "description": "幽默诙谐风格，适合轻松搞笑题材",
            "prompt_content": """写作风格建议：
1. 语气轻松有梗，像真人聊天
2. 笑点尽量落在人物互动上
3. 反转和夸张适度，别油腻
4. 保持轻快基调，但不影响剧情推进""",
            "order_index": 6
        },
    ]
    
    op.bulk_insert(writing_styles_table, writing_styles_data)
    print(f"✅ SQLite: 已插入 {len(writing_styles_data)} 条全局写作风格预设")


def downgrade() -> None:
    """删除预置数据"""
    
    # 删除写作风格预设（只删除全局预设）
    op.execute("DELETE FROM writing_styles WHERE user_id IS NULL")
    print("✅ SQLite: 已删除全局写作风格预设")
    
    # 删除关系类型
    op.execute("DELETE FROM relationship_types")
    print("✅ SQLite: 已删除关系类型数据")
