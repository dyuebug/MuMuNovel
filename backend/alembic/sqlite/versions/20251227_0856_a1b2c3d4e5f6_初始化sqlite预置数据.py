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
1. 叙述像身边人讲故事，口语自然，不端着
2. 长短句交替，关键处用短句提速，情绪段落可适度放长
3. 情绪落在动作、停顿和细节里，少用空泛形容词
4. 偶尔可用贴场景的网络表达，点到即止，避免生硬玩梗""",
            "order_index": 1
        },
        {
            "user_id": None,
            "name": "古典优雅",
            "style_type": "preset",
            "preset_id": "classical",
            "description": "古典文雅的写作风格，适合古装、仙侠题材",
            "prompt_content": """写作风格建议：
1. 以典雅白话为底，句式有古风韵味但保持易读
2. 长句铺意境，短句落情绪，读感要有起伏
3. 意象与用典适度，宁少勿滥，避免堆砌辞藻
4. 人物对话符合时代身份，不要突然冒出现代网络口头禅""",
            "order_index": 2
        },
        {
            "user_id": None,
            "name": "现代简约",
            "style_type": "preset",
            "preset_id": "modern",
            "description": "现代简约风格，适合轻小说、网文快节奏叙事",
            "prompt_content": """写作风格建议：
1. 语言干净直接，信息清晰，像当下网文读者熟悉的叙述节奏
2. 多用对话和行动推进剧情，段落利落，少空转
3. 长短句混用，转折处可用短句“收一下”，增强冲击
4. 可少量加入自然口语和轻梗，但必须服务人物与情境""",
            "order_index": 3
        },
        {
            "user_id": None,
            "name": "文艺细腻",
            "style_type": "preset",
            "preset_id": "literary",
            "description": "文艺细腻风格，注重心理描写和氛围营造",
            "prompt_content": """写作风格建议：
1. 文字细腻但不矫情，像在轻声讲一段真事
2. 长句描摹氛围，短句点破心绪，让情感有呼吸感
3. 心理描写要具体可感，避免大段抽象抒情
4. 比喻和修辞克制使用，读起来顺滑，不要“为了文艺而文艺”""",
            "order_index": 4
        },
        {
            "user_id": None,
            "name": "紧张悬疑",
            "style_type": "preset",
            "preset_id": "suspense",
            "description": "紧张悬疑风格，适合推理、惊悚题材",
            "prompt_content": """写作风格建议：
1. 信息要清楚，氛围要压迫，读者能看懂也会紧张
2. 长句铺线索，短句制造顿挫和压迫感
3. 悬念与伏笔要可回收，关键信息别故弄玄虚
4. 对话贴近人物当下状态，可有口语感，但不插无关玩梗""",
            "order_index": 5
        },
        {
            "user_id": None,
            "name": "幽默诙谐",
            "style_type": "preset",
            "preset_id": "humorous",
            "description": "幽默诙谐风格，适合轻松搞笑题材",
            "prompt_content": """写作风格建议：
1. 语气轻松机灵，像朋友互怼互逗，别油腻
2. 包袱尽量来自人物关系和情境反差，不靠硬抖段子
3. 长短句配合节奏，笑点后留一点“回弹空间”
4. 网络热梗可用但要新鲜、克制、贴场景，避免连续刷梗""",
            "order_index": 6
        },
        {
            "user_id": None,
            "name": "低AI生活化",
            "style_type": "preset",
            "preset_id": "low_ai_life",
            "description": "低AI感的生活化叙事，强调口语自然、节奏起伏与去工整感",
            "prompt_content": """写作风格建议：
1. 叙述像真人在讲亲历故事，口语自然，不要写成说明文
2. 句式长短交替，快节奏处用短句，情绪段落允许稍慢一点
3. 控制修辞密度，每段最多一个明显比喻，别连环堆意象
4. 别把每句话都写成“金句”，保留普通过渡句和生活化连接词
5. 对话允许停顿、打断和半句收口，贴近中国人真实聊天节奏
6. 去掉模板化总结和口号腔，少用“总之/事实上/值得注意的是”
7. 保留少量不完美表达和情绪毛边，让人物声音有区分度
            8. 网络热梗仅可偶发点缀，优先给配角使用，必须贴场景且不过量""",
            "order_index": 7
        },
        {
            "user_id": None,
            "name": "低AI连载感",
            "style_type": "preset",
            "preset_id": "low_ai_serial",
            "description": "低AI感的网文连载风格，强调现场感、自然口语和非工整节奏",
            "prompt_content": """写作风格建议：
1. 叙述要有“现场正在发生”的感觉，少解释，多让动作和反应自己说话
2. 句式长短交替，关键推进处用短句，情绪过渡用中句，别整段同长度
3. 段落允许有轻微粗粝感，不追求句句漂亮，优先保证可读和代入
4. 别把每句话都打磨成金句，保留自然的过渡句和口头连接词
5. 对话贴近日常中文，允许停顿、打断、欲言又止，角色语气要分开
6. 修辞要克制，每段最多一个明显比喻，避免连续堆意象
7. 热梗仅可偶发点缀，优先放在配角台词，必须贴场景且不过量
8. 章末可留轻钩子，但不要生硬反转，保持“下一章想看”的顺滑感""",
            "order_index": 8
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
