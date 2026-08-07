use fs2::FileExt;
use sea_orm::{
    ConnectionTrait, DatabaseBackend, DatabaseConnection, DbErr, Statement, TransactionTrait,
};
use serde_json::{json, Value};

use super::password_hash_service::CANONICAL_ARGON2_VERIFIER_STORAGE_LENGTH;
use sha1::{Digest, Sha1};
use std::fs::OpenOptions;
use std::path::PathBuf;
use std::time::{Duration, Instant};
use tokio::time::sleep;

pub(crate) const POSTGRES_ALEMBIC_HEAD: &str = "20260807_autopilot_retry_backoff";
const MIGRATION_RUNTIME_OWNER: &str = "rust_db_migrator_migration_executor";
const RUST_METADATA_OWNER: &str = "schema_migration_metadata_service";
const MIGRATION_LOCK_NAME: &str = "mumuainovel:alembic";
const MIGRATION_LOCK_FILE_NAME: &str = ".migration-singleflight.lock";
const DEFAULT_MIGRATION_LOCK_TIMEOUT_SECONDS: u64 = 300;
const DEFAULT_MIGRATION_LOCK_POLL_INTERVAL_SECONDS: f64 = 1.0;
const RUST_MIGRATION_TAIL_HARDENING_REPLAY_ENABLED_ENV: &str =
    "RUST_MIGRATION_TAIL_HARDENING_REPLAY_ENABLED";
const RUST_EXECUTABLE_MIGRATION_COVERAGE: &str =
    "initial_schema_seed_data_settings_system_prompt_foreshadows_prompt_workshop_character_state_settings_api_compat_writing_style_project_defaults_batch_runtime_tail_hardening_password_hash_phc_text_autopilot_invocation_audit_durable_novel_autopilot_plot_analysis_content_digest_autopilot_user_id_capacity_and_provider_retry_backoff";
const INITIAL_SCHEMA_SQL: &str = include_str!("schema_migration_initial_schema.sql");
const ALEMBIC_VERSION_NUM_LENGTH: i32 = 64;
const PASSWORD_HASH_STORAGE_METADATA_QUERY: &str =
    "SELECT data_type, udt_name, character_maximum_length \
FROM information_schema.columns \
WHERE table_schema = current_schema() \
AND table_name = 'user_passwords' \
AND column_name = 'password_hash' \
LIMIT 1";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct MigrationRevisionCatalogEntry {
    pub(crate) revision: &'static str,
    pub(crate) down_revision: Option<&'static str>,
    pub(crate) filename: &'static str,
}

const POSTGRES_REVISION_CATALOG: &[MigrationRevisionCatalogEntry] = &[
    MigrationRevisionCatalogEntry {
        revision: "ee0a189f1532",
        down_revision: None,
        filename: "20251226_1008_ee0a189f1532_初始数据库结构.py",
    },
    MigrationRevisionCatalogEntry {
        revision: "e411428f00c0",
        down_revision: Some("ee0a189f1532"),
        filename: "20251226_1102_e411428f00c0_初始化预置数据.py",
    },
    MigrationRevisionCatalogEntry {
        revision: "a7e4408e1d5b",
        down_revision: Some("e411428f00c0"),
        filename: "20251227_1541_a7e4408e1d5b_添加system_prompt字段到settings表.py",
    },
    MigrationRevisionCatalogEntry {
        revision: "6a73f37e9adb",
        down_revision: Some("a7e4408e1d5b"),
        filename: "20260119_1729_6a73f37e9adb_添加伏笔管理表.py",
    },
    MigrationRevisionCatalogEntry {
        revision: "421237957b27",
        down_revision: Some("6a73f37e9adb"),
        filename: "20260127_1404_421237957b27_添加提示词工坊相关表结构.py",
    },
    MigrationRevisionCatalogEntry {
        revision: "d4d253e3f4c6",
        down_revision: Some("421237957b27"),
        filename: "20260212_1244_d4d253e3f4c6_添加角色心理状态追踪字段.py",
    },
    MigrationRevisionCatalogEntry {
        revision: "20260222_api_compat",
        down_revision: Some("d4d253e3f4c6"),
        filename: "20260222_add_api_compatibility_fields.py",
    },
    MigrationRevisionCatalogEntry {
        revision: "b3f6c1a9d2e4",
        down_revision: Some("20260222_api_compat"),
        filename: "20260301_1510_b3f6c1a9d2e4_新增低ai生活化写作风格预设.py",
    },
    MigrationRevisionCatalogEntry {
        revision: "c4e9d1b7a2f0",
        down_revision: Some("b3f6c1a9d2e4"),
        filename: "20260301_1700_c4e9d1b7a2f0_更新低ai生活化风格文案v2.py",
    },
    MigrationRevisionCatalogEntry {
        revision: "e8b4d6c1f2a7",
        down_revision: Some("c4e9d1b7a2f0"),
        filename: "20260301_1730_e8b4d6c1f2a7_新增低ai连载感写作风格预设.py",
    },
    MigrationRevisionCatalogEntry {
        revision: "20260322_proj_gen_defaults",
        down_revision: Some("e8b4d6c1f2a7"),
        filename: "20260322_1200_project_generation_defaults.py",
    },
    MigrationRevisionCatalogEntry {
        revision: "20260323_proj_quality_prefs",
        down_revision: Some("20260322_proj_gen_defaults"),
        filename: "20260323_1030_project_quality_preferences.py",
    },
    MigrationRevisionCatalogEntry {
        revision: "20260325_batch_runtime_store",
        down_revision: Some("20260323_proj_quality_prefs"),
        filename: "20260325_0900_batch_runtime_store.py",
    },
    MigrationRevisionCatalogEntry {
        revision: "20260325_batch_workflow_state",
        down_revision: Some("20260325_batch_runtime_store"),
        filename: "20260325_2210_batch_workflow_runtime_state.py",
    },
    MigrationRevisionCatalogEntry {
        revision: "20260517_analysis_task_hardening",
        down_revision: Some("20260325_batch_workflow_state"),
        filename: "20260517_1200_analysis_task_progress_hardening.py",
    },
    MigrationRevisionCatalogEntry {
        revision: "20260517_batch_task_defaults",
        down_revision: Some("20260517_analysis_task_hardening"),
        filename: "20260517_1300_batch_generation_task_defaults_hardening.py",
    },
    MigrationRevisionCatalogEntry {
        revision: "20260517_regeneration_task_defaults",
        down_revision: Some("20260517_batch_task_defaults"),
        filename: "20260517_1400_regeneration_task_defaults_hardening.py",
    },
    MigrationRevisionCatalogEntry {
        revision: "20260517_settings_core_defaults",
        down_revision: Some("20260517_regeneration_task_defaults"),
        filename: "20260517_1500_settings_core_defaults_hardening.py",
    },
    MigrationRevisionCatalogEntry {
        revision: "20260517_project_core_defaults",
        down_revision: Some("20260517_settings_core_defaults"),
        filename: "20260517_1600_project_core_defaults_hardening.py",
    },
    MigrationRevisionCatalogEntry {
        revision: "20260712_password_hash_phc_text",
        down_revision: Some("20260517_project_core_defaults"),
        filename: "20260712_1200_password_hash_phc_text.py",
    },
    MigrationRevisionCatalogEntry {
        revision: "20260716_autopilot_invocation_audit",
        down_revision: Some("20260712_password_hash_phc_text"),
        filename: "20260716_2200_autopilot_invocation_audit.py",
    },
    MigrationRevisionCatalogEntry {
        revision: "20260719_durable_novel_autopilot",
        down_revision: Some("20260716_autopilot_invocation_audit"),
        filename: "20260719_1200_durable_novel_autopilot.py",
    },
    MigrationRevisionCatalogEntry {
        revision: "20260719_analysis_content_digest",
        down_revision: Some("20260719_durable_novel_autopilot"),
        filename: "20260719_1600_plot_analysis_source_content_digest.py",
    },
    MigrationRevisionCatalogEntry {
        revision: "20260719_autopilot_user_id_capacity",
        down_revision: Some("20260719_analysis_content_digest"),
        filename: "20260719_1700_novel_autopilot_user_id_capacity.py",
    },
    MigrationRevisionCatalogEntry {
        revision: "20260720_audit_actor_id_capacity",
        down_revision: Some("20260719_autopilot_user_id_capacity"),
        filename: "20260720_0900_autopilot_audit_actor_user_id_capacity.py",
    },
    MigrationRevisionCatalogEntry {
        revision: POSTGRES_ALEMBIC_HEAD,
        down_revision: Some("20260720_audit_actor_id_capacity"),
        filename: "20260807_1200_novel_autopilot_retry_backoff.py",
    },
];

pub(crate) fn postgres_revision_catalog() -> &'static [MigrationRevisionCatalogEntry] {
    POSTGRES_REVISION_CATALOG
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RustMigrationSqlStep {
    pub(crate) sql: &'static str,
    pub(crate) statement_kind: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RustMigrationExecutableRevision {
    pub(crate) revision: &'static str,
    pub(crate) filename: &'static str,
    pub(crate) execution_scope: &'static str,
    pub(crate) upgrade_steps: &'static [RustMigrationSqlStep],
    pub(crate) downgrade_steps: &'static [RustMigrationSqlStep],
}

const INITIAL_SCHEMA_UPGRADE_STEPS: &[RustMigrationSqlStep] = &[RustMigrationSqlStep {
    sql: INITIAL_SCHEMA_SQL,
    statement_kind: "ddl_initial_schema_script",
}];

const INITIAL_SCHEMA_DOWNGRADE_STEPS: &[RustMigrationSqlStep] = &[RustMigrationSqlStep {
    sql: "DROP SCHEMA public CASCADE; CREATE SCHEMA public",
    statement_kind: "ddl_initial_schema_drop_all",
}];

const RELATIONSHIP_TYPES_SEED_INSERT_SQL: &str = r#"INSERT INTO relationship_types (name, category, reverse_name, intimacy_range, icon, description) VALUES
('父亲', 'family', '子女', 'high', '👨', '父子/父女关系'),
('母亲', 'family', '子女', 'high', '👩', '母子/母女关系'),
('兄弟', 'family', '兄弟', 'high', '👬', '兄弟关系'),
('姐妹', 'family', '姐妹', 'high', '👭', '姐妹关系'),
('子女', 'family', '父母', 'high', '👶', '子女关系'),
('配偶', 'family', '配偶', 'high', '💑', '夫妻关系'),
('恋人', 'family', '恋人', 'high', '💕', '恋爱关系'),
('师父', 'social', '徒弟', 'high', '🎓', '师徒关系（师父视角）'),
('徒弟', 'social', '师父', 'high', '📚', '师徒关系（徒弟视角）'),
('朋友', 'social', '朋友', 'medium', '🤝', '朋友关系'),
('同学', 'social', '同学', 'medium', '🎒', '同学关系'),
('邻居', 'social', '邻居', 'low', '🏘️', '邻居关系'),
('知己', 'social', '知己', 'high', '💙', '知心好友'),
('上司', 'professional', '下属', 'low', '👔', '上下级关系（上司视角）'),
('下属', 'professional', '上司', 'low', '💼', '上下级关系（下属视角）'),
('同事', 'professional', '同事', 'medium', '🤵', '同事关系'),
('合作伙伴', 'professional', '合作伙伴', 'medium', '🤜🤛', '合作关系'),
('敌人', 'hostile', '敌人', 'low', '⚔️', '敌对关系'),
('仇人', 'hostile', '仇人', 'low', '💢', '仇恨关系'),
('竞争对手', 'hostile', '竞争对手', 'low', '🎯', '竞争关系'),
('宿敌', 'hostile', '宿敌', 'low', '⚡', '宿命之敌')"#;

const INITIAL_WRITING_STYLES_SEED_INSERT_SQL: &str = r#"INSERT INTO writing_styles (user_id, name, style_type, preset_id, description, prompt_content, order_index) VALUES
(NULL, '自然流畅', 'preset', 'natural', '自然流畅的叙事风格，适合现代都市、现实题材', $$写作风格建议：
1. 叙述像身边人讲故事，口语自然，不端着
2. 长短句交替，关键处用短句提速，情绪段落可适度放长
3. 情绪落在动作、停顿和细节里，少用空泛形容词
4. 偶尔可用贴场景的网络表达，点到即止，避免生硬玩梗$$, 1),
(NULL, '古典优雅', 'preset', 'classical', '古典文雅的写作风格，适合古装、仙侠题材', $$写作风格建议：
1. 以典雅白话为底，句式有古风韵味但保持易读
2. 长句铺意境，短句落情绪，读感要有起伏
3. 意象与用典适度，宁少勿滥，避免堆砌辞藻
4. 人物对话符合时代身份，不要突然冒出现代网络口头禅$$, 2),
(NULL, '现代简约', 'preset', 'modern', '现代简约风格，适合轻小说、网文快节奏叙事', $$写作风格建议：
1. 语言干净直接，信息清晰，像当下网文读者熟悉的叙述节奏
2. 多用对话和行动推进剧情，段落利落，少空转
3. 长短句混用，转折处可用短句“收一下”，增强冲击
4. 可少量加入自然口语和轻梗，但必须服务人物与情境$$, 3),
(NULL, '文艺细腻', 'preset', 'literary', '文艺细腻风格，注重心理描写和氛围营造', $$写作风格建议：
1. 文字细腻但不矫情，像在轻声讲一段真事
2. 长句描摹氛围，短句点破心绪，让情感有呼吸感
3. 心理描写要具体可感，避免大段抽象抒情
4. 比喻和修辞克制使用，读起来顺滑，不要“为了文艺而文艺”$$, 4),
(NULL, '紧张悬疑', 'preset', 'suspense', '紧张悬疑风格，适合推理、惊悚题材', $$写作风格建议：
1. 信息要清楚，氛围要压迫，读者能看懂也会紧张
2. 长句铺线索，短句制造顿挫和压迫感
3. 悬念与伏笔要可回收，关键信息别故弄玄虚
4. 对话贴近人物当下状态，可有口语感，但不插无关玩梗$$, 5),
(NULL, '幽默诙谐', 'preset', 'humorous', '幽默诙谐风格，适合轻松搞笑题材', $$写作风格建议：
1. 语气轻松机灵，像朋友互怼互逗，别油腻
2. 包袱尽量来自人物关系和情境反差，不靠硬抖段子
3. 长短句配合节奏，笑点后留一点“回弹空间”
4. 网络热梗可用但要新鲜、克制、贴场景，避免连续刷梗$$, 6),
(NULL, '低AI生活化', 'preset', 'low_ai_life', '低AI感的生活化叙事，强调口语自然、节奏起伏与去工整感', $$写作风格建议：
1. 叙述像真人在讲亲历故事，口语自然，不要写成说明文
2. 句式长短交替，快节奏处用短句，情绪段落允许稍慢一点
3. 控制修辞密度，每段最多一个明显比喻，别连环堆意象
4. 别把每句话都写成“金句”，保留普通过渡句和生活化连接词
5. 对话允许停顿、打断和半句收口，贴近中国人真实聊天节奏
6. 去掉模板化总结和口号腔，少用“总之/事实上/值得注意的是”
7. 保留少量不完美表达和情绪毛边，让人物声音有区分度
            8. 网络热梗仅可偶发点缀，优先给配角使用，必须贴场景且不过量$$, 7),
(NULL, '低AI连载感', 'preset', 'low_ai_serial', '低AI感的网文连载风格，强调现场感、自然口语和非工整节奏', $$写作风格建议：
1. 叙述要有“现场正在发生”的感觉，少解释，多让动作和反应自己说话
2. 句式长短交替，关键推进处用短句，情绪过渡用中句，别整段同长度
3. 段落允许有轻微粗粝感，不追求句句漂亮，优先保证可读和代入
4. 别把每句话都打磨成金句，保留自然的过渡句和口头连接词
5. 对话贴近日常中文，允许停顿、打断、欲言又止，角色语气要分开
6. 修辞要克制，每段最多一个明显比喻，避免连续堆意象
7. 热梗仅可偶发点缀，优先放在配角台词，必须贴场景且不过量
8. 章末可留轻钩子，但不要生硬反转，保持“下一章想看”的顺滑感$$, 8)"#;

const INITIAL_SEED_DATA_UPGRADE_STEPS: &[RustMigrationSqlStep] = &[
    RustMigrationSqlStep {
        sql: RELATIONSHIP_TYPES_SEED_INSERT_SQL,
        statement_kind: "data_seed_insert",
    },
    RustMigrationSqlStep {
        sql: INITIAL_WRITING_STYLES_SEED_INSERT_SQL,
        statement_kind: "data_seed_insert",
    },
];

const INITIAL_SEED_DATA_DOWNGRADE_STEPS: &[RustMigrationSqlStep] = &[
    RustMigrationSqlStep {
        sql: "DELETE FROM writing_styles WHERE user_id IS NULL",
        statement_kind: "data_delete",
    },
    RustMigrationSqlStep {
        sql: "DELETE FROM relationship_types",
        statement_kind: "data_delete",
    },
];

const LOW_AI_LIFE_INSERT_SQL: &str = r#"INSERT INTO writing_styles (user_id, name, style_type, preset_id, description, prompt_content, order_index)
SELECT NULL, '低AI生活化', 'preset', 'low_ai_life', '低AI感的生活化叙事，强调口语自然、节奏起伏与去工整感', $$写作风格建议：
1. 叙述像真人在讲亲历故事，口语自然，不要写成说明文
2. 句式长短交替，快节奏处用短句，情绪段落允许稍慢一点
3. 控制修辞密度，每段最多一个明显比喻，别连环堆意象
4. 别把每句话都写成“金句”，保留普通过渡句和生活化连接词
5. 对话允许停顿、打断和半句收口，贴近中国人真实聊天节奏
6. 去掉模板化总结和口号腔，少用“总之/事实上/值得注意的是”
7. 保留少量不完美表达和情绪毛边，让人物声音有区分度
8. 网络热梗仅可偶发点缀，优先给配角使用，必须贴场景且不过量$$, COALESCE((SELECT MAX(order_index) FROM writing_styles WHERE user_id IS NULL), 0) + 1
WHERE NOT EXISTS (SELECT 1 FROM writing_styles WHERE user_id IS NULL AND preset_id = 'low_ai_life')"#;

const LOW_AI_LIFE_V2_UPDATE_SQL: &str = r#"UPDATE writing_styles
SET name = '低AI生活化', style_type = 'preset', description = '低AI感的生活化叙事，强调口语自然、节奏起伏与去工整感', prompt_content = $$写作风格建议：
1. 叙述像真人在讲亲历故事，口语自然，不要写成说明文
2. 句式长短交替，快节奏处用短句，情绪段落允许稍慢一点
3. 控制修辞密度，每段最多一个明显比喻，别连环堆意象
4. 别把每句话都写成“金句”，保留普通过渡句和生活化连接词
5. 对话允许停顿、打断和半句收口，贴近中国人真实聊天节奏
6. 去掉模板化总结和口号腔，少用“总之/事实上/值得注意的是”
7. 保留少量不完美表达和情绪毛边，让人物声音有区分度
8. 网络热梗仅可偶发点缀，优先给配角使用，必须贴场景且不过量$$
WHERE id = (SELECT id FROM writing_styles WHERE user_id IS NULL AND preset_id = 'low_ai_life' LIMIT 1)"#;

const LOW_AI_LIFE_V1_UPDATE_SQL: &str = r#"UPDATE writing_styles
SET name = '低AI生活化', style_type = 'preset', description = '低AI感的生活化叙事，强调中文口语自然度与长短句节奏', prompt_content = $$写作风格建议：
1. 叙述像真人在讲亲历故事，口语自然，不要写成说明文
2. 句式长短交替：推进情节用短句提速，情绪段落可适当放长
3. 去掉机械排比和总结腔，少用“总之/事实上/值得注意的是”等套话
4. 对话贴近日常中文，保留人物各自的说话习惯和小毛边
5. 能用动作和细节表达，就别改成抽象解释，让情绪自己落地
6. 网络热梗可少量使用，必须贴场景、贴人物，避免硬塞和连续刷梗$$
WHERE id = (SELECT id FROM writing_styles WHERE user_id IS NULL AND preset_id = 'low_ai_life' LIMIT 1)"#;

const LOW_AI_LIFE_V1_INSERT_SQL: &str = r#"INSERT INTO writing_styles (user_id, name, style_type, preset_id, description, prompt_content, order_index)
SELECT NULL, '低AI生活化', 'preset', 'low_ai_life', '低AI感的生活化叙事，强调中文口语自然度与长短句节奏', $$写作风格建议：
1. 叙述像真人在讲亲历故事，口语自然，不要写成说明文
2. 句式长短交替：推进情节用短句提速，情绪段落可适当放长
3. 去掉机械排比和总结腔，少用“总之/事实上/值得注意的是”等套话
4. 对话贴近日常中文，保留人物各自的说话习惯和小毛边
5. 能用动作和细节表达，就别改成抽象解释，让情绪自己落地
6. 网络热梗可少量使用，必须贴场景、贴人物，避免硬塞和连续刷梗$$, COALESCE((SELECT MAX(order_index) FROM writing_styles WHERE user_id IS NULL), 0) + 1
WHERE NOT EXISTS (SELECT 1 FROM writing_styles WHERE user_id IS NULL AND preset_id = 'low_ai_life')"#;

const LOW_AI_SERIAL_UPDATE_SQL: &str = r#"UPDATE writing_styles
SET name = '低AI连载感', style_type = 'preset', description = '低AI感的网文连载风格，强调现场感、自然口语和非工整节奏', prompt_content = $$写作风格建议：
1. 叙述要有“现场正在发生”的感觉，少解释，多让动作和反应自己说话
2. 句式长短交替，关键推进处用短句，情绪过渡用中句，别整段同长度
3. 段落允许有轻微粗粝感，不追求句句漂亮，优先保证可读和代入
4. 别把每句话都打磨成金句，保留自然的过渡句和口头连接词
5. 对话贴近日常中文，允许停顿、打断、欲言又止，角色语气要分开
6. 修辞要克制，每段最多一个明显比喻，避免连续堆意象
7. 热梗仅可偶发点缀，优先放在配角台词，必须贴场景且不过量
8. 章末可留轻钩子，但不要生硬反转，保持“下一章想看”的顺滑感$$
WHERE id = (SELECT id FROM writing_styles WHERE user_id IS NULL AND preset_id = 'low_ai_serial' LIMIT 1)"#;

const LOW_AI_SERIAL_INSERT_SQL: &str = r#"INSERT INTO writing_styles (user_id, name, style_type, preset_id, description, prompt_content, order_index)
SELECT NULL, '低AI连载感', 'preset', 'low_ai_serial', '低AI感的网文连载风格，强调现场感、自然口语和非工整节奏', $$写作风格建议：
1. 叙述要有“现场正在发生”的感觉，少解释，多让动作和反应自己说话
2. 句式长短交替，关键推进处用短句，情绪过渡用中句，别整段同长度
3. 段落允许有轻微粗粝感，不追求句句漂亮，优先保证可读和代入
4. 别把每句话都打磨成金句，保留自然的过渡句和口头连接词
5. 对话贴近日常中文，允许停顿、打断、欲言又止，角色语气要分开
6. 修辞要克制，每段最多一个明显比喻，避免连续堆意象
7. 热梗仅可偶发点缀，优先放在配角台词，必须贴场景且不过量
8. 章末可留轻钩子，但不要生硬反转，保持“下一章想看”的顺滑感$$, COALESCE((SELECT MAX(order_index) FROM writing_styles WHERE user_id IS NULL), 0) + 1
WHERE NOT EXISTS (SELECT 1 FROM writing_styles WHERE user_id IS NULL AND preset_id = 'low_ai_serial')"#;

const WRITING_STYLE_LOW_AI_LIFE_INSERT_UPGRADE_STEPS: &[RustMigrationSqlStep] =
    &[RustMigrationSqlStep {
        sql: LOW_AI_LIFE_INSERT_SQL,
        statement_kind: "data_insert_if_missing",
    }];

const WRITING_STYLE_LOW_AI_LIFE_INSERT_DOWNGRADE_STEPS: &[RustMigrationSqlStep] =
    &[RustMigrationSqlStep {
        sql: "DELETE FROM writing_styles WHERE user_id IS NULL AND preset_id = 'low_ai_life'",
        statement_kind: "data_delete",
    }];

const WRITING_STYLE_LOW_AI_LIFE_V2_UPGRADE_STEPS: &[RustMigrationSqlStep] = &[
    RustMigrationSqlStep {
        sql: LOW_AI_LIFE_V2_UPDATE_SQL,
        statement_kind: "data_update_if_exists",
    },
    RustMigrationSqlStep {
        sql: LOW_AI_LIFE_INSERT_SQL,
        statement_kind: "data_insert_if_missing",
    },
];

const WRITING_STYLE_LOW_AI_LIFE_V2_DOWNGRADE_STEPS: &[RustMigrationSqlStep] = &[
    RustMigrationSqlStep {
        sql: LOW_AI_LIFE_V1_UPDATE_SQL,
        statement_kind: "data_update_if_exists",
    },
    RustMigrationSqlStep {
        sql: LOW_AI_LIFE_V1_INSERT_SQL,
        statement_kind: "data_insert_if_missing",
    },
];

const WRITING_STYLE_LOW_AI_SERIAL_UPGRADE_STEPS: &[RustMigrationSqlStep] = &[
    RustMigrationSqlStep {
        sql: LOW_AI_SERIAL_UPDATE_SQL,
        statement_kind: "data_update_if_exists",
    },
    RustMigrationSqlStep {
        sql: LOW_AI_SERIAL_INSERT_SQL,
        statement_kind: "data_insert_if_missing",
    },
];

const WRITING_STYLE_LOW_AI_SERIAL_DOWNGRADE_STEPS: &[RustMigrationSqlStep] =
    &[RustMigrationSqlStep {
        sql: "DELETE FROM writing_styles WHERE user_id IS NULL AND preset_id = 'low_ai_serial'",
        statement_kind: "data_delete",
    }];

const SETTINGS_API_COMPAT_UPGRADE_STEPS: &[RustMigrationSqlStep] = &[
    RustMigrationSqlStep {
        sql: "ALTER TABLE settings ADD COLUMN api_backup_urls TEXT",
        statement_kind: "ddl_add_column",
    },
    RustMigrationSqlStep {
        sql: "ALTER TABLE settings ADD COLUMN provider_type VARCHAR(50) DEFAULT 'openai'",
        statement_kind: "ddl_add_column",
    },
    RustMigrationSqlStep {
        sql: "ALTER TABLE settings ADD COLUMN fallback_strategy VARCHAR(20) DEFAULT 'auto'",
        statement_kind: "ddl_add_column",
    },
    RustMigrationSqlStep {
        sql: "ALTER TABLE settings ADD COLUMN azure_api_version VARCHAR(50)",
        statement_kind: "ddl_add_column",
    },
];

const SETTINGS_API_COMPAT_DOWNGRADE_STEPS: &[RustMigrationSqlStep] = &[
    RustMigrationSqlStep {
        sql: "ALTER TABLE settings DROP COLUMN azure_api_version",
        statement_kind: "ddl_drop_column",
    },
    RustMigrationSqlStep {
        sql: "ALTER TABLE settings DROP COLUMN fallback_strategy",
        statement_kind: "ddl_drop_column",
    },
    RustMigrationSqlStep {
        sql: "ALTER TABLE settings DROP COLUMN provider_type",
        statement_kind: "ddl_drop_column",
    },
    RustMigrationSqlStep {
        sql: "ALTER TABLE settings DROP COLUMN api_backup_urls",
        statement_kind: "ddl_drop_column",
    },
];

const CHARACTER_STATE_TRACKING_UPGRADE_STEPS: &[RustMigrationSqlStep] = &[
    RustMigrationSqlStep {
        sql: "ALTER TABLE characters ADD COLUMN status VARCHAR(20)",
        statement_kind: "ddl_add_column",
    },
    RustMigrationSqlStep {
        sql: "ALTER TABLE characters ADD COLUMN status_changed_chapter INTEGER",
        statement_kind: "ddl_add_column",
    },
    RustMigrationSqlStep {
        sql: "ALTER TABLE characters ADD COLUMN current_state TEXT",
        statement_kind: "ddl_add_column",
    },
    RustMigrationSqlStep {
        sql: "ALTER TABLE characters ADD COLUMN state_updated_chapter INTEGER",
        statement_kind: "ddl_add_column",
    },
];

const CHARACTER_STATE_TRACKING_DOWNGRADE_STEPS: &[RustMigrationSqlStep] = &[
    RustMigrationSqlStep {
        sql: "ALTER TABLE characters DROP COLUMN state_updated_chapter",
        statement_kind: "ddl_drop_column",
    },
    RustMigrationSqlStep {
        sql: "ALTER TABLE characters DROP COLUMN current_state",
        statement_kind: "ddl_drop_column",
    },
    RustMigrationSqlStep {
        sql: "ALTER TABLE characters DROP COLUMN status_changed_chapter",
        statement_kind: "ddl_drop_column",
    },
    RustMigrationSqlStep {
        sql: "ALTER TABLE characters DROP COLUMN status",
        statement_kind: "ddl_drop_column",
    },
];

const PROMPT_WORKSHOP_UPGRADE_STEPS: &[RustMigrationSqlStep] = &[
    RustMigrationSqlStep {
        sql: "CREATE TABLE prompt_submissions (
    id VARCHAR(36) NOT NULL,
    submitter_id VARCHAR(255) NOT NULL,
    submitter_name VARCHAR(100),
    source_instance VARCHAR(255) NOT NULL,
    name VARCHAR(100) NOT NULL,
    description TEXT,
    prompt_content TEXT NOT NULL,
    category VARCHAR(50),
    tags JSON,
    author_display_name VARCHAR(100),
    is_anonymous BOOLEAN,
    status VARCHAR(20),
    reviewer_id VARCHAR(100),
    review_note TEXT,
    reviewed_at TIMESTAMP,
    workshop_item_id VARCHAR(36),
    created_at TIMESTAMP DEFAULT now(),
    updated_at TIMESTAMP DEFAULT now(),
    PRIMARY KEY (id)
)",
        statement_kind: "ddl_create_table",
    },
    RustMigrationSqlStep {
        sql: "CREATE INDEX idx_submissions_created_at ON prompt_submissions (created_at)",
        statement_kind: "ddl_create_index",
    },
    RustMigrationSqlStep {
        sql: "CREATE INDEX idx_submissions_source ON prompt_submissions (source_instance)",
        statement_kind: "ddl_create_index",
    },
    RustMigrationSqlStep {
        sql: "CREATE INDEX idx_submissions_status ON prompt_submissions (status)",
        statement_kind: "ddl_create_index",
    },
    RustMigrationSqlStep {
        sql: "CREATE INDEX idx_submissions_submitter ON prompt_submissions (submitter_id)",
        statement_kind: "ddl_create_index",
    },
    RustMigrationSqlStep {
        sql: "CREATE TABLE prompt_workshop_items (
    id VARCHAR(36) NOT NULL,
    name VARCHAR(100) NOT NULL,
    description TEXT,
    prompt_content TEXT NOT NULL,
    category VARCHAR(50),
    tags JSON,
    author_id VARCHAR(255),
    author_name VARCHAR(100),
    source_instance VARCHAR(255),
    is_official BOOLEAN,
    download_count INTEGER,
    like_count INTEGER,
    status VARCHAR(20),
    created_at TIMESTAMP DEFAULT now(),
    updated_at TIMESTAMP DEFAULT now(),
    PRIMARY KEY (id)
)",
        statement_kind: "ddl_create_table",
    },
    RustMigrationSqlStep {
        sql: "CREATE INDEX idx_workshop_items_category ON prompt_workshop_items (category)",
        statement_kind: "ddl_create_index",
    },
    RustMigrationSqlStep {
        sql: "CREATE INDEX idx_workshop_items_created_at ON prompt_workshop_items (created_at)",
        statement_kind: "ddl_create_index",
    },
    RustMigrationSqlStep {
        sql: "CREATE INDEX idx_workshop_items_download_count ON prompt_workshop_items (download_count)",
        statement_kind: "ddl_create_index",
    },
    RustMigrationSqlStep {
        sql: "CREATE INDEX idx_workshop_items_status ON prompt_workshop_items (status)",
        statement_kind: "ddl_create_index",
    },
    RustMigrationSqlStep {
        sql: "CREATE TABLE prompt_workshop_likes (
    id VARCHAR(36) NOT NULL,
    user_identifier VARCHAR(255) NOT NULL,
    workshop_item_id VARCHAR(36) NOT NULL,
    created_at TIMESTAMP DEFAULT now(),
    PRIMARY KEY (id),
    FOREIGN KEY(workshop_item_id) REFERENCES prompt_workshop_items (id) ON DELETE CASCADE
)",
        statement_kind: "ddl_create_table",
    },
    RustMigrationSqlStep {
        sql: "CREATE UNIQUE INDEX idx_likes_user_item ON prompt_workshop_likes (user_identifier, workshop_item_id)",
        statement_kind: "ddl_create_index",
    },
];

const PROMPT_WORKSHOP_DOWNGRADE_STEPS: &[RustMigrationSqlStep] = &[
    RustMigrationSqlStep {
        sql: "DROP INDEX idx_likes_user_item",
        statement_kind: "ddl_drop_index",
    },
    RustMigrationSqlStep {
        sql: "DROP TABLE prompt_workshop_likes",
        statement_kind: "ddl_drop_table",
    },
    RustMigrationSqlStep {
        sql: "DROP INDEX idx_workshop_items_status",
        statement_kind: "ddl_drop_index",
    },
    RustMigrationSqlStep {
        sql: "DROP INDEX idx_workshop_items_download_count",
        statement_kind: "ddl_drop_index",
    },
    RustMigrationSqlStep {
        sql: "DROP INDEX idx_workshop_items_created_at",
        statement_kind: "ddl_drop_index",
    },
    RustMigrationSqlStep {
        sql: "DROP INDEX idx_workshop_items_category",
        statement_kind: "ddl_drop_index",
    },
    RustMigrationSqlStep {
        sql: "DROP TABLE prompt_workshop_items",
        statement_kind: "ddl_drop_table",
    },
    RustMigrationSqlStep {
        sql: "DROP INDEX idx_submissions_submitter",
        statement_kind: "ddl_drop_index",
    },
    RustMigrationSqlStep {
        sql: "DROP INDEX idx_submissions_status",
        statement_kind: "ddl_drop_index",
    },
    RustMigrationSqlStep {
        sql: "DROP INDEX idx_submissions_source",
        statement_kind: "ddl_drop_index",
    },
    RustMigrationSqlStep {
        sql: "DROP INDEX idx_submissions_created_at",
        statement_kind: "ddl_drop_index",
    },
    RustMigrationSqlStep {
        sql: "DROP TABLE prompt_submissions",
        statement_kind: "ddl_drop_table",
    },
];

const FORESHADOWS_TABLE_UPGRADE_STEPS: &[RustMigrationSqlStep] = &[
    RustMigrationSqlStep {
        sql: "CREATE TABLE foreshadows (
    id VARCHAR(36) NOT NULL,
    project_id VARCHAR(36) NOT NULL,
    title VARCHAR(200) NOT NULL,
    content TEXT NOT NULL,
    hint_text TEXT,
    resolution_text TEXT,
    source_type VARCHAR(20),
    source_memory_id VARCHAR(100),
    source_analysis_id VARCHAR(36),
    plant_chapter_id VARCHAR(36),
    plant_chapter_number INTEGER,
    target_resolve_chapter_id VARCHAR(36),
    target_resolve_chapter_number INTEGER,
    actual_resolve_chapter_id VARCHAR(36),
    actual_resolve_chapter_number INTEGER,
    status VARCHAR(20),
    is_long_term BOOLEAN,
    importance FLOAT,
    strength INTEGER,
    subtlety INTEGER,
    urgency INTEGER,
    related_characters JSON,
    related_foreshadow_ids JSON,
    tags JSON,
    category VARCHAR(50),
    notes TEXT,
    resolution_notes TEXT,
    auto_remind BOOLEAN,
    remind_before_chapters INTEGER,
    include_in_context BOOLEAN,
    created_at TIMESTAMP DEFAULT now(),
    updated_at TIMESTAMP DEFAULT now(),
    planted_at TIMESTAMP,
    resolved_at TIMESTAMP,
    PRIMARY KEY (id),
    FOREIGN KEY(actual_resolve_chapter_id) REFERENCES chapters (id) ON DELETE SET NULL,
    FOREIGN KEY(plant_chapter_id) REFERENCES chapters (id) ON DELETE SET NULL,
    FOREIGN KEY(project_id) REFERENCES projects (id) ON DELETE CASCADE,
    FOREIGN KEY(target_resolve_chapter_id) REFERENCES chapters (id) ON DELETE SET NULL
)",
        statement_kind: "ddl_create_table",
    },
    RustMigrationSqlStep {
        sql: "CREATE INDEX ix_foreshadows_project_id ON foreshadows (project_id)",
        statement_kind: "ddl_create_index",
    },
    RustMigrationSqlStep {
        sql: "CREATE INDEX ix_foreshadows_status ON foreshadows (status)",
        statement_kind: "ddl_create_index",
    },
];

const FORESHADOWS_TABLE_DOWNGRADE_STEPS: &[RustMigrationSqlStep] = &[
    RustMigrationSqlStep {
        sql: "DROP INDEX ix_foreshadows_status",
        statement_kind: "ddl_drop_index",
    },
    RustMigrationSqlStep {
        sql: "DROP INDEX ix_foreshadows_project_id",
        statement_kind: "ddl_drop_index",
    },
    RustMigrationSqlStep {
        sql: "DROP TABLE foreshadows",
        statement_kind: "ddl_drop_table",
    },
];

const SETTINGS_SYSTEM_PROMPT_UPGRADE_STEPS: &[RustMigrationSqlStep] = &[RustMigrationSqlStep {
    sql: "ALTER TABLE settings ADD COLUMN system_prompt TEXT",
    statement_kind: "ddl_add_column",
}];

const SETTINGS_SYSTEM_PROMPT_DOWNGRADE_STEPS: &[RustMigrationSqlStep] = &[RustMigrationSqlStep {
    sql: "ALTER TABLE settings DROP COLUMN system_prompt",
    statement_kind: "ddl_drop_column",
}];

const PROJECT_GENERATION_DEFAULTS_UPGRADE_STEPS: &[RustMigrationSqlStep] = &[
    RustMigrationSqlStep {
        sql: "ALTER TABLE projects ADD COLUMN default_creative_mode VARCHAR(50)",
        statement_kind: "ddl_add_column",
    },
    RustMigrationSqlStep {
        sql: "ALTER TABLE projects ADD COLUMN default_story_focus VARCHAR(50)",
        statement_kind: "ddl_add_column",
    },
    RustMigrationSqlStep {
        sql: "ALTER TABLE projects ADD COLUMN default_plot_stage VARCHAR(20)",
        statement_kind: "ddl_add_column",
    },
    RustMigrationSqlStep {
        sql: "ALTER TABLE projects ADD COLUMN default_story_creation_brief TEXT",
        statement_kind: "ddl_add_column",
    },
];

const PROJECT_GENERATION_DEFAULTS_DOWNGRADE_STEPS: &[RustMigrationSqlStep] = &[
    RustMigrationSqlStep {
        sql: "ALTER TABLE projects DROP COLUMN default_story_creation_brief",
        statement_kind: "ddl_drop_column",
    },
    RustMigrationSqlStep {
        sql: "ALTER TABLE projects DROP COLUMN default_plot_stage",
        statement_kind: "ddl_drop_column",
    },
    RustMigrationSqlStep {
        sql: "ALTER TABLE projects DROP COLUMN default_story_focus",
        statement_kind: "ddl_drop_column",
    },
    RustMigrationSqlStep {
        sql: "ALTER TABLE projects DROP COLUMN default_creative_mode",
        statement_kind: "ddl_drop_column",
    },
];

const PROJECT_QUALITY_PREFS_UPGRADE_STEPS: &[RustMigrationSqlStep] = &[
    RustMigrationSqlStep {
        sql: "ALTER TABLE projects ADD COLUMN default_quality_preset VARCHAR(50)",
        statement_kind: "ddl_add_column",
    },
    RustMigrationSqlStep {
        sql: "ALTER TABLE projects ADD COLUMN default_quality_notes TEXT",
        statement_kind: "ddl_add_column",
    },
];

const PROJECT_QUALITY_PREFS_DOWNGRADE_STEPS: &[RustMigrationSqlStep] = &[
    RustMigrationSqlStep {
        sql: "ALTER TABLE projects DROP COLUMN default_quality_notes",
        statement_kind: "ddl_drop_column",
    },
    RustMigrationSqlStep {
        sql: "ALTER TABLE projects DROP COLUMN default_quality_preset",
        statement_kind: "ddl_drop_column",
    },
];

const BATCH_RUNTIME_STORE_UPGRADE_STEPS: &[RustMigrationSqlStep] = &[
    RustMigrationSqlStep {
        sql: "CREATE TABLE chapter_draft_attempts (id VARCHAR(36) NOT NULL, project_id VARCHAR(36) NOT NULL, chapter_id VARCHAR(36), batch_task_id VARCHAR(36), source VARCHAR(40) DEFAULT 'chapter' NOT NULL, attempt_state VARCHAR(40) DEFAULT 'candidate' NOT NULL, quality_gate_action VARCHAR(40), quality_gate_decision VARCHAR(40), word_count INTEGER DEFAULT 0 NOT NULL, summary_preview TEXT, content_preview TEXT, quality_metrics JSON, repair_payload JSON, created_at TIMESTAMP WITHOUT TIME ZONE DEFAULT CURRENT_TIMESTAMP, PRIMARY KEY (id), FOREIGN KEY(batch_task_id) REFERENCES batch_generation_tasks (id) ON DELETE SET NULL, FOREIGN KEY(chapter_id) REFERENCES chapters (id) ON DELETE SET NULL, FOREIGN KEY(project_id) REFERENCES projects (id) ON DELETE CASCADE)",
        statement_kind: "ddl_create_table",
    },
    RustMigrationSqlStep {
        sql: "CREATE TABLE batch_generation_snapshots (id VARCHAR(36) NOT NULL, batch_task_id VARCHAR(36) NOT NULL, latest_quality_metrics JSON, quality_metrics_history JSON, quality_metrics_summary JSON, created_at TIMESTAMP WITHOUT TIME ZONE DEFAULT CURRENT_TIMESTAMP, updated_at TIMESTAMP WITHOUT TIME ZONE DEFAULT CURRENT_TIMESTAMP, PRIMARY KEY (id), FOREIGN KEY(batch_task_id) REFERENCES batch_generation_tasks (id) ON DELETE CASCADE, UNIQUE (batch_task_id))",
        statement_kind: "ddl_create_table",
    },
    RustMigrationSqlStep {
        sql: "CREATE INDEX ix_chapter_draft_attempts_project_id ON chapter_draft_attempts (project_id)",
        statement_kind: "ddl_create_index",
    },
    RustMigrationSqlStep {
        sql: "CREATE INDEX ix_chapter_draft_attempts_chapter_id ON chapter_draft_attempts (chapter_id)",
        statement_kind: "ddl_create_index",
    },
    RustMigrationSqlStep {
        sql: "CREATE INDEX ix_chapter_draft_attempts_batch_task_id ON chapter_draft_attempts (batch_task_id)",
        statement_kind: "ddl_create_index",
    },
];

const BATCH_RUNTIME_STORE_DOWNGRADE_STEPS: &[RustMigrationSqlStep] = &[
    RustMigrationSqlStep {
        sql: "DROP INDEX ix_chapter_draft_attempts_batch_task_id",
        statement_kind: "ddl_drop_index",
    },
    RustMigrationSqlStep {
        sql: "DROP INDEX ix_chapter_draft_attempts_chapter_id",
        statement_kind: "ddl_drop_index",
    },
    RustMigrationSqlStep {
        sql: "DROP INDEX ix_chapter_draft_attempts_project_id",
        statement_kind: "ddl_drop_index",
    },
    RustMigrationSqlStep {
        sql: "DROP TABLE batch_generation_snapshots",
        statement_kind: "ddl_drop_table",
    },
    RustMigrationSqlStep {
        sql: "DROP TABLE chapter_draft_attempts",
        statement_kind: "ddl_drop_table",
    },
];

const BATCH_WORKFLOW_STATE_UPGRADE_STEPS: &[RustMigrationSqlStep] = &[RustMigrationSqlStep {
    sql: "ALTER TABLE batch_generation_snapshots ADD COLUMN workflow_runtime_state JSON",
    statement_kind: "ddl_add_column",
}];

const BATCH_WORKFLOW_STATE_DOWNGRADE_STEPS: &[RustMigrationSqlStep] = &[RustMigrationSqlStep {
    sql: "ALTER TABLE batch_generation_snapshots DROP COLUMN workflow_runtime_state",
    statement_kind: "ddl_drop_column",
}];

const ANALYSIS_TASK_HARDENING_UPGRADE_STEPS: &[RustMigrationSqlStep] = &[
    RustMigrationSqlStep {
        sql: "UPDATE analysis_tasks SET progress = 0 WHERE progress IS NULL",
        statement_kind: "data_backfill",
    },
    RustMigrationSqlStep {
        sql: "UPDATE analysis_tasks SET status = 'pending' WHERE status IS NULL",
        statement_kind: "data_backfill",
    },
    RustMigrationSqlStep {
        sql: "ALTER TABLE analysis_tasks ALTER COLUMN progress SET DEFAULT 0",
        statement_kind: "ddl_alter_default",
    },
    RustMigrationSqlStep {
        sql: "ALTER TABLE analysis_tasks ALTER COLUMN progress SET NOT NULL",
        statement_kind: "ddl_set_not_null",
    },
    RustMigrationSqlStep {
        sql: "ALTER TABLE analysis_tasks ALTER COLUMN status SET DEFAULT 'pending'",
        statement_kind: "ddl_alter_default",
    },
    RustMigrationSqlStep {
        sql: "ALTER TABLE analysis_tasks ALTER COLUMN status SET NOT NULL",
        statement_kind: "ddl_set_not_null",
    },
];

const ANALYSIS_TASK_HARDENING_DOWNGRADE_STEPS: &[RustMigrationSqlStep] = &[
    RustMigrationSqlStep {
        sql: "ALTER TABLE analysis_tasks ALTER COLUMN status DROP DEFAULT",
        statement_kind: "ddl_drop_default",
    },
    RustMigrationSqlStep {
        sql: "ALTER TABLE analysis_tasks ALTER COLUMN progress DROP NOT NULL",
        statement_kind: "ddl_drop_not_null",
    },
    RustMigrationSqlStep {
        sql: "ALTER TABLE analysis_tasks ALTER COLUMN progress DROP DEFAULT",
        statement_kind: "ddl_drop_default",
    },
];

const BATCH_TASK_DEFAULTS_UPGRADE_STEPS: &[RustMigrationSqlStep] = &[
    RustMigrationSqlStep {
        sql: "UPDATE batch_generation_tasks SET target_word_count = 3000 WHERE target_word_count IS NULL",
        statement_kind: "data_backfill",
    },
    RustMigrationSqlStep {
        sql: "UPDATE batch_generation_tasks SET enable_analysis = false WHERE enable_analysis IS NULL",
        statement_kind: "data_backfill",
    },
    RustMigrationSqlStep {
        sql: "UPDATE batch_generation_tasks SET status = 'pending' WHERE status IS NULL",
        statement_kind: "data_backfill",
    },
    RustMigrationSqlStep {
        sql: "UPDATE batch_generation_tasks SET total_chapters = 0 WHERE total_chapters IS NULL",
        statement_kind: "data_backfill",
    },
    RustMigrationSqlStep {
        sql: "UPDATE batch_generation_tasks SET completed_chapters = 0 WHERE completed_chapters IS NULL",
        statement_kind: "data_backfill",
    },
    RustMigrationSqlStep {
        sql: "UPDATE batch_generation_tasks SET failed_chapters = '[]'::json WHERE failed_chapters IS NULL",
        statement_kind: "data_backfill",
    },
    RustMigrationSqlStep {
        sql: "UPDATE batch_generation_tasks SET current_retry_count = 0 WHERE current_retry_count IS NULL",
        statement_kind: "data_backfill",
    },
    RustMigrationSqlStep {
        sql: "UPDATE batch_generation_tasks SET max_retries = 3 WHERE max_retries IS NULL",
        statement_kind: "data_backfill",
    },
    RustMigrationSqlStep {
        sql: "ALTER TABLE batch_generation_tasks ALTER COLUMN target_word_count SET DEFAULT 3000",
        statement_kind: "ddl_alter_default",
    },
    RustMigrationSqlStep {
        sql: "ALTER TABLE batch_generation_tasks ALTER COLUMN target_word_count SET NOT NULL",
        statement_kind: "ddl_set_not_null",
    },
    RustMigrationSqlStep {
        sql: "ALTER TABLE batch_generation_tasks ALTER COLUMN enable_analysis SET DEFAULT false",
        statement_kind: "ddl_alter_default",
    },
    RustMigrationSqlStep {
        sql: "ALTER TABLE batch_generation_tasks ALTER COLUMN enable_analysis SET NOT NULL",
        statement_kind: "ddl_set_not_null",
    },
    RustMigrationSqlStep {
        sql: "ALTER TABLE batch_generation_tasks ALTER COLUMN status SET DEFAULT 'pending'",
        statement_kind: "ddl_alter_default",
    },
    RustMigrationSqlStep {
        sql: "ALTER TABLE batch_generation_tasks ALTER COLUMN status SET NOT NULL",
        statement_kind: "ddl_set_not_null",
    },
    RustMigrationSqlStep {
        sql: "ALTER TABLE batch_generation_tasks ALTER COLUMN total_chapters SET DEFAULT 0",
        statement_kind: "ddl_alter_default",
    },
    RustMigrationSqlStep {
        sql: "ALTER TABLE batch_generation_tasks ALTER COLUMN total_chapters SET NOT NULL",
        statement_kind: "ddl_set_not_null",
    },
    RustMigrationSqlStep {
        sql: "ALTER TABLE batch_generation_tasks ALTER COLUMN completed_chapters SET DEFAULT 0",
        statement_kind: "ddl_alter_default",
    },
    RustMigrationSqlStep {
        sql: "ALTER TABLE batch_generation_tasks ALTER COLUMN completed_chapters SET NOT NULL",
        statement_kind: "ddl_set_not_null",
    },
    RustMigrationSqlStep {
        sql: "ALTER TABLE batch_generation_tasks ALTER COLUMN failed_chapters SET DEFAULT '[]'::json",
        statement_kind: "ddl_alter_default",
    },
    RustMigrationSqlStep {
        sql: "ALTER TABLE batch_generation_tasks ALTER COLUMN failed_chapters SET NOT NULL",
        statement_kind: "ddl_set_not_null",
    },
    RustMigrationSqlStep {
        sql: "ALTER TABLE batch_generation_tasks ALTER COLUMN current_retry_count SET DEFAULT 0",
        statement_kind: "ddl_alter_default",
    },
    RustMigrationSqlStep {
        sql: "ALTER TABLE batch_generation_tasks ALTER COLUMN current_retry_count SET NOT NULL",
        statement_kind: "ddl_set_not_null",
    },
    RustMigrationSqlStep {
        sql: "ALTER TABLE batch_generation_tasks ALTER COLUMN max_retries SET DEFAULT 3",
        statement_kind: "ddl_alter_default",
    },
    RustMigrationSqlStep {
        sql: "ALTER TABLE batch_generation_tasks ALTER COLUMN max_retries SET NOT NULL",
        statement_kind: "ddl_set_not_null",
    },
];

const BATCH_TASK_DEFAULTS_DOWNGRADE_STEPS: &[RustMigrationSqlStep] = &[
    RustMigrationSqlStep {
        sql: "ALTER TABLE batch_generation_tasks ALTER COLUMN max_retries DROP NOT NULL",
        statement_kind: "ddl_drop_not_null",
    },
    RustMigrationSqlStep {
        sql: "ALTER TABLE batch_generation_tasks ALTER COLUMN max_retries DROP DEFAULT",
        statement_kind: "ddl_drop_default",
    },
    RustMigrationSqlStep {
        sql: "ALTER TABLE batch_generation_tasks ALTER COLUMN current_retry_count DROP NOT NULL",
        statement_kind: "ddl_drop_not_null",
    },
    RustMigrationSqlStep {
        sql: "ALTER TABLE batch_generation_tasks ALTER COLUMN current_retry_count DROP DEFAULT",
        statement_kind: "ddl_drop_default",
    },
    RustMigrationSqlStep {
        sql: "ALTER TABLE batch_generation_tasks ALTER COLUMN completed_chapters DROP NOT NULL",
        statement_kind: "ddl_drop_not_null",
    },
    RustMigrationSqlStep {
        sql: "ALTER TABLE batch_generation_tasks ALTER COLUMN completed_chapters DROP DEFAULT",
        statement_kind: "ddl_drop_default",
    },
    RustMigrationSqlStep {
        sql: "ALTER TABLE batch_generation_tasks ALTER COLUMN failed_chapters DROP NOT NULL",
        statement_kind: "ddl_drop_not_null",
    },
    RustMigrationSqlStep {
        sql: "ALTER TABLE batch_generation_tasks ALTER COLUMN failed_chapters DROP DEFAULT",
        statement_kind: "ddl_drop_default",
    },
    RustMigrationSqlStep {
        sql: "ALTER TABLE batch_generation_tasks ALTER COLUMN total_chapters DROP NOT NULL",
        statement_kind: "ddl_drop_not_null",
    },
    RustMigrationSqlStep {
        sql: "ALTER TABLE batch_generation_tasks ALTER COLUMN total_chapters DROP DEFAULT",
        statement_kind: "ddl_drop_default",
    },
    RustMigrationSqlStep {
        sql: "ALTER TABLE batch_generation_tasks ALTER COLUMN status DROP NOT NULL",
        statement_kind: "ddl_drop_not_null",
    },
    RustMigrationSqlStep {
        sql: "ALTER TABLE batch_generation_tasks ALTER COLUMN status DROP DEFAULT",
        statement_kind: "ddl_drop_default",
    },
    RustMigrationSqlStep {
        sql: "ALTER TABLE batch_generation_tasks ALTER COLUMN enable_analysis DROP NOT NULL",
        statement_kind: "ddl_drop_not_null",
    },
    RustMigrationSqlStep {
        sql: "ALTER TABLE batch_generation_tasks ALTER COLUMN enable_analysis DROP DEFAULT",
        statement_kind: "ddl_drop_default",
    },
    RustMigrationSqlStep {
        sql: "ALTER TABLE batch_generation_tasks ALTER COLUMN target_word_count DROP NOT NULL",
        statement_kind: "ddl_drop_not_null",
    },
    RustMigrationSqlStep {
        sql: "ALTER TABLE batch_generation_tasks ALTER COLUMN target_word_count DROP DEFAULT",
        statement_kind: "ddl_drop_default",
    },
];

const REGENERATION_TASK_DEFAULTS_UPGRADE_STEPS: &[RustMigrationSqlStep] = &[
    RustMigrationSqlStep {
        sql:
            "UPDATE regeneration_tasks SET target_word_count = 3000 WHERE target_word_count IS NULL",
        statement_kind: "data_backfill",
    },
    RustMigrationSqlStep {
        sql: "UPDATE regeneration_tasks SET status = 'pending' WHERE status IS NULL",
        statement_kind: "data_backfill",
    },
    RustMigrationSqlStep {
        sql: "UPDATE regeneration_tasks SET progress = 0 WHERE progress IS NULL",
        statement_kind: "data_backfill",
    },
    RustMigrationSqlStep {
        sql: "UPDATE regeneration_tasks SET version_number = 1 WHERE version_number IS NULL",
        statement_kind: "data_backfill",
    },
    RustMigrationSqlStep {
        sql: "ALTER TABLE regeneration_tasks ALTER COLUMN target_word_count SET DEFAULT 3000",
        statement_kind: "ddl_alter_default",
    },
    RustMigrationSqlStep {
        sql: "ALTER TABLE regeneration_tasks ALTER COLUMN target_word_count SET NOT NULL",
        statement_kind: "ddl_set_not_null",
    },
    RustMigrationSqlStep {
        sql: "ALTER TABLE regeneration_tasks ALTER COLUMN status SET DEFAULT 'pending'",
        statement_kind: "ddl_alter_default",
    },
    RustMigrationSqlStep {
        sql: "ALTER TABLE regeneration_tasks ALTER COLUMN status SET NOT NULL",
        statement_kind: "ddl_set_not_null",
    },
    RustMigrationSqlStep {
        sql: "ALTER TABLE regeneration_tasks ALTER COLUMN progress SET DEFAULT 0",
        statement_kind: "ddl_alter_default",
    },
    RustMigrationSqlStep {
        sql: "ALTER TABLE regeneration_tasks ALTER COLUMN progress SET NOT NULL",
        statement_kind: "ddl_set_not_null",
    },
    RustMigrationSqlStep {
        sql: "ALTER TABLE regeneration_tasks ALTER COLUMN version_number SET DEFAULT 1",
        statement_kind: "ddl_alter_default",
    },
    RustMigrationSqlStep {
        sql: "ALTER TABLE regeneration_tasks ALTER COLUMN version_number SET NOT NULL",
        statement_kind: "ddl_set_not_null",
    },
];

const REGENERATION_TASK_DEFAULTS_DOWNGRADE_STEPS: &[RustMigrationSqlStep] = &[
    RustMigrationSqlStep {
        sql: "ALTER TABLE regeneration_tasks ALTER COLUMN version_number DROP NOT NULL",
        statement_kind: "ddl_drop_not_null",
    },
    RustMigrationSqlStep {
        sql: "ALTER TABLE regeneration_tasks ALTER COLUMN version_number DROP DEFAULT",
        statement_kind: "ddl_drop_default",
    },
    RustMigrationSqlStep {
        sql: "ALTER TABLE regeneration_tasks ALTER COLUMN progress DROP NOT NULL",
        statement_kind: "ddl_drop_not_null",
    },
    RustMigrationSqlStep {
        sql: "ALTER TABLE regeneration_tasks ALTER COLUMN progress DROP DEFAULT",
        statement_kind: "ddl_drop_default",
    },
    RustMigrationSqlStep {
        sql: "ALTER TABLE regeneration_tasks ALTER COLUMN status DROP NOT NULL",
        statement_kind: "ddl_drop_not_null",
    },
    RustMigrationSqlStep {
        sql: "ALTER TABLE regeneration_tasks ALTER COLUMN status DROP DEFAULT",
        statement_kind: "ddl_drop_default",
    },
    RustMigrationSqlStep {
        sql: "ALTER TABLE regeneration_tasks ALTER COLUMN target_word_count DROP NOT NULL",
        statement_kind: "ddl_drop_not_null",
    },
    RustMigrationSqlStep {
        sql: "ALTER TABLE regeneration_tasks ALTER COLUMN target_word_count DROP DEFAULT",
        statement_kind: "ddl_drop_default",
    },
];

const SETTINGS_CORE_DEFAULTS_UPGRADE_STEPS: &[RustMigrationSqlStep] = &[
    RustMigrationSqlStep {
        sql: "UPDATE settings SET api_provider = 'openai' WHERE api_provider IS NULL",
        statement_kind: "data_backfill",
    },
    RustMigrationSqlStep {
        sql: "UPDATE settings SET llm_model = 'gpt-4' WHERE llm_model IS NULL",
        statement_kind: "data_backfill",
    },
    RustMigrationSqlStep {
        sql: "UPDATE settings SET temperature = 0.7 WHERE temperature IS NULL",
        statement_kind: "data_backfill",
    },
    RustMigrationSqlStep {
        sql: "UPDATE settings SET max_tokens = 2000 WHERE max_tokens IS NULL",
        statement_kind: "data_backfill",
    },
    RustMigrationSqlStep {
        sql: "ALTER TABLE settings ALTER COLUMN api_provider SET DEFAULT 'openai'",
        statement_kind: "ddl_alter_default",
    },
    RustMigrationSqlStep {
        sql: "ALTER TABLE settings ALTER COLUMN api_provider SET NOT NULL",
        statement_kind: "ddl_set_not_null",
    },
    RustMigrationSqlStep {
        sql: "ALTER TABLE settings ALTER COLUMN llm_model SET DEFAULT 'gpt-4'",
        statement_kind: "ddl_alter_default",
    },
    RustMigrationSqlStep {
        sql: "ALTER TABLE settings ALTER COLUMN llm_model SET NOT NULL",
        statement_kind: "ddl_set_not_null",
    },
    RustMigrationSqlStep {
        sql: "ALTER TABLE settings ALTER COLUMN temperature SET DEFAULT 0.7",
        statement_kind: "ddl_alter_default",
    },
    RustMigrationSqlStep {
        sql: "ALTER TABLE settings ALTER COLUMN temperature SET NOT NULL",
        statement_kind: "ddl_set_not_null",
    },
    RustMigrationSqlStep {
        sql: "ALTER TABLE settings ALTER COLUMN max_tokens SET DEFAULT 2000",
        statement_kind: "ddl_alter_default",
    },
    RustMigrationSqlStep {
        sql: "ALTER TABLE settings ALTER COLUMN max_tokens SET NOT NULL",
        statement_kind: "ddl_set_not_null",
    },
];

const SETTINGS_CORE_DEFAULTS_DOWNGRADE_STEPS: &[RustMigrationSqlStep] = &[
    RustMigrationSqlStep {
        sql: "ALTER TABLE settings ALTER COLUMN max_tokens DROP NOT NULL",
        statement_kind: "ddl_drop_not_null",
    },
    RustMigrationSqlStep {
        sql: "ALTER TABLE settings ALTER COLUMN max_tokens DROP DEFAULT",
        statement_kind: "ddl_drop_default",
    },
    RustMigrationSqlStep {
        sql: "ALTER TABLE settings ALTER COLUMN temperature DROP NOT NULL",
        statement_kind: "ddl_drop_not_null",
    },
    RustMigrationSqlStep {
        sql: "ALTER TABLE settings ALTER COLUMN temperature DROP DEFAULT",
        statement_kind: "ddl_drop_default",
    },
    RustMigrationSqlStep {
        sql: "ALTER TABLE settings ALTER COLUMN llm_model DROP NOT NULL",
        statement_kind: "ddl_drop_not_null",
    },
    RustMigrationSqlStep {
        sql: "ALTER TABLE settings ALTER COLUMN llm_model DROP DEFAULT",
        statement_kind: "ddl_drop_default",
    },
    RustMigrationSqlStep {
        sql: "ALTER TABLE settings ALTER COLUMN api_provider DROP NOT NULL",
        statement_kind: "ddl_drop_not_null",
    },
    RustMigrationSqlStep {
        sql: "ALTER TABLE settings ALTER COLUMN api_provider DROP DEFAULT",
        statement_kind: "ddl_drop_default",
    },
];

const PROJECT_CORE_DEFAULTS_UPGRADE_STEPS: &[RustMigrationSqlStep] = &[
    RustMigrationSqlStep {
        sql: "UPDATE projects SET target_words = 0 WHERE target_words IS NULL",
        statement_kind: "data_backfill",
    },
    RustMigrationSqlStep {
        sql: "UPDATE projects SET current_words = 0 WHERE current_words IS NULL",
        statement_kind: "data_backfill",
    },
    RustMigrationSqlStep {
        sql: "UPDATE projects SET status = 'planning' WHERE status IS NULL",
        statement_kind: "data_backfill",
    },
    RustMigrationSqlStep {
        sql: "UPDATE projects SET wizard_status = 'incomplete' WHERE wizard_status IS NULL",
        statement_kind: "data_backfill",
    },
    RustMigrationSqlStep {
        sql: "UPDATE projects SET wizard_step = 0 WHERE wizard_step IS NULL",
        statement_kind: "data_backfill",
    },
    RustMigrationSqlStep {
        sql: "UPDATE projects SET character_count = 5 WHERE character_count IS NULL",
        statement_kind: "data_backfill",
    },
    RustMigrationSqlStep {
        sql: "ALTER TABLE projects ALTER COLUMN target_words SET DEFAULT 0",
        statement_kind: "ddl_alter_default",
    },
    RustMigrationSqlStep {
        sql: "ALTER TABLE projects ALTER COLUMN target_words SET NOT NULL",
        statement_kind: "ddl_set_not_null",
    },
    RustMigrationSqlStep {
        sql: "ALTER TABLE projects ALTER COLUMN current_words SET DEFAULT 0",
        statement_kind: "ddl_alter_default",
    },
    RustMigrationSqlStep {
        sql: "ALTER TABLE projects ALTER COLUMN current_words SET NOT NULL",
        statement_kind: "ddl_set_not_null",
    },
    RustMigrationSqlStep {
        sql: "ALTER TABLE projects ALTER COLUMN status SET DEFAULT 'planning'",
        statement_kind: "ddl_alter_default",
    },
    RustMigrationSqlStep {
        sql: "ALTER TABLE projects ALTER COLUMN status SET NOT NULL",
        statement_kind: "ddl_set_not_null",
    },
    RustMigrationSqlStep {
        sql: "ALTER TABLE projects ALTER COLUMN wizard_status SET DEFAULT 'incomplete'",
        statement_kind: "ddl_alter_default",
    },
    RustMigrationSqlStep {
        sql: "ALTER TABLE projects ALTER COLUMN wizard_status SET NOT NULL",
        statement_kind: "ddl_set_not_null",
    },
    RustMigrationSqlStep {
        sql: "ALTER TABLE projects ALTER COLUMN wizard_step SET DEFAULT 0",
        statement_kind: "ddl_alter_default",
    },
    RustMigrationSqlStep {
        sql: "ALTER TABLE projects ALTER COLUMN wizard_step SET NOT NULL",
        statement_kind: "ddl_set_not_null",
    },
    RustMigrationSqlStep {
        sql: "ALTER TABLE projects ALTER COLUMN outline_mode SET DEFAULT 'one-to-many'",
        statement_kind: "ddl_alter_default",
    },
    RustMigrationSqlStep {
        sql: "ALTER TABLE projects ALTER COLUMN outline_mode SET NOT NULL",
        statement_kind: "ddl_set_not_null",
    },
    RustMigrationSqlStep {
        sql: "ALTER TABLE projects ALTER COLUMN character_count SET DEFAULT 5",
        statement_kind: "ddl_alter_default",
    },
    RustMigrationSqlStep {
        sql: "ALTER TABLE projects ALTER COLUMN character_count SET NOT NULL",
        statement_kind: "ddl_set_not_null",
    },
];

const PROJECT_CORE_DEFAULTS_DOWNGRADE_STEPS: &[RustMigrationSqlStep] = &[
    RustMigrationSqlStep {
        sql: "ALTER TABLE projects ALTER COLUMN character_count DROP NOT NULL",
        statement_kind: "ddl_drop_not_null",
    },
    RustMigrationSqlStep {
        sql: "ALTER TABLE projects ALTER COLUMN character_count DROP DEFAULT",
        statement_kind: "ddl_drop_default",
    },
    RustMigrationSqlStep {
        sql: "ALTER TABLE projects ALTER COLUMN outline_mode DROP DEFAULT",
        statement_kind: "ddl_drop_default",
    },
    RustMigrationSqlStep {
        sql: "ALTER TABLE projects ALTER COLUMN wizard_step DROP NOT NULL",
        statement_kind: "ddl_drop_not_null",
    },
    RustMigrationSqlStep {
        sql: "ALTER TABLE projects ALTER COLUMN wizard_step DROP DEFAULT",
        statement_kind: "ddl_drop_default",
    },
    RustMigrationSqlStep {
        sql: "ALTER TABLE projects ALTER COLUMN wizard_status DROP NOT NULL",
        statement_kind: "ddl_drop_not_null",
    },
    RustMigrationSqlStep {
        sql: "ALTER TABLE projects ALTER COLUMN wizard_status DROP DEFAULT",
        statement_kind: "ddl_drop_default",
    },
    RustMigrationSqlStep {
        sql: "ALTER TABLE projects ALTER COLUMN status DROP NOT NULL",
        statement_kind: "ddl_drop_not_null",
    },
    RustMigrationSqlStep {
        sql: "ALTER TABLE projects ALTER COLUMN status DROP DEFAULT",
        statement_kind: "ddl_drop_default",
    },
    RustMigrationSqlStep {
        sql: "ALTER TABLE projects ALTER COLUMN current_words DROP NOT NULL",
        statement_kind: "ddl_drop_not_null",
    },
    RustMigrationSqlStep {
        sql: "ALTER TABLE projects ALTER COLUMN current_words DROP DEFAULT",
        statement_kind: "ddl_drop_default",
    },
    RustMigrationSqlStep {
        sql: "ALTER TABLE projects ALTER COLUMN target_words DROP NOT NULL",
        statement_kind: "ddl_drop_not_null",
    },
    RustMigrationSqlStep {
        sql: "ALTER TABLE projects ALTER COLUMN target_words DROP DEFAULT",
        statement_kind: "ddl_drop_default",
    },
];

const PASSWORD_HASH_PHC_TEXT_UPGRADE_STEPS: &[RustMigrationSqlStep] = &[
    RustMigrationSqlStep {
        sql: "ALTER TABLE user_passwords ALTER COLUMN password_hash TYPE TEXT",
        statement_kind: "ddl_alter_column_type",
    },
    RustMigrationSqlStep {
        sql: "COMMENT ON COLUMN user_passwords.password_hash IS '密码校验值（Argon2 PHC 或兼容的 legacy SHA256）'",
        statement_kind: "ddl_comment_column",
    },
];

const PASSWORD_HASH_PHC_TEXT_DOWNGRADE_STEPS: &[RustMigrationSqlStep] = &[
    RustMigrationSqlStep {
        sql: r#"DO $$
BEGIN
    IF EXISTS (
        SELECT 1
        FROM user_passwords
        WHERE length(password_hash) > 64
    ) THEN
        RAISE EXCEPTION
            'cannot downgrade password_hash to VARCHAR(64): long verifier exists';
    END IF;
END
$$"#,
        statement_kind: "ddl_guard_long_password_verifier",
    },
    RustMigrationSqlStep {
        sql: "ALTER TABLE user_passwords ALTER COLUMN password_hash TYPE VARCHAR(64)",
        statement_kind: "ddl_alter_column_type",
    },
    RustMigrationSqlStep {
        sql: "COMMENT ON COLUMN user_passwords.password_hash IS '密码哈希（SHA256）'",
        statement_kind: "ddl_comment_column",
    },
];

const AUTOPILOT_INVOCATION_AUDIT_UPGRADE_STEPS: &[RustMigrationSqlStep] = &[
    RustMigrationSqlStep {
        sql: r#"CREATE TABLE autopilot_invocation_audits (
    id VARCHAR(36) NOT NULL,
    task_id VARCHAR(36) NOT NULL,
    project_id VARCHAR(36) NOT NULL,
    actor_user_id VARCHAR(36) NOT NULL,
    schema_version VARCHAR(64) NOT NULL,
    tool_name VARCHAR(128) NOT NULL,
    tool_schema_version VARCHAR(64) NOT NULL,
    confirmed_by_user BOOLEAN NOT NULL,
    execution_mode VARCHAR(64) NOT NULL,
    provider_name TEXT,
    model_name TEXT,
    prompt_digest VARCHAR(80),
    input_digest VARCHAR(80) NOT NULL,
    input_summary TEXT NOT NULL,
    status VARCHAR(32) NOT NULL,
    result_summary TEXT,
    error_code VARCHAR(128),
    created_at TIMESTAMP WITHOUT TIME ZONE NOT NULL,
    started_at TIMESTAMP WITHOUT TIME ZONE,
    completed_at TIMESTAMP WITHOUT TIME ZONE,
    CONSTRAINT pk_autopilot_invocation_audits PRIMARY KEY (id),
    CONSTRAINT fk_autopilot_invocation_audits_project_id
        FOREIGN KEY (project_id) REFERENCES projects (id) ON DELETE CASCADE
)"#,
        statement_kind: "ddl_create_table",
    },
    RustMigrationSqlStep {
        sql: "CREATE UNIQUE INDEX uq_autopilot_invocation_audits_task_id ON autopilot_invocation_audits (task_id)",
        statement_kind: "ddl_create_unique_index",
    },
    RustMigrationSqlStep {
        sql: "CREATE INDEX ix_autopilot_invocation_audits_project_created_at ON autopilot_invocation_audits (project_id, created_at)",
        statement_kind: "ddl_create_index",
    },
];

const AUTOPILOT_INVOCATION_AUDIT_DOWNGRADE_STEPS: &[RustMigrationSqlStep] =
    &[RustMigrationSqlStep {
        sql: "DROP TABLE autopilot_invocation_audits",
        statement_kind: "ddl_drop_table",
    }];

const DURABLE_NOVEL_AUTOPILOT_UPGRADE_STEPS: &[RustMigrationSqlStep] = &[
    RustMigrationSqlStep {
        sql: r#"CREATE TABLE novel_autopilot_runs (
    id VARCHAR(36) NOT NULL,
    project_id VARCHAR(36) NOT NULL,
    user_id VARCHAR(36) NOT NULL,
    schema_version VARCHAR(64) NOT NULL,
    status VARCHAR(32) NOT NULL,
    current_phase VARCHAR(64) NOT NULL,
    current_step VARCHAR(128),
    active_scope_key VARCHAR(36),
    current_chapter_id VARCHAR(36),
    current_chapter_number INTEGER,
    total_chapters INTEGER NOT NULL,
    completed_chapters INTEGER NOT NULL,
    failed_chapters JSON NOT NULL,
    pending_rewrites JSON NOT NULL,
    total_word_count BIGINT NOT NULL,
    execution_scope VARCHAR(64) NOT NULL,
    human_gate_mode VARCHAR(64) NOT NULL,
    gate_interval INTEGER,
    config_snapshot JSON NOT NULL,
    max_chapters INTEGER,
    max_tokens BIGINT,
    max_estimated_cost DOUBLE PRECISION,
    max_runtime_seconds BIGINT,
    used_tokens BIGINT NOT NULL,
    estimated_cost DOUBLE PRECISION NOT NULL,
    epoch BIGINT NOT NULL,
    version BIGINT NOT NULL,
    consecutive_provider_failures INTEGER NOT NULL,
    consecutive_quality_failures INTEGER NOT NULL,
    last_error_code VARCHAR(128),
    guidance_digest VARCHAR(80),
    active_background_task_id VARCHAR(36),
    final_export_ref TEXT,
    created_at TIMESTAMP WITHOUT TIME ZONE NOT NULL,
    updated_at TIMESTAMP WITHOUT TIME ZONE NOT NULL,
    started_at TIMESTAMP WITHOUT TIME ZONE,
    paused_at TIMESTAMP WITHOUT TIME ZONE,
    completed_at TIMESTAMP WITHOUT TIME ZONE,
    CONSTRAINT pk_novel_autopilot_runs PRIMARY KEY (id),
    CONSTRAINT fk_novel_autopilot_runs_project_id
        FOREIGN KEY (project_id) REFERENCES projects (id) ON DELETE CASCADE,
    CONSTRAINT fk_novel_autopilot_runs_current_chapter_id
        FOREIGN KEY (current_chapter_id) REFERENCES chapters (id) ON DELETE SET NULL
)"#,
        statement_kind: "ddl_create_table",
    },
    RustMigrationSqlStep {
        sql: "CREATE UNIQUE INDEX uq_novel_autopilot_runs_active_scope_key ON novel_autopilot_runs (active_scope_key)",
        statement_kind: "ddl_create_unique_index",
    },
    RustMigrationSqlStep {
        sql: "CREATE INDEX ix_novel_autopilot_runs_project_created_at ON novel_autopilot_runs (project_id, created_at)",
        statement_kind: "ddl_create_index",
    },
    RustMigrationSqlStep {
        sql: r#"CREATE TABLE novel_autopilot_step_runs (
    id VARCHAR(36) NOT NULL,
    run_id VARCHAR(36) NOT NULL,
    step_key VARCHAR(160) NOT NULL,
    step_type VARCHAR(64) NOT NULL,
    phase VARCHAR(64) NOT NULL,
    chapter_id VARCHAR(36),
    chapter_number INTEGER,
    attempt INTEGER NOT NULL,
    run_epoch BIGINT NOT NULL,
    status VARCHAR(32) NOT NULL,
    background_task_id VARCHAR(36),
    input_digest VARCHAR(80) NOT NULL,
    result_digest VARCHAR(80),
    quality_decision VARCHAR(32),
    error_code VARCHAR(128),
    started_at TIMESTAMP WITHOUT TIME ZONE,
    completed_at TIMESTAMP WITHOUT TIME ZONE,
    created_at TIMESTAMP WITHOUT TIME ZONE NOT NULL,
    updated_at TIMESTAMP WITHOUT TIME ZONE NOT NULL,
    CONSTRAINT pk_novel_autopilot_step_runs PRIMARY KEY (id),
    CONSTRAINT fk_novel_autopilot_step_runs_run_id
        FOREIGN KEY (run_id) REFERENCES novel_autopilot_runs (id) ON DELETE CASCADE,
    CONSTRAINT fk_novel_autopilot_step_runs_chapter_id
        FOREIGN KEY (chapter_id) REFERENCES chapters (id) ON DELETE SET NULL,
    CONSTRAINT uq_novel_autopilot_step_runs_run_step_attempt
        UNIQUE (run_id, step_key, attempt)
)"#,
        statement_kind: "ddl_create_table",
    },
    RustMigrationSqlStep {
        sql: "CREATE INDEX ix_novel_autopilot_step_runs_run_status_created_at ON novel_autopilot_step_runs (run_id, status, created_at)",
        statement_kind: "ddl_create_index",
    },
];

const DURABLE_NOVEL_AUTOPILOT_DOWNGRADE_STEPS: &[RustMigrationSqlStep] = &[
    RustMigrationSqlStep {
        sql: "DROP TABLE novel_autopilot_step_runs",
        statement_kind: "ddl_drop_table",
    },
    RustMigrationSqlStep {
        sql: "DROP TABLE novel_autopilot_runs",
        statement_kind: "ddl_drop_table",
    },
];

const PLOT_ANALYSIS_CONTENT_DIGEST_UPGRADE_STEPS: &[RustMigrationSqlStep] =
    &[RustMigrationSqlStep {
        sql: "ALTER TABLE plot_analysis ADD COLUMN source_content_digest VARCHAR(80)",
        statement_kind: "ddl_add_column",
    }];

const PLOT_ANALYSIS_CONTENT_DIGEST_DOWNGRADE_STEPS: &[RustMigrationSqlStep] =
    &[RustMigrationSqlStep {
        sql: "ALTER TABLE plot_analysis DROP COLUMN source_content_digest",
        statement_kind: "ddl_drop_column",
    }];

const AUTOPILOT_USER_ID_CAPACITY_UPGRADE_STEPS: &[RustMigrationSqlStep] = &[RustMigrationSqlStep {
    sql: "ALTER TABLE novel_autopilot_runs ALTER COLUMN user_id TYPE VARCHAR(100)",
    statement_kind: "ddl_alter_column_type",
}];

const AUTOPILOT_USER_ID_CAPACITY_DOWNGRADE_STEPS: &[RustMigrationSqlStep] =
    &[RustMigrationSqlStep {
        sql: "ALTER TABLE novel_autopilot_runs ALTER COLUMN user_id TYPE VARCHAR(36)",
        statement_kind: "ddl_alter_column_type",
    }];

const AUTOPILOT_AUDIT_ACTOR_ID_CAPACITY_UPGRADE_STEPS: &[RustMigrationSqlStep] =
    &[RustMigrationSqlStep {
        sql: "ALTER TABLE autopilot_invocation_audits ALTER COLUMN actor_user_id TYPE VARCHAR(100)",
        statement_kind: "ddl_alter_column_type",
    }];

const AUTOPILOT_AUDIT_ACTOR_ID_CAPACITY_DOWNGRADE_STEPS: &[RustMigrationSqlStep] =
    &[RustMigrationSqlStep {
        sql: "ALTER TABLE autopilot_invocation_audits ALTER COLUMN actor_user_id TYPE VARCHAR(36)",
        statement_kind: "ddl_alter_column_type",
    }];

const AUTOPILOT_RETRY_BACKOFF_UPGRADE_STEPS: &[RustMigrationSqlStep] = &[
    RustMigrationSqlStep {
        sql: "ALTER TABLE novel_autopilot_runs ADD COLUMN next_attempt_at TIMESTAMP WITHOUT TIME ZONE",
        statement_kind: "ddl_add_column",
    },
    RustMigrationSqlStep {
        sql: "CREATE INDEX ix_novel_autopilot_runs_status_next_attempt_at ON novel_autopilot_runs (status, next_attempt_at)",
        statement_kind: "ddl_create_index",
    },
];

const AUTOPILOT_RETRY_BACKOFF_DOWNGRADE_STEPS: &[RustMigrationSqlStep] = &[
    RustMigrationSqlStep {
        sql: "DROP INDEX ix_novel_autopilot_runs_status_next_attempt_at",
        statement_kind: "ddl_drop_index",
    },
    RustMigrationSqlStep {
        sql: "ALTER TABLE novel_autopilot_runs DROP COLUMN next_attempt_at",
        statement_kind: "ddl_drop_column",
    },
];

const RUST_EXECUTABLE_POSTGRES_REVISIONS: &[RustMigrationExecutableRevision] = &[
    RustMigrationExecutableRevision {
        revision: "ee0a189f1532",
        filename: "20251226_1008_ee0a189f1532_初始数据库结构.py",
        execution_scope: RUST_EXECUTABLE_MIGRATION_COVERAGE,
        upgrade_steps: INITIAL_SCHEMA_UPGRADE_STEPS,
        downgrade_steps: INITIAL_SCHEMA_DOWNGRADE_STEPS,
    },
    RustMigrationExecutableRevision {
        revision: "e411428f00c0",
        filename: "20251226_1102_e411428f00c0_初始化预置数据.py",
        execution_scope: RUST_EXECUTABLE_MIGRATION_COVERAGE,
        upgrade_steps: INITIAL_SEED_DATA_UPGRADE_STEPS,
        downgrade_steps: INITIAL_SEED_DATA_DOWNGRADE_STEPS,
    },
    RustMigrationExecutableRevision {
        revision: "a7e4408e1d5b",
        filename: "20251227_1541_a7e4408e1d5b_添加system_prompt字段到settings表.py",
        execution_scope: RUST_EXECUTABLE_MIGRATION_COVERAGE,
        upgrade_steps: SETTINGS_SYSTEM_PROMPT_UPGRADE_STEPS,
        downgrade_steps: SETTINGS_SYSTEM_PROMPT_DOWNGRADE_STEPS,
    },
    RustMigrationExecutableRevision {
        revision: "6a73f37e9adb",
        filename: "20260119_1729_6a73f37e9adb_添加伏笔管理表.py",
        execution_scope: RUST_EXECUTABLE_MIGRATION_COVERAGE,
        upgrade_steps: FORESHADOWS_TABLE_UPGRADE_STEPS,
        downgrade_steps: FORESHADOWS_TABLE_DOWNGRADE_STEPS,
    },
    RustMigrationExecutableRevision {
        revision: "421237957b27",
        filename: "20260127_1404_421237957b27_添加提示词工坊相关表结构.py",
        execution_scope: RUST_EXECUTABLE_MIGRATION_COVERAGE,
        upgrade_steps: PROMPT_WORKSHOP_UPGRADE_STEPS,
        downgrade_steps: PROMPT_WORKSHOP_DOWNGRADE_STEPS,
    },
    RustMigrationExecutableRevision {
        revision: "d4d253e3f4c6",
        filename: "20260212_1244_d4d253e3f4c6_添加角色心理状态追踪字段.py",
        execution_scope: RUST_EXECUTABLE_MIGRATION_COVERAGE,
        upgrade_steps: CHARACTER_STATE_TRACKING_UPGRADE_STEPS,
        downgrade_steps: CHARACTER_STATE_TRACKING_DOWNGRADE_STEPS,
    },
    RustMigrationExecutableRevision {
        revision: "20260222_api_compat",
        filename: "20260222_add_api_compatibility_fields.py",
        execution_scope: RUST_EXECUTABLE_MIGRATION_COVERAGE,
        upgrade_steps: SETTINGS_API_COMPAT_UPGRADE_STEPS,
        downgrade_steps: SETTINGS_API_COMPAT_DOWNGRADE_STEPS,
    },
    RustMigrationExecutableRevision {
        revision: "b3f6c1a9d2e4",
        filename: "20260301_1510_b3f6c1a9d2e4_新增低ai生活化写作风格预设.py",
        execution_scope: RUST_EXECUTABLE_MIGRATION_COVERAGE,
        upgrade_steps: WRITING_STYLE_LOW_AI_LIFE_INSERT_UPGRADE_STEPS,
        downgrade_steps: WRITING_STYLE_LOW_AI_LIFE_INSERT_DOWNGRADE_STEPS,
    },
    RustMigrationExecutableRevision {
        revision: "c4e9d1b7a2f0",
        filename: "20260301_1700_c4e9d1b7a2f0_更新低ai生活化风格文案v2.py",
        execution_scope: RUST_EXECUTABLE_MIGRATION_COVERAGE,
        upgrade_steps: WRITING_STYLE_LOW_AI_LIFE_V2_UPGRADE_STEPS,
        downgrade_steps: WRITING_STYLE_LOW_AI_LIFE_V2_DOWNGRADE_STEPS,
    },
    RustMigrationExecutableRevision {
        revision: "e8b4d6c1f2a7",
        filename: "20260301_1730_e8b4d6c1f2a7_新增低ai连载感写作风格预设.py",
        execution_scope: RUST_EXECUTABLE_MIGRATION_COVERAGE,
        upgrade_steps: WRITING_STYLE_LOW_AI_SERIAL_UPGRADE_STEPS,
        downgrade_steps: WRITING_STYLE_LOW_AI_SERIAL_DOWNGRADE_STEPS,
    },
    RustMigrationExecutableRevision {
        revision: "20260322_proj_gen_defaults",
        filename: "20260322_1200_project_generation_defaults.py",
        execution_scope: RUST_EXECUTABLE_MIGRATION_COVERAGE,
        upgrade_steps: PROJECT_GENERATION_DEFAULTS_UPGRADE_STEPS,
        downgrade_steps: PROJECT_GENERATION_DEFAULTS_DOWNGRADE_STEPS,
    },
    RustMigrationExecutableRevision {
        revision: "20260323_proj_quality_prefs",
        filename: "20260323_1030_project_quality_preferences.py",
        execution_scope: RUST_EXECUTABLE_MIGRATION_COVERAGE,
        upgrade_steps: PROJECT_QUALITY_PREFS_UPGRADE_STEPS,
        downgrade_steps: PROJECT_QUALITY_PREFS_DOWNGRADE_STEPS,
    },
    RustMigrationExecutableRevision {
        revision: "20260325_batch_runtime_store",
        filename: "20260325_0900_batch_runtime_store.py",
        execution_scope: RUST_EXECUTABLE_MIGRATION_COVERAGE,
        upgrade_steps: BATCH_RUNTIME_STORE_UPGRADE_STEPS,
        downgrade_steps: BATCH_RUNTIME_STORE_DOWNGRADE_STEPS,
    },
    RustMigrationExecutableRevision {
        revision: "20260325_batch_workflow_state",
        filename: "20260325_2210_batch_workflow_runtime_state.py",
        execution_scope: RUST_EXECUTABLE_MIGRATION_COVERAGE,
        upgrade_steps: BATCH_WORKFLOW_STATE_UPGRADE_STEPS,
        downgrade_steps: BATCH_WORKFLOW_STATE_DOWNGRADE_STEPS,
    },
    RustMigrationExecutableRevision {
        revision: "20260517_analysis_task_hardening",
        filename: "20260517_1200_analysis_task_progress_hardening.py",
        execution_scope: RUST_EXECUTABLE_MIGRATION_COVERAGE,
        upgrade_steps: ANALYSIS_TASK_HARDENING_UPGRADE_STEPS,
        downgrade_steps: ANALYSIS_TASK_HARDENING_DOWNGRADE_STEPS,
    },
    RustMigrationExecutableRevision {
        revision: "20260517_batch_task_defaults",
        filename: "20260517_1300_batch_generation_task_defaults_hardening.py",
        execution_scope: RUST_EXECUTABLE_MIGRATION_COVERAGE,
        upgrade_steps: BATCH_TASK_DEFAULTS_UPGRADE_STEPS,
        downgrade_steps: BATCH_TASK_DEFAULTS_DOWNGRADE_STEPS,
    },
    RustMigrationExecutableRevision {
        revision: "20260517_regeneration_task_defaults",
        filename: "20260517_1400_regeneration_task_defaults_hardening.py",
        execution_scope: RUST_EXECUTABLE_MIGRATION_COVERAGE,
        upgrade_steps: REGENERATION_TASK_DEFAULTS_UPGRADE_STEPS,
        downgrade_steps: REGENERATION_TASK_DEFAULTS_DOWNGRADE_STEPS,
    },
    RustMigrationExecutableRevision {
        revision: "20260517_settings_core_defaults",
        filename: "20260517_1500_settings_core_defaults_hardening.py",
        execution_scope: RUST_EXECUTABLE_MIGRATION_COVERAGE,
        upgrade_steps: SETTINGS_CORE_DEFAULTS_UPGRADE_STEPS,
        downgrade_steps: SETTINGS_CORE_DEFAULTS_DOWNGRADE_STEPS,
    },
    RustMigrationExecutableRevision {
        revision: "20260517_project_core_defaults",
        filename: "20260517_1600_project_core_defaults_hardening.py",
        execution_scope: RUST_EXECUTABLE_MIGRATION_COVERAGE,
        upgrade_steps: PROJECT_CORE_DEFAULTS_UPGRADE_STEPS,
        downgrade_steps: PROJECT_CORE_DEFAULTS_DOWNGRADE_STEPS,
    },
    RustMigrationExecutableRevision {
        revision: "20260712_password_hash_phc_text",
        filename: "20260712_1200_password_hash_phc_text.py",
        execution_scope: RUST_EXECUTABLE_MIGRATION_COVERAGE,
        upgrade_steps: PASSWORD_HASH_PHC_TEXT_UPGRADE_STEPS,
        downgrade_steps: PASSWORD_HASH_PHC_TEXT_DOWNGRADE_STEPS,
    },
    RustMigrationExecutableRevision {
        revision: "20260716_autopilot_invocation_audit",
        filename: "20260716_2200_autopilot_invocation_audit.py",
        execution_scope: RUST_EXECUTABLE_MIGRATION_COVERAGE,
        upgrade_steps: AUTOPILOT_INVOCATION_AUDIT_UPGRADE_STEPS,
        downgrade_steps: AUTOPILOT_INVOCATION_AUDIT_DOWNGRADE_STEPS,
    },
    RustMigrationExecutableRevision {
        revision: "20260719_durable_novel_autopilot",
        filename: "20260719_1200_durable_novel_autopilot.py",
        execution_scope: RUST_EXECUTABLE_MIGRATION_COVERAGE,
        upgrade_steps: DURABLE_NOVEL_AUTOPILOT_UPGRADE_STEPS,
        downgrade_steps: DURABLE_NOVEL_AUTOPILOT_DOWNGRADE_STEPS,
    },
    RustMigrationExecutableRevision {
        revision: "20260719_analysis_content_digest",
        filename: "20260719_1600_plot_analysis_source_content_digest.py",
        execution_scope: RUST_EXECUTABLE_MIGRATION_COVERAGE,
        upgrade_steps: PLOT_ANALYSIS_CONTENT_DIGEST_UPGRADE_STEPS,
        downgrade_steps: PLOT_ANALYSIS_CONTENT_DIGEST_DOWNGRADE_STEPS,
    },
    RustMigrationExecutableRevision {
        revision: "20260719_autopilot_user_id_capacity",
        filename: "20260719_1700_novel_autopilot_user_id_capacity.py",
        execution_scope: RUST_EXECUTABLE_MIGRATION_COVERAGE,
        upgrade_steps: AUTOPILOT_USER_ID_CAPACITY_UPGRADE_STEPS,
        downgrade_steps: AUTOPILOT_USER_ID_CAPACITY_DOWNGRADE_STEPS,
    },
    RustMigrationExecutableRevision {
        revision: "20260720_audit_actor_id_capacity",
        filename: "20260720_0900_autopilot_audit_actor_user_id_capacity.py",
        execution_scope: RUST_EXECUTABLE_MIGRATION_COVERAGE,
        upgrade_steps: AUTOPILOT_AUDIT_ACTOR_ID_CAPACITY_UPGRADE_STEPS,
        downgrade_steps: AUTOPILOT_AUDIT_ACTOR_ID_CAPACITY_DOWNGRADE_STEPS,
    },
    RustMigrationExecutableRevision {
        revision: POSTGRES_ALEMBIC_HEAD,
        filename: "20260807_1200_novel_autopilot_retry_backoff.py",
        execution_scope: RUST_EXECUTABLE_MIGRATION_COVERAGE,
        upgrade_steps: AUTOPILOT_RETRY_BACKOFF_UPGRADE_STEPS,
        downgrade_steps: AUTOPILOT_RETRY_BACKOFF_DOWNGRADE_STEPS,
    },
];

pub(crate) fn rust_executable_postgres_revisions() -> &'static [RustMigrationExecutableRevision] {
    RUST_EXECUTABLE_POSTGRES_REVISIONS
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LiveAlembicHeadCheck {
    pub(crate) status: &'static str,
    pub(crate) expected_head: &'static str,
    pub(crate) actual_head: Option<String>,
    pub(crate) matches_catalog_head: bool,
    pub(crate) error: Option<String>,
}

impl LiveAlembicHeadCheck {
    pub(crate) fn not_checked(reason: impl Into<String>) -> Self {
        Self {
            status: "not_checked",
            expected_head: POSTGRES_ALEMBIC_HEAD,
            actual_head: None,
            matches_catalog_head: false,
            error: Some(reason.into()),
        }
    }

    fn missing_table(error: impl Into<String>) -> Self {
        Self {
            status: "table_missing",
            expected_head: POSTGRES_ALEMBIC_HEAD,
            actual_head: None,
            matches_catalog_head: false,
            error: Some(error.into()),
        }
    }

    fn empty_table() -> Self {
        Self {
            status: "empty_table",
            expected_head: POSTGRES_ALEMBIC_HEAD,
            actual_head: None,
            matches_catalog_head: false,
            error: None,
        }
    }

    fn from_live_head(actual_head: String) -> Self {
        let matches_catalog_head = actual_head == POSTGRES_ALEMBIC_HEAD;
        Self {
            status: if matches_catalog_head {
                "head_matches"
            } else {
                "head_mismatch"
            },
            expected_head: POSTGRES_ALEMBIC_HEAD,
            actual_head: Some(actual_head),
            matches_catalog_head,
            error: None,
        }
    }

    fn query_error(error: impl Into<String>) -> Self {
        Self {
            status: "query_error",
            expected_head: POSTGRES_ALEMBIC_HEAD,
            actual_head: None,
            matches_catalog_head: false,
            error: Some(error.into()),
        }
    }

    pub(crate) fn to_json(&self) -> Value {
        json!({
            "owner": RUST_METADATA_OWNER,
            "status": self.status,
            "expected_head": self.expected_head,
            "actual_head": self.actual_head,
            "matches_catalog_head": self.matches_catalog_head,
            "read_only": true,
            "query": "SELECT version_num FROM alembic_version LIMIT 1",
            "error": self.error,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PasswordHashStorageCompatibilityCheck {
    pub(crate) status: &'static str,
    pub(crate) data_type: Option<String>,
    pub(crate) udt_name: Option<String>,
    pub(crate) character_maximum_length: Option<i32>,
    pub(crate) supports_canonical_argon2: Option<bool>,
    pub(crate) matches_target_storage_contract: Option<bool>,
    pub(crate) required_for_readiness: bool,
    pub(crate) allows_readiness: bool,
    pub(crate) error: Option<String>,
}

impl PasswordHashStorageCompatibilityCheck {
    pub(crate) fn not_checked_database_unavailable() -> Self {
        Self {
            status: "not_checked_database_unavailable",
            data_type: None,
            udt_name: None,
            character_maximum_length: None,
            supports_canonical_argon2: None,
            matches_target_storage_contract: None,
            required_for_readiness: true,
            allows_readiness: false,
            error: Some("database unavailable".to_string()),
        }
    }

    fn not_applicable_non_postgres() -> Self {
        Self {
            status: "not_applicable_non_postgres",
            data_type: None,
            udt_name: None,
            character_maximum_length: None,
            supports_canonical_argon2: None,
            matches_target_storage_contract: None,
            required_for_readiness: false,
            allows_readiness: true,
            error: None,
        }
    }

    fn column_missing() -> Self {
        Self {
            status: "blocked_column_missing",
            data_type: None,
            udt_name: None,
            character_maximum_length: None,
            supports_canonical_argon2: Some(false),
            matches_target_storage_contract: Some(false),
            required_for_readiness: true,
            allows_readiness: false,
            error: None,
        }
    }

    fn query_error(error: impl Into<String>) -> Self {
        Self {
            status: "blocked_query_error",
            data_type: None,
            udt_name: None,
            character_maximum_length: None,
            supports_canonical_argon2: None,
            matches_target_storage_contract: None,
            required_for_readiness: true,
            allows_readiness: false,
            error: Some(error.into()),
        }
    }

    pub(crate) fn from_column_metadata(
        data_type: String,
        udt_name: String,
        character_maximum_length: Option<i32>,
    ) -> Self {
        let normalized_data_type = data_type.trim().to_ascii_lowercase();
        let normalized_udt_name = udt_name.trim().to_ascii_lowercase();
        let is_text = normalized_data_type == "text" || normalized_udt_name == "text";
        let is_varchar = normalized_data_type == "character varying"
            || normalized_data_type == "varchar"
            || normalized_udt_name == "varchar";
        let required_length = CANONICAL_ARGON2_VERIFIER_STORAGE_LENGTH as i32;

        let (status, supports_canonical_argon2, matches_target_storage_contract) = if is_text {
            ("compatible_unbounded_text", true, true)
        } else if is_varchar && character_maximum_length.is_none() {
            ("compatible_unbounded_character_varying", true, false)
        } else if is_varchar
            && character_maximum_length.is_some_and(|length| length >= required_length)
        {
            ("compatible_bounded_capacity", true, false)
        } else if is_varchar {
            ("blocked_capacity_too_small", false, false)
        } else {
            ("blocked_unsupported_type", false, false)
        };

        Self {
            status,
            data_type: Some(data_type),
            udt_name: Some(udt_name),
            character_maximum_length,
            supports_canonical_argon2: Some(supports_canonical_argon2),
            matches_target_storage_contract: Some(matches_target_storage_contract),
            required_for_readiness: true,
            allows_readiness: supports_canonical_argon2,
            error: None,
        }
    }

    pub(crate) fn to_json(&self) -> Value {
        json!({
            "owner": RUST_METADATA_OWNER,
            "status": self.status,
            "table_name": "user_passwords",
            "column_name": "password_hash",
            "data_type": self.data_type,
            "udt_name": self.udt_name,
            "character_maximum_length": self.character_maximum_length,
            "required_minimum_length": CANONICAL_ARGON2_VERIFIER_STORAGE_LENGTH,
            "target_storage_contract": "unbounded_text",
            "supports_canonical_argon2": self.supports_canonical_argon2,
            "matches_target_storage_contract": self.matches_target_storage_contract,
            "required_for_readiness": self.required_for_readiness,
            "allows_readiness": self.allows_readiness,
            "read_only": true,
            "query": PASSWORD_HASH_STORAGE_METADATA_QUERY,
            "error": self.error,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RustMigrationExecutorPreflight {
    pub(crate) status: &'static str,
    pub(crate) execution_mode: &'static str,
    pub(crate) catalog_head: &'static str,
    pub(crate) live_head: Option<String>,
    pub(crate) no_op_executor_smoke_ready: bool,
    pub(crate) can_replace_python_migrator: bool,
    pub(crate) blockers: Vec<String>,
    pub(crate) next_action: &'static str,
}

impl RustMigrationExecutorPreflight {
    pub(crate) fn from_live_head_check(live_head_check: &LiveAlembicHeadCheck) -> Self {
        match live_head_check.status {
            "head_matches" => Self {
                status: "preflight_ready_for_noop_executor_smoke",
                execution_mode: "read_only_preflight_no_ddl",
                catalog_head: POSTGRES_ALEMBIC_HEAD,
                live_head: live_head_check.actual_head.clone(),
                no_op_executor_smoke_ready: true,
                can_replace_python_migrator: true,
                blockers: Vec::new(),
                next_action:
                    "delete or archive frozen Python migrator source-map files after Rust db-migrator deploy smoke",
            },
            "head_mismatch" => Self::blocked(
                "blocked_live_head_mismatch",
                live_head_check.actual_head.clone(),
                "live database Alembic head does not match Rust revision catalog head",
                "run Rust migration-executor to the catalog head before deleting Python source-map files",
            ),
            "empty_table" => Self::blocked(
                "blocked_empty_alembic_version_table",
                None,
                "alembic_version exists but has no revision row",
                "repair migration metadata with Rust migration-executor before deleting Python source-map files",
            ),
            "table_missing" => Self::blocked(
                "blocked_missing_alembic_version_table",
                None,
                "alembic_version table is missing",
                "bootstrap schema through Rust migration-executor before deleting Python source-map files",
            ),
            "query_error" => Self::blocked(
                "blocked_live_head_query_error",
                None,
                live_head_check
                    .error
                    .as_deref()
                    .unwrap_or("live Alembic head query failed"),
                "fix live database migration metadata visibility before Rust executor cutover",
            ),
            _ => Self::blocked(
                "blocked_live_head_not_checked",
                None,
                live_head_check
                    .error
                    .as_deref()
                    .unwrap_or("live Alembic head was not checked"),
                "connect to the database and verify live Alembic head before Rust executor cutover",
            ),
        }
    }

    fn blocked(
        status: &'static str,
        live_head: Option<String>,
        blocker: impl Into<String>,
        next_action: &'static str,
    ) -> Self {
        Self {
            status,
            execution_mode: "read_only_preflight_no_ddl",
            catalog_head: POSTGRES_ALEMBIC_HEAD,
            live_head,
            no_op_executor_smoke_ready: false,
            can_replace_python_migrator: false,
            blockers: vec![blocker.into()],
            next_action,
        }
    }

    pub(crate) fn to_json(&self) -> Value {
        json!({
            "owner": RUST_METADATA_OWNER,
            "status": self.status,
            "execution_mode": self.execution_mode,
            "catalog_head": self.catalog_head,
            "live_head": self.live_head,
            "no_op_executor_smoke_ready": self.no_op_executor_smoke_ready,
            "can_replace_python_migrator": self.can_replace_python_migrator,
            "ddl_execution_enabled": false,
            "blockers": self.blockers,
            "next_action": self.next_action,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RustMigrationReplayPlan {
    pub(crate) ok: bool,
    pub(crate) status: &'static str,
    pub(crate) current_revision: Option<String>,
    pub(crate) target_revision: &'static str,
    pub(crate) pending_revisions: Vec<&'static str>,
    pub(crate) pending_files: Vec<&'static str>,
    pub(crate) rust_executable_pending_revisions: Vec<&'static str>,
    pub(crate) rust_executable_pending_sql_step_count: usize,
    pub(crate) pending_revisions_all_have_rust_steps: bool,
    pub(crate) already_at_head: bool,
    pub(crate) ddl_replay_ready: bool,
    pub(crate) ddl_executed: bool,
    pub(crate) can_replace_python_migrator: bool,
    pub(crate) blockers: Vec<String>,
    pub(crate) next_action: &'static str,
}

impl RustMigrationReplayPlan {
    pub(crate) fn from_live_head_check(live_head_check: &LiveAlembicHeadCheck) -> Self {
        match live_head_check.status {
            "head_matches" => Self {
                ok: true,
                status: "already_at_catalog_head",
                current_revision: live_head_check.actual_head.clone(),
                target_revision: POSTGRES_ALEMBIC_HEAD,
                pending_revisions: Vec::new(),
                pending_files: Vec::new(),
                rust_executable_pending_revisions: Vec::new(),
                rust_executable_pending_sql_step_count: 0,
                pending_revisions_all_have_rust_steps: true,
                already_at_head: true,
                ddl_replay_ready: false,
                ddl_executed: false,
                can_replace_python_migrator: true,
                blockers: Vec::new(),
                next_action:
                    "delete or archive frozen Python migrator source-map files after Rust db-migrator deploy smoke",
            },
            "head_mismatch" => {
                let current_revision = live_head_check.actual_head.clone();
                let Some(current_revision_ref) = current_revision.as_deref() else {
                    return Self::blocked(
                        "blocked_missing_live_revision",
                        None,
                        "live database head mismatch did not include a revision id",
                        "repair alembic_version before Rust replay planning",
                    );
                };

                match pending_catalog_entries_after(current_revision_ref) {
                    Some(pending_entries) => {
                        let pending_revisions = pending_entries
                            .iter()
                            .map(|entry| entry.revision)
                            .collect::<Vec<_>>();
                        let pending_files =
                            pending_entries.iter().map(|entry| entry.filename).collect();
                        let rust_executable_pending_revisions = pending_revisions
                            .iter()
                            .copied()
                            .filter(|revision| rust_executable_revision(*revision).is_some())
                            .collect::<Vec<_>>();
                        let pending_revisions_all_have_rust_steps =
                            rust_executable_pending_revisions.len() == pending_revisions.len();
                        let rust_executable_pending_sql_step_count =
                            rust_executable_pending_revisions
                                .iter()
                                .filter_map(|revision| rust_executable_revision(revision))
                                .map(|revision| revision.upgrade_steps.len())
                                .sum();
                        let mut blockers = Vec::new();
                        if !pending_revisions_all_have_rust_steps {
                            blockers.push(
                                "Not every pending revision has Rust SQL step parity yet"
                                    .to_string(),
                            );
                        }

                        Self {
                            ok: true,
                            status: if pending_revisions_all_have_rust_steps {
                                "pending_catalog_revisions_have_rust_steps"
                            } else {
                                "pending_catalog_revisions_detected"
                            },
                            current_revision,
                            target_revision: POSTGRES_ALEMBIC_HEAD,
                            pending_revisions,
                            pending_files,
                            rust_executable_pending_revisions,
                            rust_executable_pending_sql_step_count,
                            pending_revisions_all_have_rust_steps,
                            already_at_head: false,
                            ddl_replay_ready: pending_revisions_all_have_rust_steps,
                            ddl_executed: false,
                            can_replace_python_migrator: pending_revisions_all_have_rust_steps,
                            blockers,
                            next_action:
                                "run Rust migration-executor under single-flight, then delete or archive frozen Python migrator source-map files",
                        }
                    }
                    None => {
                        let blocker = format!(
                            "live database head is not present in the Rust PostgreSQL revision catalog: {current_revision_ref}"
                        );
                        Self::blocked(
                            "blocked_unknown_live_revision",
                            current_revision,
                            blocker,
                            "repair migration history or add the missing revision to the Rust catalog before replay",
                        )
                    }
                }
            }
            "empty_table" => Self::blocked(
                "blocked_empty_alembic_version_table",
                None,
                "alembic_version exists but has no revision row",
                "bootstrap schema with the Rust db-migrator before Rust replay",
            ),
            "table_missing" => {
                let pending_entries = postgres_revision_catalog();
                let pending_revisions = pending_entries
                    .iter()
                    .map(|entry| entry.revision)
                    .collect::<Vec<_>>();
                let pending_files = pending_entries.iter().map(|entry| entry.filename).collect();
                let rust_executable_pending_revisions = pending_revisions
                    .iter()
                    .copied()
                    .filter(|revision| rust_executable_revision(*revision).is_some())
                    .collect::<Vec<_>>();
                let pending_revisions_all_have_rust_steps =
                    rust_executable_pending_revisions.len() == pending_revisions.len();
                let rust_executable_pending_sql_step_count = rust_executable_pending_revisions
                    .iter()
                    .filter_map(|revision| rust_executable_revision(revision))
                    .map(|revision| revision.upgrade_steps.len())
                    .sum();
                let mut blockers = Vec::new();
                if !pending_revisions_all_have_rust_steps {
                    blockers.push("Not every catalog revision has Rust SQL step parity yet".to_string());
                }

                Self {
                    ok: true,
                    status: if pending_revisions_all_have_rust_steps {
                        "initial_schema_bootstrap_has_rust_steps"
                    } else {
                        "initial_schema_bootstrap_detected"
                    },
                    current_revision: None,
                    target_revision: POSTGRES_ALEMBIC_HEAD,
                    pending_revisions,
                    pending_files,
                    rust_executable_pending_revisions,
                    rust_executable_pending_sql_step_count,
                    pending_revisions_all_have_rust_steps,
                    already_at_head: false,
                    ddl_replay_ready: pending_revisions_all_have_rust_steps,
                    ddl_executed: false,
                    can_replace_python_migrator: pending_revisions_all_have_rust_steps,
                    blockers,
                    next_action:
                        "run Rust migration-executor under single-flight, then delete or archive frozen Python migrator source-map files",
                }
            }
            "query_error" => Self::blocked(
                "blocked_live_head_query_error",
                None,
                live_head_check
                    .error
                    .as_deref()
                    .unwrap_or("live Alembic head query failed"),
                "fix live migration metadata visibility before Rust replay",
            ),
            _ => Self::blocked(
                "blocked_live_head_not_checked",
                None,
                live_head_check
                    .error
                    .as_deref()
                    .unwrap_or("live Alembic head was not checked"),
                "check live Alembic head before Rust replay planning",
            ),
        }
    }

    fn blocked(
        status: &'static str,
        current_revision: Option<String>,
        blocker: impl Into<String>,
        next_action: &'static str,
    ) -> Self {
        Self {
            ok: false,
            status,
            current_revision,
            target_revision: POSTGRES_ALEMBIC_HEAD,
            pending_revisions: Vec::new(),
            pending_files: Vec::new(),
            rust_executable_pending_revisions: Vec::new(),
            rust_executable_pending_sql_step_count: 0,
            pending_revisions_all_have_rust_steps: false,
            already_at_head: false,
            ddl_replay_ready: false,
            ddl_executed: false,
            can_replace_python_migrator: false,
            blockers: vec![blocker.into()],
            next_action,
        }
    }

    pub(crate) fn to_json(&self) -> Value {
        json!({
            "owner": RUST_METADATA_OWNER,
            "ok": self.ok,
            "status": self.status,
            "current_revision": self.current_revision,
            "target_revision": self.target_revision,
            "pending_revision_count": self.pending_revisions.len(),
            "pending_revisions": self.pending_revisions,
            "pending_files": self.pending_files,
            "rust_executable_pending_revisions": self.rust_executable_pending_revisions,
            "rust_executable_pending_sql_step_count": self.rust_executable_pending_sql_step_count,
            "pending_revisions_all_have_rust_steps": self.pending_revisions_all_have_rust_steps,
            "already_at_head": self.already_at_head,
            "ddl_replay_ready": self.ddl_replay_ready,
            "ddl_executed": self.ddl_executed,
            "can_replace_python_migrator": self.can_replace_python_migrator,
            "blockers": self.blockers,
            "next_action": self.next_action,
        })
    }
}

fn pending_catalog_entries_after(
    revision: &str,
) -> Option<Vec<&'static MigrationRevisionCatalogEntry>> {
    let catalog = postgres_revision_catalog();
    let position = catalog
        .iter()
        .position(|entry| entry.revision == revision)?;
    Some(catalog.iter().skip(position + 1).collect())
}

fn rust_executable_revision(revision: &str) -> Option<&'static RustMigrationExecutableRevision> {
    rust_executable_postgres_revisions()
        .iter()
        .find(|entry| entry.revision == revision)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RustMigrationNoopExecutorSmokeResult {
    pub(crate) ok: bool,
    pub(crate) status: &'static str,
    pub(crate) execution_mode: &'static str,
    pub(crate) gate_enabled: bool,
    pub(crate) ddl_executed: bool,
    pub(crate) live_head: Option<String>,
    pub(crate) expected_head: &'static str,
    pub(crate) rollback_boundary: &'static str,
    pub(crate) blockers: Vec<String>,
}

impl RustMigrationNoopExecutorSmokeResult {
    fn disabled() -> Self {
        Self {
            ok: false,
            status: "disabled_by_config",
            execution_mode: "read_only_noop_executor_smoke",
            gate_enabled: false,
            ddl_executed: false,
            live_head: None,
            expected_head: POSTGRES_ALEMBIC_HEAD,
            rollback_boundary: "python_db_migrator_alembic",
            blockers: vec!["RUST_MIGRATION_NOOP_EXECUTOR_SMOKE_ENABLED is false".to_string()],
        }
    }

    fn blocked(preflight: RustMigrationExecutorPreflight) -> Self {
        Self {
            ok: false,
            status: "blocked_by_preflight",
            execution_mode: "read_only_noop_executor_smoke",
            gate_enabled: true,
            ddl_executed: false,
            live_head: preflight.live_head,
            expected_head: POSTGRES_ALEMBIC_HEAD,
            rollback_boundary: "python_db_migrator_alembic",
            blockers: preflight.blockers,
        }
    }

    fn ready(preflight: RustMigrationExecutorPreflight) -> Self {
        Self {
            ok: true,
            status: "noop_executor_smoke_passed",
            execution_mode: "read_only_noop_executor_smoke",
            gate_enabled: true,
            ddl_executed: false,
            live_head: preflight.live_head,
            expected_head: POSTGRES_ALEMBIC_HEAD,
            rollback_boundary: "python_db_migrator_alembic",
            blockers: preflight.blockers,
        }
    }

    pub(crate) fn to_json(&self) -> Value {
        json!({
            "owner": RUST_METADATA_OWNER,
            "ok": self.ok,
            "status": self.status,
            "execution_mode": self.execution_mode,
            "gate_enabled": self.gate_enabled,
            "ddl_executed": self.ddl_executed,
            "live_head": self.live_head,
            "expected_head": self.expected_head,
            "rollback_boundary": self.rollback_boundary,
            "blockers": self.blockers,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RustMigrationTailHardeningReplayResult {
    pub(crate) ok: bool,
    pub(crate) status: &'static str,
    pub(crate) gate_enabled: bool,
    pub(crate) ddl_executed: bool,
    pub(crate) executed_revisions: Vec<&'static str>,
    pub(crate) executed_sql_step_count: usize,
    pub(crate) final_revision: Option<String>,
    pub(crate) blockers: Vec<String>,
}

impl RustMigrationTailHardeningReplayResult {
    fn disabled(plan: &RustMigrationReplayPlan) -> Self {
        Self {
            ok: false,
            status: "disabled_by_config",
            gate_enabled: false,
            ddl_executed: false,
            executed_revisions: Vec::new(),
            executed_sql_step_count: 0,
            final_revision: plan.current_revision.clone(),
            blockers: vec![format!(
                "{RUST_MIGRATION_TAIL_HARDENING_REPLAY_ENABLED_ENV} is false"
            )],
        }
    }

    fn already_at_head(plan: &RustMigrationReplayPlan, gate_enabled: bool) -> Self {
        Self {
            ok: true,
            status: "already_at_catalog_head",
            gate_enabled,
            ddl_executed: false,
            executed_revisions: Vec::new(),
            executed_sql_step_count: 0,
            final_revision: plan.current_revision.clone(),
            blockers: plan.blockers.clone(),
        }
    }

    fn blocked(
        status: &'static str,
        gate_enabled: bool,
        plan: &RustMigrationReplayPlan,
        blocker: impl Into<String>,
    ) -> Self {
        Self {
            ok: false,
            status,
            gate_enabled,
            ddl_executed: false,
            executed_revisions: Vec::new(),
            executed_sql_step_count: 0,
            final_revision: plan.current_revision.clone(),
            blockers: vec![blocker.into()],
        }
    }

    fn applied(
        gate_enabled: bool,
        executed_revisions: Vec<&'static str>,
        executed_sql_step_count: usize,
        final_revision: Option<String>,
    ) -> Self {
        Self {
            ok: true,
            status: "tail_hardening_replay_applied",
            gate_enabled,
            ddl_executed: executed_sql_step_count > 0,
            executed_revisions,
            executed_sql_step_count,
            final_revision,
            blockers: Vec::new(),
        }
    }

    pub(crate) fn to_json(&self) -> Value {
        json!({
            "owner": RUST_METADATA_OWNER,
            "ok": self.ok,
            "status": self.status,
            "gate_env": RUST_MIGRATION_TAIL_HARDENING_REPLAY_ENABLED_ENV,
            "gate_enabled": self.gate_enabled,
            "ddl_executed": self.ddl_executed,
            "executed_revisions": self.executed_revisions,
            "executed_sql_step_count": self.executed_sql_step_count,
            "final_revision": self.final_revision,
            "blockers": self.blockers,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MigrationSingleFlightConfig {
    pub(crate) lock_name: String,
    pub(crate) timeout_seconds: u64,
    pub(crate) poll_interval_millis: u64,
    pub(crate) lock_file_path: String,
}

impl MigrationSingleFlightConfig {
    pub(crate) fn from_env() -> Self {
        let timeout_seconds = std::env::var("MIGRATION_LOCK_TIMEOUT_SECONDS")
            .ok()
            .and_then(|value| value.parse::<u64>().ok())
            .unwrap_or(DEFAULT_MIGRATION_LOCK_TIMEOUT_SECONDS);
        let poll_interval_millis = std::env::var("MIGRATION_LOCK_POLL_INTERVAL")
            .ok()
            .and_then(|value| value.parse::<f64>().ok())
            .map(|seconds| (seconds.max(0.05) * 1000.0).round() as u64)
            .unwrap_or((DEFAULT_MIGRATION_LOCK_POLL_INTERVAL_SECONDS * 1000.0) as u64);
        let lock_file_path = std::env::var("MIGRATION_LOCK_FILE")
            .unwrap_or_else(|_| MIGRATION_LOCK_FILE_NAME.to_string());

        Self {
            lock_name: MIGRATION_LOCK_NAME.to_string(),
            timeout_seconds,
            poll_interval_millis,
            lock_file_path,
        }
    }

    fn timeout(&self) -> Duration {
        Duration::from_secs(self.timeout_seconds)
    }

    fn poll_interval(&self) -> Duration {
        Duration::from_millis(self.poll_interval_millis.max(1))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MigrationSingleFlightReport {
    pub(crate) ok: bool,
    pub(crate) status: &'static str,
    pub(crate) lock_mode: &'static str,
    pub(crate) lock_acquired: bool,
    pub(crate) lock_key: i64,
    pub(crate) fallback_reason: Option<String>,
    pub(crate) timeout_seconds: u64,
    pub(crate) poll_interval_millis: u64,
    pub(crate) held_during_live_head_check: bool,
    pub(crate) error: Option<String>,
}

impl MigrationSingleFlightReport {
    fn postgres(lock_key: i64, config: &MigrationSingleFlightConfig) -> Self {
        Self {
            ok: true,
            status: "lock_acquired",
            lock_mode: "postgres_advisory_lock",
            lock_acquired: true,
            lock_key,
            fallback_reason: None,
            timeout_seconds: config.timeout_seconds,
            poll_interval_millis: config.poll_interval_millis,
            held_during_live_head_check: true,
            error: None,
        }
    }

    fn file(
        lock_key: i64,
        config: &MigrationSingleFlightConfig,
        fallback_reason: impl Into<String>,
    ) -> Self {
        Self {
            ok: true,
            status: "lock_acquired",
            lock_mode: "file_lock",
            lock_acquired: true,
            lock_key,
            fallback_reason: Some(fallback_reason.into()),
            timeout_seconds: config.timeout_seconds,
            poll_interval_millis: config.poll_interval_millis,
            held_during_live_head_check: true,
            error: None,
        }
    }

    fn failed(
        lock_key: i64,
        config: &MigrationSingleFlightConfig,
        lock_mode: &'static str,
        error: impl Into<String>,
        fallback_reason: Option<String>,
    ) -> Self {
        Self {
            ok: false,
            status: "lock_unavailable",
            lock_mode,
            lock_acquired: false,
            lock_key,
            fallback_reason,
            timeout_seconds: config.timeout_seconds,
            poll_interval_millis: config.poll_interval_millis,
            held_during_live_head_check: false,
            error: Some(error.into()),
        }
    }

    pub(crate) fn to_json(&self) -> Value {
        json!({
            "owner": RUST_METADATA_OWNER,
            "ok": self.ok,
            "status": self.status,
            "lock_mode": self.lock_mode,
            "lock_acquired": self.lock_acquired,
            "lock_name": MIGRATION_LOCK_NAME,
            "lock_key": self.lock_key,
            "fallback_reason": self.fallback_reason,
            "timeout_seconds": self.timeout_seconds,
            "poll_interval_millis": self.poll_interval_millis,
            "held_during_live_head_check": self.held_during_live_head_check,
            "ddl_executed": false,
            "error": self.error,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum PostgresMigrationLockError {
    Timeout(String),
    Unavailable(String),
}

impl std::fmt::Display for PostgresMigrationLockError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Timeout(message) | Self::Unavailable(message) => f.write_str(message),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RustMigrationExecutorShellReport {
    pub(crate) exit_code: i32,
    pub(crate) status: &'static str,
    pub(crate) single_flight: MigrationSingleFlightReport,
    pub(crate) replay_plan: RustMigrationReplayPlan,
    pub(crate) tail_hardening_replay: RustMigrationTailHardeningReplayResult,
    pub(crate) smoke: RustMigrationNoopExecutorSmokeResult,
}

impl RustMigrationExecutorShellReport {
    pub(crate) fn to_json(&self) -> Value {
        json!({
            "owner": RUST_METADATA_OWNER,
            "exit_code": self.exit_code,
            "status": self.status,
            "single_flight": self.single_flight.to_json(),
            "replay_plan": self.replay_plan.to_json(),
            "tail_hardening_replay": self.tail_hardening_replay.to_json(),
            "smoke": self.smoke.to_json(),
        })
    }
}

pub(crate) async fn run_rust_migration_executor_shell(
    db: &DatabaseConnection,
    gate_enabled: bool,
) -> RustMigrationExecutorShellReport {
    let single_flight_config = MigrationSingleFlightConfig::from_env();
    let tail_hardening_replay_gate =
        env_bool(RUST_MIGRATION_TAIL_HARDENING_REPLAY_ENABLED_ENV, false);
    run_rust_migration_executor_shell_with_config_and_tail_gate(
        db,
        gate_enabled,
        &single_flight_config,
        tail_hardening_replay_gate,
    )
    .await
}

pub(crate) async fn run_rust_migration_executor_shell_with_tail_gate(
    db: &DatabaseConnection,
    gate_enabled: bool,
    tail_hardening_replay_gate: bool,
) -> RustMigrationExecutorShellReport {
    let single_flight_config = MigrationSingleFlightConfig::from_env();
    run_rust_migration_executor_shell_with_config_and_tail_gate(
        db,
        gate_enabled,
        &single_flight_config,
        tail_hardening_replay_gate,
    )
    .await
}

#[cfg(test)]
pub(crate) async fn run_rust_migration_already_applied_executor_shell_with_config(
    db: &DatabaseConnection,
    gate_enabled: bool,
    single_flight_config: &MigrationSingleFlightConfig,
) -> RustMigrationExecutorShellReport {
    let tail_hardening_replay_gate =
        env_bool(RUST_MIGRATION_TAIL_HARDENING_REPLAY_ENABLED_ENV, false);
    run_rust_migration_executor_shell_with_config_and_tail_gate(
        db,
        gate_enabled,
        single_flight_config,
        tail_hardening_replay_gate,
    )
    .await
}

async fn run_rust_migration_executor_shell_with_config_and_tail_gate(
    db: &DatabaseConnection,
    gate_enabled: bool,
    single_flight_config: &MigrationSingleFlightConfig,
    tail_hardening_replay_gate: bool,
) -> RustMigrationExecutorShellReport {
    let (single_flight, live_head_check) =
        run_migration_single_flight_live_head_check(db, single_flight_config).await;
    let capacity_check = if single_flight.ok {
        ensure_alembic_version_table_capacity(db).await
    } else {
        Ok(false)
    };
    let live_head_check = if let Err(error) = capacity_check.as_ref() {
        LiveAlembicHeadCheck::query_error(format!(
            "failed ensuring alembic_version.version_num capacity: {error}"
        ))
    } else if single_flight.ok {
        check_live_alembic_head(db).await
    } else {
        live_head_check
    };
    let replay_plan = if single_flight.ok {
        RustMigrationReplayPlan::from_live_head_check(&live_head_check)
    } else {
        RustMigrationReplayPlan::from_live_head_check(&LiveAlembicHeadCheck::not_checked(
            "migration single-flight lock unavailable",
        ))
    };
    let tail_hardening_replay = if single_flight.ok {
        run_rust_migration_tail_hardening_replay(db, &replay_plan, tail_hardening_replay_gate).await
    } else {
        RustMigrationTailHardeningReplayResult::blocked(
            "blocked_by_single_flight",
            tail_hardening_replay_gate,
            &replay_plan,
            "migration single-flight lock was not acquired",
        )
    };
    let live_head_check_for_smoke =
        if tail_hardening_replay.ok && tail_hardening_replay.ddl_executed {
            check_live_alembic_head(db).await
        } else {
            live_head_check
        };
    let smoke = run_rust_migration_noop_executor_smoke_from_live_head(
        gate_enabled,
        if single_flight.ok {
            Some(live_head_check_for_smoke)
        } else {
            None
        },
    );
    let status = if smoke.ok && tail_hardening_replay.ddl_executed {
        "migration_needed_executor_shell_passed"
    } else if smoke.ok {
        "already_applied_executor_shell_passed"
    } else if !single_flight.ok {
        "already_applied_executor_shell_lock_blocked"
    } else if tail_hardening_replay.status == "blocked_initial_schema_requires_postgres" {
        "migration_needed_executor_shell_blocked"
    } else {
        "already_applied_executor_shell_blocked"
    };

    RustMigrationExecutorShellReport {
        exit_code: if smoke.ok { 0 } else { 1 },
        status,
        single_flight,
        replay_plan,
        tail_hardening_replay,
        smoke,
    }
}

pub(crate) async fn run_rust_migration_noop_executor_smoke(
    db: &DatabaseConnection,
    gate_enabled: bool,
) -> RustMigrationNoopExecutorSmokeResult {
    let live_head_check = check_live_alembic_head(db).await;
    run_rust_migration_noop_executor_smoke_from_live_head(gate_enabled, Some(live_head_check))
}

fn run_rust_migration_noop_executor_smoke_from_live_head(
    gate_enabled: bool,
    live_head_check: Option<LiveAlembicHeadCheck>,
) -> RustMigrationNoopExecutorSmokeResult {
    if !gate_enabled {
        return RustMigrationNoopExecutorSmokeResult::disabled();
    }

    let live_head_check = live_head_check.unwrap_or_else(|| {
        LiveAlembicHeadCheck::not_checked("migration single-flight lock unavailable")
    });
    let preflight = RustMigrationExecutorPreflight::from_live_head_check(&live_head_check);

    if preflight.no_op_executor_smoke_ready {
        RustMigrationNoopExecutorSmokeResult::ready(preflight)
    } else {
        RustMigrationNoopExecutorSmokeResult::blocked(preflight)
    }
}

pub(crate) async fn run_rust_migration_tail_hardening_replay(
    db: &DatabaseConnection,
    plan: &RustMigrationReplayPlan,
    gate_enabled: bool,
) -> RustMigrationTailHardeningReplayResult {
    if !gate_enabled {
        return RustMigrationTailHardeningReplayResult::disabled(plan);
    }

    if plan.already_at_head {
        return RustMigrationTailHardeningReplayResult::already_at_head(plan, gate_enabled);
    }

    if !plan.ok {
        return RustMigrationTailHardeningReplayResult::blocked(
            "blocked_by_replay_plan",
            gate_enabled,
            plan,
            plan.blockers
                .first()
                .cloned()
                .unwrap_or_else(|| "migration replay plan is blocked".to_string()),
        );
    }

    if !plan.pending_revisions_all_have_rust_steps {
        return RustMigrationTailHardeningReplayResult::blocked(
            "blocked_pending_revisions_without_rust_steps",
            gate_enabled,
            plan,
            "not every pending revision has Rust SQL step parity",
        );
    }

    if plan_requires_initial_schema_bootstrap(plan)
        && db.get_database_backend() != DatabaseBackend::Postgres
    {
        return RustMigrationTailHardeningReplayResult::blocked(
            "blocked_initial_schema_requires_postgres",
            gate_enabled,
            plan,
            "initial schema bootstrap uses PostgreSQL offline Alembic SQL and must be deploy-smoked against PostgreSQL before deleting the Python migrator source-map",
        );
    }

    let mut executed_revisions = Vec::new();
    let mut executed_sql_step_count = 0;
    for revision_id in &plan.pending_revisions {
        let Some(executable_revision) = rust_executable_revision(revision_id) else {
            return RustMigrationTailHardeningReplayResult::blocked(
                "blocked_missing_rust_revision_steps",
                gate_enabled,
                plan,
                format!("missing Rust SQL steps for revision {revision_id}"),
            );
        };

        match execute_rust_migration_revision_atomically(db, executable_revision).await {
            Ok(executed_steps) => {
                executed_sql_step_count += executed_steps;
                executed_revisions.push(executable_revision.revision);
            }
            Err(failure) => {
                return RustMigrationTailHardeningReplayResult::blocked(
                    failure.status,
                    gate_enabled,
                    plan,
                    failure.blocker,
                );
            }
        }
    }

    RustMigrationTailHardeningReplayResult::applied(
        gate_enabled,
        executed_revisions,
        executed_sql_step_count,
        Some(POSTGRES_ALEMBIC_HEAD.to_string()),
    )
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RustMigrationRevisionExecutionFailure {
    status: &'static str,
    blocker: String,
}

impl RustMigrationRevisionExecutionFailure {
    fn new(status: &'static str, blocker: String) -> Self {
        Self { status, blocker }
    }
}

async fn execute_rust_migration_revision_atomically(
    db: &DatabaseConnection,
    revision: &RustMigrationExecutableRevision,
) -> Result<usize, RustMigrationRevisionExecutionFailure> {
    let transaction = db.begin().await.map_err(|error| {
        RustMigrationRevisionExecutionFailure::new(
            "blocked_transaction_begin_error",
            format!(
                "failed beginning transaction for revision {}: {error}",
                revision.revision
            ),
        )
    })?;

    for (step_index, step) in revision.upgrade_steps.iter().enumerate() {
        if let Err(error) = execute_raw_sql_step(&transaction, step.sql).await {
            let blocker = format!(
                "failed executing revision {} step {} ({}): {error}",
                revision.revision,
                step_index + 1,
                step.statement_kind
            );
            let blocker = append_rollback_diagnostic(blocker, transaction.rollback().await.err());
            return Err(RustMigrationRevisionExecutionFailure::new(
                "blocked_sql_execution_error",
                blocker,
            ));
        }
    }

    if let Err(error) = update_live_alembic_revision(&transaction, revision.revision).await {
        let blocker = format!(
            "failed updating alembic_version to {}: {error}",
            revision.revision
        );
        let blocker = append_rollback_diagnostic(blocker, transaction.rollback().await.err());
        return Err(RustMigrationRevisionExecutionFailure::new(
            "blocked_alembic_version_update_error",
            blocker,
        ));
    }

    transaction.commit().await.map_err(|error| {
        RustMigrationRevisionExecutionFailure::new(
            "blocked_transaction_commit_error",
            format!(
                "failed committing transaction for revision {}: {error}",
                revision.revision
            ),
        )
    })?;

    Ok(revision.upgrade_steps.len())
}

fn append_rollback_diagnostic(blocker: String, rollback_error: Option<DbErr>) -> String {
    match rollback_error {
        Some(error) => format!("{blocker}; transaction rollback also failed: {error}"),
        None => format!("{blocker}; revision transaction rolled back"),
    }
}

fn plan_requires_initial_schema_bootstrap(plan: &RustMigrationReplayPlan) -> bool {
    plan.pending_revisions
        .iter()
        .any(|revision| *revision == "ee0a189f1532")
}

async fn execute_raw_sql_step<C>(db: &C, sql: &str) -> Result<(), DbErr>
where
    C: ConnectionTrait,
{
    let statements = split_sql_script_statements(sql);
    if statements.len() <= 1 {
        return db
            .execute(Statement::from_string(
                db.get_database_backend(),
                sql.to_string(),
            ))
            .await
            .map(|_| ());
    }

    for statement in statements {
        if is_transaction_control_statement(statement) {
            continue;
        }
        db.execute(Statement::from_string(
            db.get_database_backend(),
            statement.to_string(),
        ))
        .await?;
    }
    Ok(())
}

fn split_sql_script_statements(sql: &str) -> Vec<&str> {
    sql.split(";\n")
        .filter_map(normalize_sql_script_statement)
        .collect()
}

fn normalize_sql_script_statement(statement: &str) -> Option<&str> {
    let mut normalized = statement.trim();
    while let Some(stripped) = normalized.strip_prefix("--") {
        let Some((_, rest)) = stripped.split_once('\n') else {
            return None;
        };
        normalized = rest.trim_start();
    }
    if normalized.is_empty() {
        None
    } else {
        Some(normalized)
    }
}

fn is_transaction_control_statement(statement: &str) -> bool {
    matches!(
        statement.trim().to_ascii_uppercase().as_str(),
        "BEGIN" | "COMMIT"
    )
}

async fn update_live_alembic_revision<C>(db: &C, revision: &str) -> Result<(), DbErr>
where
    C: ConnectionTrait,
{
    let statement = match db.get_database_backend() {
        DatabaseBackend::Postgres => "UPDATE alembic_version SET version_num = $1",
        _ => "UPDATE alembic_version SET version_num = ?",
    };
    db.execute(Statement::from_sql_and_values(
        db.get_database_backend(),
        statement,
        [revision.into()],
    ))
    .await
    .map(|_| ())
}

async fn ensure_alembic_version_table_capacity(db: &DatabaseConnection) -> Result<bool, DbErr> {
    if db.get_database_backend() != DatabaseBackend::Postgres {
        return Ok(false);
    }

    let statement = Statement::from_string(
        db.get_database_backend(),
        "SELECT character_maximum_length \
         FROM information_schema.columns \
         WHERE table_schema = current_schema() \
           AND table_name = 'alembic_version' \
           AND column_name = 'version_num' \
         LIMIT 1"
            .to_string(),
    );

    let Some(row) = db.query_one(statement).await? else {
        return Ok(false);
    };
    let Ok(Some(current_length)) = row.try_get::<Option<i32>>("", "character_maximum_length")
    else {
        return Ok(false);
    };
    if current_length >= ALEMBIC_VERSION_NUM_LENGTH {
        return Ok(false);
    }

    db.execute(Statement::from_string(
            db.get_database_backend(),
            format!(
                "ALTER TABLE alembic_version ALTER COLUMN version_num TYPE VARCHAR({ALEMBIC_VERSION_NUM_LENGTH})"
            ),
        ))
        .await?;
    Ok(true)
}

async fn run_migration_single_flight_live_head_check(
    db: &DatabaseConnection,
    config: &MigrationSingleFlightConfig,
) -> (MigrationSingleFlightReport, LiveAlembicHeadCheck) {
    let lock_key = migration_advisory_lock_key(&config.lock_name);
    if db.get_database_backend() == DatabaseBackend::Postgres {
        match acquire_postgres_migration_lock(db, lock_key, config).await {
            Ok(()) => {
                let live_head = check_live_alembic_head(db).await;
                release_postgres_migration_lock(db, lock_key).await;
                return (
                    MigrationSingleFlightReport::postgres(lock_key, config),
                    live_head,
                );
            }
            Err(PostgresMigrationLockError::Timeout(error)) => {
                let report = MigrationSingleFlightReport::failed(
                    lock_key,
                    config,
                    "postgres_advisory_lock",
                    error,
                    None,
                );
                return (
                    report,
                    LiveAlembicHeadCheck::not_checked(
                        "migration PostgreSQL advisory lock timed out",
                    ),
                );
            }
            Err(PostgresMigrationLockError::Unavailable(error)) => {
                let fallback_reason = format!("postgres advisory lock unavailable: {error}");
                match run_file_locked_live_head_check(db, lock_key, config, Some(fallback_reason))
                    .await
                {
                    Ok((report, live_head)) => return (report, live_head),
                    Err((report, live_head)) => return (report, live_head),
                }
            }
        }
    }

    match run_file_locked_live_head_check(
        db,
        lock_key,
        config,
        Some(format!(
            "database backend {:?} does not support PostgreSQL advisory locks",
            db.get_database_backend()
        )),
    )
    .await
    {
        Ok((report, live_head)) => (report, live_head),
        Err((report, live_head)) => (report, live_head),
    }
}

fn env_bool(key: &str, default: bool) -> bool {
    std::env::var(key)
        .map(|value| value == "1" || value.eq_ignore_ascii_case("true"))
        .unwrap_or(default)
}

async fn acquire_postgres_migration_lock(
    db: &DatabaseConnection,
    lock_key: i64,
    config: &MigrationSingleFlightConfig,
) -> Result<(), PostgresMigrationLockError> {
    let started_at = Instant::now();
    while started_at.elapsed() < config.timeout() {
        let statement = Statement::from_sql_and_values(
            db.get_database_backend(),
            "SELECT pg_try_advisory_lock($1) AS acquired",
            [lock_key.into()],
        );
        let row = db
            .query_one(statement)
            .await
            .map_err(|error| PostgresMigrationLockError::Unavailable(error.to_string()))?;
        let acquired = row
            .and_then(|row| row.try_get::<bool>("", "acquired").ok())
            .unwrap_or(false);
        if acquired {
            return Ok(());
        }
        sleep(config.poll_interval()).await;
    }

    Err(PostgresMigrationLockError::Timeout(format!(
        "timed out waiting for PostgreSQL migration advisory lock after {}s",
        config.timeout_seconds
    )))
}

async fn release_postgres_migration_lock(db: &DatabaseConnection, lock_key: i64) {
    let statement = Statement::from_sql_and_values(
        db.get_database_backend(),
        "SELECT pg_advisory_unlock($1)",
        [lock_key.into()],
    );
    if let Err(error) = db.query_one(statement).await {
        tracing::warn!(
            "Failed to release PostgreSQL migration advisory lock: {}",
            error
        );
    }
}

async fn run_file_locked_live_head_check(
    db: &DatabaseConnection,
    lock_key: i64,
    config: &MigrationSingleFlightConfig,
    fallback_reason: Option<String>,
) -> Result<
    (MigrationSingleFlightReport, LiveAlembicHeadCheck),
    (MigrationSingleFlightReport, LiveAlembicHeadCheck),
> {
    let lock_path = PathBuf::from(&config.lock_file_path);
    let started_at = Instant::now();
    loop {
        match OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&lock_path)
        {
            Ok(file) => match file.try_lock_exclusive() {
                Ok(()) => {
                    let live_head = check_live_alembic_head(db).await;
                    if let Err(error) = file.unlock() {
                        tracing::warn!("Failed to release migration file lock: {}", error);
                    }
                    return Ok((
                        MigrationSingleFlightReport::file(
                            lock_key,
                            config,
                            fallback_reason.unwrap_or_else(|| {
                                "PostgreSQL advisory lock not attempted".to_string()
                            }),
                        ),
                        live_head,
                    ));
                }
                Err(error) => {
                    if started_at.elapsed() >= config.timeout() {
                        let report = MigrationSingleFlightReport::failed(
                            lock_key,
                            config,
                            "file_lock",
                            format!(
                                "timed out waiting for migration file lock after {}s: {}",
                                config.timeout_seconds, error
                            ),
                            fallback_reason,
                        );
                        return Err((
                            report,
                            LiveAlembicHeadCheck::not_checked(
                                "migration single-flight file lock unavailable",
                            ),
                        ));
                    }
                }
            },
            Err(error) => {
                let report = MigrationSingleFlightReport::failed(
                    lock_key,
                    config,
                    "file_lock",
                    format!(
                        "failed to open migration lock file {}: {error}",
                        lock_path.display()
                    ),
                    fallback_reason,
                );
                return Err((
                    report,
                    LiveAlembicHeadCheck::not_checked(
                        "migration single-flight file lock unavailable",
                    ),
                ));
            }
        }

        sleep(config.poll_interval()).await;
    }
}

fn migration_advisory_lock_key(name: &str) -> i64 {
    let mut hasher = Sha1::new();
    hasher.update(name.as_bytes());
    let digest = hex::encode(hasher.finalize());
    i64::from_str_radix(&digest[..15], 16).expect("sha1 lock key prefix should fit in i64")
}

pub(crate) async fn check_password_hash_storage_compatibility(
    db: &DatabaseConnection,
) -> PasswordHashStorageCompatibilityCheck {
    if db.get_database_backend() != DatabaseBackend::Postgres {
        return PasswordHashStorageCompatibilityCheck::not_applicable_non_postgres();
    }

    let statement = Statement::from_string(
        DatabaseBackend::Postgres,
        PASSWORD_HASH_STORAGE_METADATA_QUERY.to_string(),
    );

    match db.query_one(statement).await {
        Ok(Some(row)) => {
            let data_type = match row.try_get::<String>("", "data_type") {
                Ok(value) => value,
                Err(error) => {
                    return PasswordHashStorageCompatibilityCheck::query_error(error.to_string());
                }
            };
            let udt_name = match row.try_get::<String>("", "udt_name") {
                Ok(value) => value,
                Err(error) => {
                    return PasswordHashStorageCompatibilityCheck::query_error(error.to_string());
                }
            };
            let character_maximum_length = match row
                .try_get::<Option<i32>>("", "character_maximum_length")
            {
                Ok(value) => value,
                Err(error) => {
                    return PasswordHashStorageCompatibilityCheck::query_error(error.to_string());
                }
            };

            PasswordHashStorageCompatibilityCheck::from_column_metadata(
                data_type,
                udt_name,
                character_maximum_length,
            )
        }
        Ok(None) => PasswordHashStorageCompatibilityCheck::column_missing(),
        Err(error) => PasswordHashStorageCompatibilityCheck::query_error(error.to_string()),
    }
}

pub(crate) async fn check_live_alembic_head(db: &DatabaseConnection) -> LiveAlembicHeadCheck {
    let statement = Statement::from_string(
        db.get_database_backend(),
        "SELECT version_num FROM alembic_version LIMIT 1".to_string(),
    );

    match db.query_one(statement).await {
        Ok(Some(row)) => match row.try_get::<String>("", "version_num") {
            Ok(version_num) => LiveAlembicHeadCheck::from_live_head(version_num),
            Err(error) => LiveAlembicHeadCheck::query_error(error.to_string()),
        },
        Ok(None) => LiveAlembicHeadCheck::empty_table(),
        Err(error) if is_missing_alembic_version_table_error(&error) => {
            LiveAlembicHeadCheck::missing_table(error.to_string())
        }
        Err(error) => LiveAlembicHeadCheck::query_error(error.to_string()),
    }
}

fn is_missing_alembic_version_table_error(error: &DbErr) -> bool {
    let message = error.to_string().to_lowercase();
    message.contains("alembic_version")
        && (message.contains("no such table")
            || message.contains("does not exist")
            || message.contains("undefined table")
            || message.contains("relation")
            || message.contains("not found"))
}

pub(crate) fn build_schema_migration_metadata_contract(
    live_head_check: Option<&LiveAlembicHeadCheck>,
    password_hash_storage_check: Option<&PasswordHashStorageCompatibilityCheck>,
) -> Value {
    let revision_ids = postgres_revision_catalog()
        .iter()
        .map(|entry| entry.revision)
        .collect::<Vec<_>>();
    let revision_files = postgres_revision_catalog()
        .iter()
        .map(|entry| entry.filename)
        .collect::<Vec<_>>();
    let executable_revisions = rust_executable_postgres_revisions()
        .iter()
        .map(|entry| entry.revision)
        .collect::<Vec<_>>();
    let executable_files = rust_executable_postgres_revisions()
        .iter()
        .map(|entry| entry.filename)
        .collect::<Vec<_>>();
    let executable_upgrade_step_count = rust_executable_postgres_revisions()
        .iter()
        .map(|entry| entry.upgrade_steps.len())
        .sum::<usize>();
    let executable_downgrade_step_count = rust_executable_postgres_revisions()
        .iter()
        .map(|entry| entry.downgrade_steps.len())
        .sum::<usize>();
    let fallback_live_head_check;
    let live_head_check = match live_head_check {
        Some(check) => check,
        None => {
            fallback_live_head_check = LiveAlembicHeadCheck::not_checked("database unavailable");
            &fallback_live_head_check
        }
    };
    let fallback_password_hash_storage_check;
    let password_hash_storage_check = match password_hash_storage_check {
        Some(check) => check,
        None => {
            fallback_password_hash_storage_check =
                PasswordHashStorageCompatibilityCheck::not_checked_database_unavailable();
            &fallback_password_hash_storage_check
        }
    };
    let executor_preflight = RustMigrationExecutorPreflight::from_live_head_check(live_head_check);
    let replay_plan = RustMigrationReplayPlan::from_live_head_check(live_head_check);

    json!({
        "owner": RUST_METADATA_OWNER,
        "runtime_migration_owner": MIGRATION_RUNTIME_OWNER,
        "status": "rust_migration_executor_owner_active",
        "production_migration_mode": "explicit_rust_db_migrator_before_rust_startup",
        "startup_schema_sync_allowed": false,
        "active_database_profile": "postgres",
        "postgres_alembic_head": POSTGRES_ALEMBIC_HEAD,
        "legacy_sqlite_profile": {
            "status": "manual_legacy_only",
            "included_in_production_migrator": false,
        },
        "postgres_revision_catalog": {
            "owner": RUST_METADATA_OWNER,
            "revision_count": revision_ids.len(),
            "head": POSTGRES_ALEMBIC_HEAD,
            "revisions": revision_ids,
            "files": revision_files,
            "execution_ready": true,
            "purpose": "Rust-owned revision graph catalog for active migration execution",
        },
        "rust_executable_revision_catalog": {
            "owner": RUST_METADATA_OWNER,
            "revision_count": executable_revisions.len(),
            "total_postgres_revision_count": revision_ids.len(),
            "coverage": RUST_EXECUTABLE_MIGRATION_COVERAGE,
            "revisions": executable_revisions,
            "files": executable_files,
            "upgrade_sql_step_count": executable_upgrade_step_count,
            "downgrade_sql_step_count": executable_downgrade_step_count,
            "ddl_execution_enabled": true,
            "can_replace_python_migrator": true,
        },
        "live_database_head": live_head_check.to_json(),
        "auth_password_hash_storage": password_hash_storage_check.to_json(),
        "rust_migration_executor": executor_preflight.to_json(),
        "rust_migration_replay_plan": replay_plan.to_json(),
        "python_boundary": {
            "status": "migration_wrapper_deleted_after_rust_migrator_cutover",
            "migrator_metadata_package": "backend/migrator_app/models",
            "model_registry_helper": "backend/migrator_app/models/__init__.py",
            "retired_runtime_modules": [
                "backend/migrator_app/config.py",
                "backend/migrator_app/logger.py",
                "backend/migrator_app/model_base.py"
            ],
            "migration_runner": "deleted",
            "alembic_config": "backend/alembic-postgres.ini",
            "postgres_versions": "backend/alembic/postgres/versions",
            "docker_service": "none",
        },
        "rust_boundary": {
            "runtime_entrypoint": "backend-rs/src/main.rs",
            "migration_command": "migration-executor",
            "migration_smoke_command": "migration-needed-executor-smoke",
            "database_connection_owner": "backend-rs/src/db/connection.rs",
            "readiness_evidence": "backend-rs/src/api/health.rs",
            "metadata_contract_owner": "backend-rs/src/services/schema_migration_metadata_service.rs",
        },
        "exit_readiness": {
            "backend_app_removed": true,
            "long_running_python_backend_removed": true,
            "rust_runtime_owns_http_api": true,
            "python_migrator_still_required": false,
            "rust_owned_migration_executor_ready": true,
            "python_zero_ready": true,
        },
        "next_cutover_gate": "archive frozen Python migrator metadata package after deploy path is re-smoked with Rust db-migrator",
        "rollback_boundary": "restore Python db-migrator service and migrator_app source-map if Rust migration executor smoke fails",
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm::{ConnectionTrait, Database};
    use std::collections::BTreeSet;
    use std::fs::OpenOptions;

    const ATOMIC_SUCCESS_STEPS: &[RustMigrationSqlStep] = &[
        RustMigrationSqlStep {
            sql: "INSERT INTO migration_probe (value) VALUES ('first')",
            statement_kind: "test_insert_first",
        },
        RustMigrationSqlStep {
            sql: "INSERT INTO migration_probe (value) VALUES ('second')",
            statement_kind: "test_insert_second",
        },
    ];
    const ATOMIC_SQL_FAILURE_STEPS: &[RustMigrationSqlStep] = &[
        RustMigrationSqlStep {
            sql: "INSERT INTO migration_probe (value) VALUES ('before_failure')",
            statement_kind: "test_insert_before_failure",
        },
        RustMigrationSqlStep {
            sql: "INSERT INTO missing_migration_probe (value) VALUES ('failure')",
            statement_kind: "test_missing_table_failure",
        },
    ];
    const ATOMIC_HEAD_FAILURE_STEPS: &[RustMigrationSqlStep] = &[RustMigrationSqlStep {
        sql: "INSERT INTO migration_probe (value) VALUES ('before_head_failure')",
        statement_kind: "test_insert_before_head_failure",
    }];

    const ATOMIC_SUCCESS_REVISION: RustMigrationExecutableRevision =
        RustMigrationExecutableRevision {
            revision: "test_atomic_success",
            filename: "test_atomic_success.py",
            execution_scope: "test",
            upgrade_steps: ATOMIC_SUCCESS_STEPS,
            downgrade_steps: &[],
        };
    const ATOMIC_SQL_FAILURE_REVISION: RustMigrationExecutableRevision =
        RustMigrationExecutableRevision {
            revision: "test_atomic_sql_failure",
            filename: "test_atomic_sql_failure.py",
            execution_scope: "test",
            upgrade_steps: ATOMIC_SQL_FAILURE_STEPS,
            downgrade_steps: &[],
        };
    const ATOMIC_HEAD_FAILURE_REVISION: RustMigrationExecutableRevision =
        RustMigrationExecutableRevision {
            revision: "test_atomic_head_failure",
            filename: "test_atomic_head_failure.py",
            execution_scope: "test",
            upgrade_steps: ATOMIC_HEAD_FAILURE_STEPS,
            downgrade_steps: &[],
        };

    #[test]
    fn postgres_revision_catalog_matches_current_alembic_single_chain() {
        let catalog = postgres_revision_catalog();

        assert_eq!(catalog.len(), rust_executable_postgres_revisions().len());
        assert_eq!(catalog[0].revision, "ee0a189f1532");
        assert_eq!(catalog[0].down_revision, None);
        assert_eq!(catalog[catalog.len() - 1].revision, POSTGRES_ALEMBIC_HEAD);

        let unique_revisions = catalog
            .iter()
            .map(|entry| entry.revision)
            .collect::<BTreeSet<_>>();
        assert_eq!(unique_revisions.len(), catalog.len());

        for pair in catalog.windows(2) {
            assert_eq!(
                pair[1].down_revision,
                Some(pair[0].revision),
                "revision {} should point to previous revision {}",
                pair[1].revision,
                pair[0].revision
            );
        }

        assert!(catalog.iter().all(|entry| entry.filename.ends_with(".py")));
    }

    #[test]
    fn rust_executable_postgres_revisions_cover_initial_schema_seed_data_settings_system_prompt_foreshadows_prompt_workshop_character_state_settings_api_compat_project_defaults_batch_runtime_and_tail_subset(
    ) {
        let executable = rust_executable_postgres_revisions();

        assert_eq!(executable.len(), postgres_revision_catalog().len());
        assert_eq!(
            executable.first().map(|entry| entry.revision),
            Some("ee0a189f1532")
        );
        assert_eq!(
            executable.last().map(|entry| entry.revision),
            Some(POSTGRES_ALEMBIC_HEAD)
        );
        assert_eq!(
            executable
                .iter()
                .map(|entry| entry.upgrade_steps.len())
                .sum::<usize>(),
            133
        );
        assert!(executable
            .iter()
            .all(|entry| entry.execution_scope == RUST_EXECUTABLE_MIGRATION_COVERAGE));
        assert_eq!(
            rust_executable_revision("ee0a189f1532").map(|entry| entry.upgrade_steps.len()),
            Some(1)
        );
        assert_eq!(
            rust_executable_revision("e411428f00c0").map(|entry| entry.upgrade_steps.len()),
            Some(2)
        );
        assert_eq!(
            rust_executable_revision("a7e4408e1d5b").map(|entry| entry.upgrade_steps.len()),
            Some(1)
        );
        assert_eq!(
            rust_executable_revision("6a73f37e9adb").map(|entry| entry.upgrade_steps.len()),
            Some(3)
        );
        assert_eq!(
            rust_executable_revision("421237957b27").map(|entry| entry.upgrade_steps.len()),
            Some(12)
        );
        assert_eq!(
            rust_executable_revision("d4d253e3f4c6").map(|entry| entry.upgrade_steps.len()),
            Some(4)
        );
        assert_eq!(
            rust_executable_revision("20260222_api_compat").map(|entry| entry.upgrade_steps.len()),
            Some(4)
        );
        assert_eq!(
            rust_executable_revision("e8b4d6c1f2a7").map(|entry| entry.upgrade_steps.len()),
            Some(2)
        );
    }

    #[test]
    fn password_hash_phc_text_revision_keeps_upgrade_and_guarded_downgrade_contract() {
        let revision = rust_executable_revision("20260712_password_hash_phc_text")
            .expect("password hash PHC text revision should be executable");

        assert_eq!(revision.filename, "20260712_1200_password_hash_phc_text.py");
        assert_eq!(revision.upgrade_steps.len(), 2);
        assert!(revision.upgrade_steps[0]
            .sql
            .contains("ALTER COLUMN password_hash TYPE TEXT"));
        assert!(revision.upgrade_steps[1]
            .sql
            .contains("Argon2 PHC 或兼容的 legacy SHA256"));
        assert_eq!(revision.downgrade_steps.len(), 3);
        assert!(revision.downgrade_steps[0]
            .sql
            .contains("length(password_hash) > 64"));
        assert!(revision.downgrade_steps[0]
            .sql
            .contains("cannot downgrade password_hash to VARCHAR(64)"));
        assert!(revision.downgrade_steps[1]
            .sql
            .contains("ALTER COLUMN password_hash TYPE VARCHAR(64)"));
        assert!(INITIAL_SCHEMA_SQL.contains("password_hash TEXT NOT NULL"));
        assert!(INITIAL_SCHEMA_SQL.contains("Argon2 PHC 或兼容的 legacy SHA256"));
        assert!(!INITIAL_SCHEMA_SQL.contains("password_hash VARCHAR(64) NOT NULL"));
    }

    #[test]
    fn autopilot_invocation_audit_revision_keeps_durable_indexes() {
        let revision = rust_executable_revision("20260716_autopilot_invocation_audit")
            .expect("autopilot invocation audit revision should be executable");

        assert_eq!(
            revision.filename,
            "20260716_2200_autopilot_invocation_audit.py"
        );
        assert_eq!(revision.upgrade_steps.len(), 3);
        assert!(revision.upgrade_steps[0]
            .sql
            .contains("CREATE TABLE autopilot_invocation_audits"));
        assert!(revision.upgrade_steps[1]
            .sql
            .contains("CREATE UNIQUE INDEX uq_autopilot_invocation_audits_task_id"));
        assert!(revision.upgrade_steps[2]
            .sql
            .contains("ix_autopilot_invocation_audits_project_created_at"));
        assert_eq!(revision.downgrade_steps.len(), 1);
        assert!(revision.downgrade_steps[0]
            .sql
            .contains("DROP TABLE autopilot_invocation_audits"));
    }

    #[test]
    fn durable_novel_autopilot_revision_has_run_step_constraints() {
        let revision = rust_executable_revision("20260719_durable_novel_autopilot")
            .expect("durable novel autopilot revision should be executable");

        assert_eq!(
            revision.filename,
            "20260719_1200_durable_novel_autopilot.py"
        );
        assert_eq!(revision.upgrade_steps.len(), 5);
        assert!(revision.upgrade_steps[0]
            .sql
            .contains("CREATE TABLE novel_autopilot_runs"));
        assert!(revision.upgrade_steps[1]
            .sql
            .contains("uq_novel_autopilot_runs_active_scope_key"));
        assert!(revision.upgrade_steps[3]
            .sql
            .contains("CREATE TABLE novel_autopilot_step_runs"));
        assert!(revision.upgrade_steps[3]
            .sql
            .contains("UNIQUE (run_id, step_key, attempt)"));
        assert!(revision.upgrade_steps[4]
            .sql
            .contains("ix_novel_autopilot_step_runs_run_status_created_at"));
        assert_eq!(revision.downgrade_steps.len(), 2);
        assert!(revision.downgrade_steps[0]
            .sql
            .contains("DROP TABLE novel_autopilot_step_runs"));
        assert!(revision.downgrade_steps[1]
            .sql
            .contains("DROP TABLE novel_autopilot_runs"));
    }

    #[test]
    fn autopilot_user_id_capacity_revision_remains_executable() {
        let revision = rust_executable_revision("20260719_autopilot_user_id_capacity")
            .expect("autopilot user id capacity revision should be executable");

        assert_eq!(
            revision.filename,
            "20260719_1700_novel_autopilot_user_id_capacity.py"
        );
        assert_eq!(revision.upgrade_steps.len(), 1);
        assert!(revision.upgrade_steps[0]
            .sql
            .contains("ALTER COLUMN user_id TYPE VARCHAR(100)"));
        assert_eq!(revision.downgrade_steps.len(), 1);
        assert!(revision.downgrade_steps[0]
            .sql
            .contains("ALTER COLUMN user_id TYPE VARCHAR(36)"));
    }

    #[test]
    fn autopilot_audit_actor_id_capacity_revision_remains_executable() {
        let revision = rust_executable_revision("20260720_audit_actor_id_capacity")
            .expect("autopilot audit actor id capacity revision should be executable");

        assert_eq!(
            revision.filename,
            "20260720_0900_autopilot_audit_actor_user_id_capacity.py"
        );
        assert_eq!(revision.upgrade_steps.len(), 1);
        assert!(revision.upgrade_steps[0]
            .sql
            .contains("ALTER COLUMN actor_user_id TYPE VARCHAR(100)"));
        assert_eq!(revision.downgrade_steps.len(), 1);
        assert!(revision.downgrade_steps[0]
            .sql
            .contains("ALTER COLUMN actor_user_id TYPE VARCHAR(36)"));
    }

    #[test]
    fn autopilot_retry_backoff_revision_is_the_catalog_head() {
        let revision = rust_executable_revision(POSTGRES_ALEMBIC_HEAD)
            .expect("autopilot retry backoff revision should be executable");

        assert_eq!(POSTGRES_ALEMBIC_HEAD, "20260807_autopilot_retry_backoff");
        assert_eq!(
            revision.filename,
            "20260807_1200_novel_autopilot_retry_backoff.py"
        );
        assert_eq!(revision.upgrade_steps.len(), 2);
        assert!(revision.upgrade_steps[0]
            .sql
            .contains("ADD COLUMN next_attempt_at"));
        assert!(revision.upgrade_steps[1]
            .sql
            .contains("ix_novel_autopilot_runs_status_next_attempt_at"));
        assert_eq!(revision.downgrade_steps.len(), 2);
        assert!(revision.downgrade_steps[0]
            .sql
            .contains("DROP INDEX ix_novel_autopilot_runs_status_next_attempt_at"));
        assert!(revision.downgrade_steps[1]
            .sql
            .contains("DROP COLUMN next_attempt_at"));
    }

    #[test]
    fn fresh_autopilot_schema_chain_creates_run_before_retry_backoff_column() {
        let revisions = rust_executable_postgres_revisions();
        let durable_index = revisions
            .iter()
            .position(|entry| entry.revision == "20260719_durable_novel_autopilot")
            .expect("durable novel autopilot revision should be executable");
        let retry_backoff_index = revisions
            .iter()
            .position(|entry| entry.revision == POSTGRES_ALEMBIC_HEAD)
            .expect("autopilot retry backoff revision should be executable");

        assert!(durable_index < retry_backoff_index);
        assert!(revisions[durable_index].upgrade_steps[0]
            .sql
            .contains("CREATE TABLE novel_autopilot_runs"));
        assert!(revisions[retry_backoff_index].upgrade_steps[0]
            .sql
            .contains("ADD COLUMN next_attempt_at"));
        assert!(revisions[retry_backoff_index].upgrade_steps[1]
            .sql
            .contains("ix_novel_autopilot_runs_status_next_attempt_at"));
        assert!(!INITIAL_SCHEMA_SQL.contains("novel_autopilot_runs"));
    }

    #[test]
    fn password_hash_storage_accepts_unbounded_text_target_contract() {
        let check = PasswordHashStorageCompatibilityCheck::from_column_metadata(
            "text".to_string(),
            "text".to_string(),
            None,
        );

        assert_eq!(check.status, "compatible_unbounded_text");
        assert_eq!(check.supports_canonical_argon2, Some(true));
        assert_eq!(check.matches_target_storage_contract, Some(true));
        assert!(check.allows_readiness);
    }

    #[test]
    fn password_hash_storage_accepts_sufficient_bounded_varchar_without_marking_target_complete() {
        let check = PasswordHashStorageCompatibilityCheck::from_column_metadata(
            "character varying".to_string(),
            "varchar".to_string(),
            Some(CANONICAL_ARGON2_VERIFIER_STORAGE_LENGTH as i32),
        );

        assert_eq!(check.status, "compatible_bounded_capacity");
        assert_eq!(check.supports_canonical_argon2, Some(true));
        assert_eq!(check.matches_target_storage_contract, Some(false));
        assert!(check.allows_readiness);
    }

    #[test]
    fn password_hash_storage_blocks_legacy_varchar_64_capacity() {
        let check = PasswordHashStorageCompatibilityCheck::from_column_metadata(
            "character varying".to_string(),
            "varchar".to_string(),
            Some(64),
        );

        assert_eq!(check.status, "blocked_capacity_too_small");
        assert_eq!(check.character_maximum_length, Some(64));
        assert_eq!(check.supports_canonical_argon2, Some(false));
        assert!(!check.allows_readiness);
    }

    #[test]
    fn password_hash_storage_blocks_missing_or_unsupported_columns() {
        let missing = PasswordHashStorageCompatibilityCheck::column_missing();
        let unsupported = PasswordHashStorageCompatibilityCheck::from_column_metadata(
            "bytea".to_string(),
            "bytea".to_string(),
            None,
        );

        assert_eq!(missing.status, "blocked_column_missing");
        assert!(!missing.allows_readiness);
        assert_eq!(unsupported.status, "blocked_unsupported_type");
        assert!(!unsupported.allows_readiness);
    }

    #[tokio::test]
    async fn password_hash_storage_check_is_explicitly_not_applicable_for_sqlite() {
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("connect sqlite memory db");

        let check = check_password_hash_storage_compatibility(&db).await;

        assert_eq!(check.status, "not_applicable_non_postgres");
        assert!(!check.required_for_readiness);
        assert!(check.allows_readiness);
        assert_eq!(check.supports_canonical_argon2, None);
    }

    #[test]
    fn schema_migration_contract_marks_rust_migration_executor_as_active_boundary() {
        let contract = build_schema_migration_metadata_contract(None, None);

        assert_eq!(contract["owner"], RUST_METADATA_OWNER);
        assert_eq!(contract["runtime_migration_owner"], MIGRATION_RUNTIME_OWNER);
        assert_eq!(
            contract["production_migration_mode"],
            "explicit_rust_db_migrator_before_rust_startup"
        );
        assert_eq!(contract["startup_schema_sync_allowed"], false);
        assert_eq!(contract["postgres_alembic_head"], POSTGRES_ALEMBIC_HEAD);
        assert_eq!(
            contract["postgres_revision_catalog"]["revision_count"],
            json!(postgres_revision_catalog().len())
        );
        assert_eq!(
            contract["postgres_revision_catalog"]["head"],
            POSTGRES_ALEMBIC_HEAD
        );
        assert_eq!(
            contract["postgres_revision_catalog"]["execution_ready"],
            true
        );
        assert_eq!(
            contract["rust_executable_revision_catalog"]["revision_count"],
            json!(rust_executable_postgres_revisions().len())
        );
        assert_eq!(
            contract["rust_executable_revision_catalog"]["coverage"],
            RUST_EXECUTABLE_MIGRATION_COVERAGE
        );
        assert_eq!(
            contract["rust_executable_revision_catalog"]["ddl_execution_enabled"],
            true
        );
        assert_eq!(contract["live_database_head"]["status"], "not_checked");
        assert_eq!(
            contract["live_database_head"]["expected_head"],
            POSTGRES_ALEMBIC_HEAD
        );
        assert_eq!(
            contract["live_database_head"]["matches_catalog_head"],
            false
        );
        assert_eq!(
            contract["auth_password_hash_storage"]["status"],
            "not_checked_database_unavailable"
        );
        assert_eq!(
            contract["auth_password_hash_storage"]["required_minimum_length"],
            CANONICAL_ARGON2_VERIFIER_STORAGE_LENGTH
        );
        assert_eq!(
            contract["auth_password_hash_storage"]["target_storage_contract"],
            "unbounded_text"
        );
        assert_eq!(
            contract["auth_password_hash_storage"]["allows_readiness"],
            false
        );
        assert_eq!(
            contract["rust_migration_executor"]["status"],
            "blocked_live_head_not_checked"
        );
        assert_eq!(
            contract["rust_migration_executor"]["ddl_execution_enabled"],
            false
        );
        assert_eq!(
            contract["rust_migration_executor"]["can_replace_python_migrator"],
            false
        );
        assert_eq!(
            contract["rust_migration_replay_plan"]["status"],
            "blocked_live_head_not_checked"
        );
        assert_eq!(
            contract["rust_migration_replay_plan"]["ddl_replay_ready"],
            false
        );
        assert_eq!(
            contract["rust_migration_replay_plan"]["can_replace_python_migrator"],
            false
        );
        assert_eq!(
            contract["python_boundary"]["status"],
            "migration_wrapper_deleted_after_rust_migrator_cutover"
        );
        assert_eq!(
            contract["python_boundary"]["migrator_metadata_package"],
            "backend/migrator_app/models"
        );
        assert_eq!(
            contract["python_boundary"]["model_registry_helper"],
            "backend/migrator_app/models/__init__.py"
        );
        assert_eq!(
            contract["python_boundary"]["retired_runtime_modules"][0],
            "backend/migrator_app/config.py"
        );
        assert_eq!(contract["python_boundary"]["migration_runner"], "deleted");
        assert_eq!(contract["python_boundary"]["docker_service"], "none");
        assert_eq!(
            contract["rust_boundary"]["metadata_contract_owner"],
            "backend-rs/src/services/schema_migration_metadata_service.rs"
        );
        assert_eq!(
            contract["rust_boundary"]["migration_command"],
            "migration-executor"
        );
        assert_eq!(
            contract["rust_boundary"]["migration_smoke_command"],
            "migration-needed-executor-smoke"
        );
        assert_eq!(
            contract["exit_readiness"]["long_running_python_backend_removed"],
            true
        );
        assert_eq!(
            contract["exit_readiness"]["python_migrator_still_required"],
            false
        );
        assert_eq!(
            contract["exit_readiness"]["rust_owned_migration_executor_ready"],
            true
        );
        assert_eq!(contract["exit_readiness"]["python_zero_ready"], true);
        assert_eq!(
            contract["next_cutover_gate"],
            "archive frozen Python migrator metadata package after deploy path is re-smoked with Rust db-migrator"
        );
    }

    #[test]
    fn migration_advisory_lock_key_matches_rust_migrator_contract() {
        assert_eq!(
            migration_advisory_lock_key(MIGRATION_LOCK_NAME),
            658213001019093484
        );
    }

    #[test]
    fn rust_migration_replay_plan_reports_already_applied_head_without_ddl_readiness() {
        let check = LiveAlembicHeadCheck::from_live_head(POSTGRES_ALEMBIC_HEAD.to_string());

        let plan = RustMigrationReplayPlan::from_live_head_check(&check);

        assert!(plan.ok);
        assert_eq!(plan.status, "already_at_catalog_head");
        assert!(plan.already_at_head);
        assert!(plan.pending_revisions.is_empty());
        assert!(!plan.ddl_replay_ready);
        assert!(!plan.ddl_executed);
        assert!(plan.can_replace_python_migrator);
        assert_eq!(plan.to_json()["pending_revision_count"], 0);
    }

    #[test]
    fn rust_migration_replay_plan_lists_pending_catalog_revisions() {
        let check = LiveAlembicHeadCheck::missing_table("no alembic_version table");

        let plan = RustMigrationReplayPlan::from_live_head_check(&check);

        assert!(plan.ok);
        assert_eq!(plan.status, "initial_schema_bootstrap_has_rust_steps");
        assert!(!plan.already_at_head);
        assert_eq!(
            plan.pending_revisions,
            vec![
                "ee0a189f1532",
                "e411428f00c0",
                "a7e4408e1d5b",
                "6a73f37e9adb",
                "421237957b27",
                "d4d253e3f4c6",
                "20260222_api_compat",
                "b3f6c1a9d2e4",
                "c4e9d1b7a2f0",
                "e8b4d6c1f2a7",
                "20260322_proj_gen_defaults",
                "20260323_proj_quality_prefs",
                "20260325_batch_runtime_store",
                "20260325_batch_workflow_state",
                "20260517_analysis_task_hardening",
                "20260517_batch_task_defaults",
                "20260517_regeneration_task_defaults",
                "20260517_settings_core_defaults",
                "20260517_project_core_defaults",
                "20260712_password_hash_phc_text",
                "20260716_autopilot_invocation_audit",
                "20260719_durable_novel_autopilot",
                "20260719_analysis_content_digest",
                "20260719_autopilot_user_id_capacity",
                "20260720_audit_actor_id_capacity",
                POSTGRES_ALEMBIC_HEAD,
            ]
        );
        assert_eq!(plan.pending_files.len(), postgres_revision_catalog().len());
        assert_eq!(
            plan.rust_executable_pending_revisions,
            vec![
                "ee0a189f1532",
                "e411428f00c0",
                "a7e4408e1d5b",
                "6a73f37e9adb",
                "421237957b27",
                "d4d253e3f4c6",
                "20260222_api_compat",
                "b3f6c1a9d2e4",
                "c4e9d1b7a2f0",
                "e8b4d6c1f2a7",
                "20260322_proj_gen_defaults",
                "20260323_proj_quality_prefs",
                "20260325_batch_runtime_store",
                "20260325_batch_workflow_state",
                "20260517_analysis_task_hardening",
                "20260517_batch_task_defaults",
                "20260517_regeneration_task_defaults",
                "20260517_settings_core_defaults",
                "20260517_project_core_defaults",
                "20260712_password_hash_phc_text",
                "20260716_autopilot_invocation_audit",
                "20260719_durable_novel_autopilot",
                "20260719_analysis_content_digest",
                "20260719_autopilot_user_id_capacity",
                "20260720_audit_actor_id_capacity",
                POSTGRES_ALEMBIC_HEAD,
            ]
        );
        assert!(plan.pending_revisions_all_have_rust_steps);
        assert_eq!(plan.rust_executable_pending_sql_step_count, 133);
        assert!(plan.ddl_replay_ready);
        assert!(plan.can_replace_python_migrator);
    }

    #[test]
    fn rust_migration_replay_plan_from_previous_head_applies_password_hash_audit_and_durable_run_revisions(
    ) {
        let check =
            LiveAlembicHeadCheck::from_live_head("20260517_project_core_defaults".to_string());

        let plan = RustMigrationReplayPlan::from_live_head_check(&check);

        assert!(plan.ok);
        assert_eq!(plan.status, "pending_catalog_revisions_have_rust_steps");
        assert_eq!(
            plan.pending_revisions,
            vec![
                "20260712_password_hash_phc_text",
                "20260716_autopilot_invocation_audit",
                "20260719_durable_novel_autopilot",
                "20260719_analysis_content_digest",
                "20260719_autopilot_user_id_capacity",
                "20260720_audit_actor_id_capacity",
                POSTGRES_ALEMBIC_HEAD,
            ]
        );
        assert_eq!(
            plan.rust_executable_pending_revisions,
            vec![
                "20260712_password_hash_phc_text",
                "20260716_autopilot_invocation_audit",
                "20260719_durable_novel_autopilot",
                "20260719_analysis_content_digest",
                "20260719_autopilot_user_id_capacity",
                "20260720_audit_actor_id_capacity",
                POSTGRES_ALEMBIC_HEAD,
            ]
        );
        assert_eq!(plan.rust_executable_pending_sql_step_count, 15);
        assert!(plan.pending_revisions_all_have_rust_steps);
        assert!(plan.ddl_replay_ready);
    }

    #[test]
    fn initial_schema_sql_script_splitter_keeps_first_create_table_after_alembic_comment() {
        let statements = split_sql_script_statements(INITIAL_SCHEMA_SQL);

        assert!(statements
            .iter()
            .any(|statement| statement.starts_with("CREATE TABLE batch_generation_tasks")));
        assert!(statements
            .iter()
            .any(|statement| statement.starts_with("INSERT INTO alembic_version")));
        assert!(statements.iter().any(|statement| statement
            .starts_with("CREATE INDEX ix_organization_members_organization_id")));
    }

    #[tokio::test]
    async fn revision_transaction_commits_sql_steps_and_head_together() {
        let db = setup_live_head_db().await;
        create_migration_probe_table(&db).await;
        create_alembic_version_table(&db).await;
        insert_alembic_version(&db, "old_revision").await;

        let executed_steps =
            execute_rust_migration_revision_atomically(&db, &ATOMIC_SUCCESS_REVISION)
                .await
                .expect("atomic revision should commit");

        assert_eq!(executed_steps, 2);
        assert_eq!(migration_probe_count(&db).await, 2);
        assert_eq!(
            live_revision(&db).await.as_deref(),
            Some("test_atomic_success")
        );
    }

    #[tokio::test]
    async fn revision_transaction_rolls_back_prior_steps_when_sql_fails() {
        let db = setup_live_head_db().await;
        create_migration_probe_table(&db).await;
        create_alembic_version_table(&db).await;
        insert_alembic_version(&db, "old_revision").await;

        let failure = execute_rust_migration_revision_atomically(&db, &ATOMIC_SQL_FAILURE_REVISION)
            .await
            .expect_err("invalid SQL step should fail the revision");

        assert_eq!(failure.status, "blocked_sql_execution_error");
        assert!(failure
            .blocker
            .contains("step 2 (test_missing_table_failure)"));
        assert!(failure.blocker.contains("revision transaction rolled back"));
        assert_eq!(migration_probe_count(&db).await, 0);
        assert_eq!(live_revision(&db).await.as_deref(), Some("old_revision"));
    }

    #[tokio::test]
    async fn revision_transaction_rolls_back_sql_when_head_update_fails() {
        let db = setup_live_head_db().await;
        create_migration_probe_table(&db).await;

        let failure =
            execute_rust_migration_revision_atomically(&db, &ATOMIC_HEAD_FAILURE_REVISION)
                .await
                .expect_err("missing alembic_version table should fail the revision");

        assert_eq!(failure.status, "blocked_alembic_version_update_error");
        assert!(failure
            .blocker
            .contains("failed updating alembic_version to test_atomic_head_failure"));
        assert!(failure.blocker.contains("revision transaction rolled back"));
        assert_eq!(migration_probe_count(&db).await, 0);
    }

    #[test]
    fn rust_migration_replay_plan_blocks_unknown_live_revision() {
        let check = LiveAlembicHeadCheck::from_live_head("external_revision".to_string());

        let plan = RustMigrationReplayPlan::from_live_head_check(&check);

        assert!(!plan.ok);
        assert_eq!(plan.status, "blocked_unknown_live_revision");
        assert_eq!(plan.current_revision.as_deref(), Some("external_revision"));
        assert!(plan.pending_revisions.is_empty());
        assert!(!plan.ddl_replay_ready);
    }

    #[tokio::test]
    async fn live_alembic_head_check_reports_missing_table_without_mutation() {
        let db = setup_live_head_db().await;

        let check = check_live_alembic_head(&db).await;

        assert_eq!(check.status, "table_missing");
        assert_eq!(check.expected_head, POSTGRES_ALEMBIC_HEAD);
        assert_eq!(check.actual_head, None);
        assert!(!check.matches_catalog_head);
    }

    #[tokio::test]
    async fn live_alembic_head_check_reports_empty_table() {
        let db = setup_live_head_db().await;
        create_alembic_version_table(&db).await;

        let check = check_live_alembic_head(&db).await;

        assert_eq!(check.status, "empty_table");
        assert_eq!(check.actual_head, None);
        assert!(!check.matches_catalog_head);
    }

    #[tokio::test]
    async fn live_alembic_head_check_matches_catalog_head() {
        let db = setup_live_head_db().await;
        create_alembic_version_table(&db).await;
        insert_alembic_version(&db, POSTGRES_ALEMBIC_HEAD).await;

        let check = check_live_alembic_head(&db).await;

        assert_eq!(check.status, "head_matches");
        assert_eq!(check.actual_head.as_deref(), Some(POSTGRES_ALEMBIC_HEAD));
        assert!(check.matches_catalog_head);
        assert_eq!(check.to_json()["read_only"], true);

        let preflight = RustMigrationExecutorPreflight::from_live_head_check(&check);
        assert_eq!(preflight.status, "preflight_ready_for_noop_executor_smoke");
        assert!(preflight.no_op_executor_smoke_ready);
        assert!(preflight.can_replace_python_migrator);
        assert!(preflight.blockers.is_empty());
        assert_eq!(preflight.to_json()["ddl_execution_enabled"], false);
    }

    #[tokio::test]
    async fn alembic_version_capacity_ensure_noops_for_non_postgres() {
        let db = setup_live_head_db().await;
        create_alembic_version_table(&db).await;
        insert_alembic_version(&db, POSTGRES_ALEMBIC_HEAD).await;

        let changed = ensure_alembic_version_table_capacity(&db)
            .await
            .expect("capacity check should no-op on sqlite");

        assert!(!changed);
        let check = check_live_alembic_head(&db).await;
        assert_eq!(check.status, "head_matches");
    }

    #[tokio::test]
    async fn live_alembic_head_check_reports_catalog_mismatch() {
        let db = setup_live_head_db().await;
        create_alembic_version_table(&db).await;
        insert_alembic_version(&db, "old_revision").await;

        let check = check_live_alembic_head(&db).await;

        assert_eq!(check.status, "head_mismatch");
        assert_eq!(check.actual_head.as_deref(), Some("old_revision"));
        assert!(!check.matches_catalog_head);

        let preflight = RustMigrationExecutorPreflight::from_live_head_check(&check);
        assert_eq!(preflight.status, "blocked_live_head_mismatch");
        assert!(!preflight.no_op_executor_smoke_ready);
        assert_eq!(preflight.live_head.as_deref(), Some("old_revision"));
    }

    #[test]
    fn rust_migration_executor_preflight_blocks_unchecked_live_head() {
        let check = LiveAlembicHeadCheck::not_checked("database unavailable");
        let preflight = RustMigrationExecutorPreflight::from_live_head_check(&check);

        assert_eq!(preflight.status, "blocked_live_head_not_checked");
        assert!(!preflight.no_op_executor_smoke_ready);
        assert!(!preflight.can_replace_python_migrator);
        assert_eq!(preflight.catalog_head, POSTGRES_ALEMBIC_HEAD);
    }

    #[tokio::test]
    async fn rust_migration_noop_executor_smoke_is_disabled_by_default() {
        let db = setup_live_head_db().await;

        let result = run_rust_migration_noop_executor_smoke(&db, false).await;

        assert!(!result.ok);
        assert_eq!(result.status, "disabled_by_config");
        assert!(!result.gate_enabled);
        assert!(!result.ddl_executed);
        assert_eq!(result.rollback_boundary, "python_db_migrator_alembic");
    }

    #[tokio::test]
    async fn rust_migration_noop_executor_smoke_passes_for_already_applied_head() {
        let db = setup_live_head_db().await;
        create_alembic_version_table(&db).await;
        insert_alembic_version(&db, POSTGRES_ALEMBIC_HEAD).await;

        let result = run_rust_migration_noop_executor_smoke(&db, true).await;

        assert!(result.ok);
        assert_eq!(result.status, "noop_executor_smoke_passed");
        assert!(result.gate_enabled);
        assert!(!result.ddl_executed);
        assert_eq!(result.live_head.as_deref(), Some(POSTGRES_ALEMBIC_HEAD));
        assert_eq!(result.to_json()["ddl_executed"], false);
    }

    #[tokio::test]
    async fn rust_migration_noop_executor_smoke_blocks_mismatched_head() {
        let db = setup_live_head_db().await;
        create_alembic_version_table(&db).await;
        insert_alembic_version(&db, "old_revision").await;

        let result = run_rust_migration_noop_executor_smoke(&db, true).await;

        assert!(!result.ok);
        assert_eq!(result.status, "blocked_by_preflight");
        assert!(result.gate_enabled);
        assert!(!result.ddl_executed);
        assert_eq!(result.live_head.as_deref(), Some("old_revision"));
    }

    #[tokio::test]
    async fn rust_tail_hardening_replay_is_disabled_by_default() {
        let db = setup_live_head_db().await;
        let check =
            LiveAlembicHeadCheck::from_live_head("20260517_batch_task_defaults".to_string());
        let plan = RustMigrationReplayPlan::from_live_head_check(&check);

        let result = run_rust_migration_tail_hardening_replay(&db, &plan, false).await;

        assert!(!result.ok);
        assert_eq!(result.status, "disabled_by_config");
        assert!(!result.gate_enabled);
        assert!(!result.ddl_executed);
        assert_eq!(result.executed_sql_step_count, 0);
    }

    #[tokio::test]
    async fn rust_tail_hardening_replay_noops_when_already_at_head() {
        let db = setup_live_head_db().await;
        let check = LiveAlembicHeadCheck::from_live_head(POSTGRES_ALEMBIC_HEAD.to_string());
        let plan = RustMigrationReplayPlan::from_live_head_check(&check);

        let result = run_rust_migration_tail_hardening_replay(&db, &plan, true).await;

        assert!(result.ok, "{result:?}");
        assert_eq!(result.status, "already_at_catalog_head");
        assert!(result.gate_enabled);
        assert!(!result.ddl_executed);
        assert_eq!(result.executed_revisions.len(), 0);
        assert_eq!(
            result.final_revision.as_deref(),
            Some(POSTGRES_ALEMBIC_HEAD)
        );
    }

    #[tokio::test]
    async fn rust_tail_hardening_replay_blocks_initial_schema_bootstrap_without_postgres() {
        let db = setup_live_head_db().await;
        let check = LiveAlembicHeadCheck::missing_table("no such table: alembic_version");
        let plan = RustMigrationReplayPlan::from_live_head_check(&check);

        let result = run_rust_migration_tail_hardening_replay(&db, &plan, true).await;

        assert!(!result.ok);
        assert_eq!(result.status, "blocked_initial_schema_requires_postgres");
        assert!(result.gate_enabled);
        assert!(!result.ddl_executed);
        assert!(result.executed_revisions.is_empty());
        assert_eq!(result.executed_sql_step_count, 0);
        assert_eq!(result.final_revision, None);
        assert!(result
            .blockers
            .iter()
            .any(|blocker| blocker.contains("PostgreSQL offline Alembic SQL")));
    }

    #[tokio::test]
    async fn rust_migration_already_applied_executor_shell_reports_success_exit_code() {
        let db = setup_live_head_db().await;
        create_alembic_version_table(&db).await;
        insert_alembic_version(&db, POSTGRES_ALEMBIC_HEAD).await;
        let config = test_single_flight_config("shell-success");

        let report =
            run_rust_migration_already_applied_executor_shell_with_config(&db, true, &config).await;

        assert_eq!(report.exit_code, 0);
        assert_eq!(report.status, "already_applied_executor_shell_passed");
        assert!(report.single_flight.ok);
        assert_eq!(report.single_flight.lock_mode, "file_lock");
        assert!(report.single_flight.lock_acquired);
        assert!(report.single_flight.held_during_live_head_check);
        assert_eq!(report.replay_plan.status, "already_at_catalog_head");
        assert_eq!(report.replay_plan.pending_revisions.len(), 0);
        assert!(!report.replay_plan.ddl_replay_ready);
        assert_eq!(report.tail_hardening_replay.status, "disabled_by_config");
        assert!(!report.tail_hardening_replay.ddl_executed);
        assert!(report.smoke.ok);
        assert_eq!(
            report.smoke.live_head.as_deref(),
            Some(POSTGRES_ALEMBIC_HEAD)
        );
        assert_eq!(
            report.to_json()["single_flight"]["lock_key"],
            migration_advisory_lock_key(MIGRATION_LOCK_NAME)
        );
        assert_eq!(
            report.to_json()["replay_plan"]["can_replace_python_migrator"],
            true
        );
        assert_eq!(report.to_json()["smoke"]["ddl_executed"], false);
    }

    #[tokio::test]
    async fn rust_migration_already_applied_executor_shell_reports_blocked_exit_code() {
        let db = setup_live_head_db().await;
        create_alembic_version_table(&db).await;
        insert_alembic_version(&db, "old_revision").await;
        let config = test_single_flight_config("shell-blocked");

        let report =
            run_rust_migration_already_applied_executor_shell_with_config(&db, true, &config).await;

        assert_eq!(report.exit_code, 1);
        assert_eq!(report.status, "already_applied_executor_shell_blocked");
        assert!(report.single_flight.ok);
        assert!(report.single_flight.lock_acquired);
        assert_eq!(report.replay_plan.status, "blocked_unknown_live_revision");
        assert_eq!(report.tail_hardening_replay.status, "disabled_by_config");
        assert!(!report.smoke.ok);
        assert_eq!(report.smoke.live_head.as_deref(), Some("old_revision"));
    }

    #[tokio::test]
    async fn rust_migration_executor_shell_blocks_when_single_flight_lock_unavailable() {
        let db = setup_live_head_db().await;
        create_alembic_version_table(&db).await;
        insert_alembic_version(&db, POSTGRES_ALEMBIC_HEAD).await;
        let config = test_single_flight_config("shell-lock-blocked");
        let lock_file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&config.lock_file_path)
            .expect("open lock file");
        lock_file
            .try_lock_exclusive()
            .expect("hold exclusive lock for test");

        let report =
            run_rust_migration_already_applied_executor_shell_with_config(&db, true, &config).await;

        lock_file.unlock().expect("release test lock");

        assert_eq!(report.exit_code, 1);
        assert_eq!(report.status, "already_applied_executor_shell_lock_blocked");
        assert!(!report.single_flight.ok);
        assert!(!report.single_flight.lock_acquired);
        assert!(!report.single_flight.held_during_live_head_check);
        assert_eq!(report.replay_plan.status, "blocked_live_head_not_checked");
        assert_eq!(
            report.tail_hardening_replay.status,
            "blocked_by_single_flight"
        );
        assert_eq!(report.smoke.status, "blocked_by_preflight");
        assert_eq!(report.smoke.live_head, None);
        assert_eq!(report.smoke.ddl_executed, false);
    }

    async fn setup_live_head_db() -> sea_orm::DatabaseConnection {
        Database::connect("sqlite::memory:")
            .await
            .expect("connect sqlite memory db")
    }

    async fn create_migration_probe_table(db: &sea_orm::DatabaseConnection) {
        db.execute(Statement::from_string(
            db.get_database_backend(),
            "CREATE TABLE migration_probe (value TEXT NOT NULL)".to_string(),
        ))
        .await
        .expect("create migration probe table");
    }

    async fn migration_probe_count(db: &sea_orm::DatabaseConnection) -> i64 {
        db.query_one(Statement::from_string(
            db.get_database_backend(),
            "SELECT COUNT(*) AS count FROM migration_probe".to_string(),
        ))
        .await
        .expect("query migration probe count")
        .expect("migration probe count row")
        .try_get::<i64>("", "count")
        .expect("read migration probe count")
    }

    async fn live_revision(db: &sea_orm::DatabaseConnection) -> Option<String> {
        db.query_one(Statement::from_string(
            db.get_database_backend(),
            "SELECT version_num FROM alembic_version LIMIT 1".to_string(),
        ))
        .await
        .expect("query live revision")
        .map(|row| {
            row.try_get::<String>("", "version_num")
                .expect("read live revision")
        })
    }

    async fn create_alembic_version_table(db: &sea_orm::DatabaseConnection) {
        db.execute(Statement::from_string(
            db.get_database_backend(),
            "CREATE TABLE alembic_version (version_num VARCHAR(64) NOT NULL)".to_string(),
        ))
        .await
        .expect("create alembic_version table");
    }

    async fn insert_alembic_version(db: &sea_orm::DatabaseConnection, version_num: &str) {
        db.execute(Statement::from_sql_and_values(
            db.get_database_backend(),
            "INSERT INTO alembic_version (version_num) VALUES (?)",
            [version_num.into()],
        ))
        .await
        .expect("insert alembic version");
    }

    fn test_single_flight_config(slug: &str) -> MigrationSingleFlightConfig {
        let mut lock_file_path = std::env::temp_dir();
        lock_file_path.push(format!("mumunovel-{slug}.lock"));
        let _ = std::fs::remove_file(&lock_file_path);

        MigrationSingleFlightConfig {
            lock_name: MIGRATION_LOCK_NAME.to_string(),
            timeout_seconds: 0,
            poll_interval_millis: 1,
            lock_file_path: lock_file_path.to_string_lossy().to_string(),
        }
    }
}
