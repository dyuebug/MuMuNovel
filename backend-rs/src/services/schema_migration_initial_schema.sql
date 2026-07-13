BEGIN;

CREATE TABLE alembic_version (
    version_num VARCHAR(64) NOT NULL, 
    CONSTRAINT alembic_version_pkc PRIMARY KEY (version_num)
);

-- Running upgrade  -> ee0a189f1532

CREATE TABLE batch_generation_tasks (
    id VARCHAR(36) NOT NULL, 
    project_id VARCHAR(36) NOT NULL, 
    user_id VARCHAR(100) NOT NULL, 
    start_chapter_number INTEGER NOT NULL, 
    chapter_count INTEGER NOT NULL, 
    chapter_ids JSON NOT NULL, 
    style_id INTEGER, 
    target_word_count INTEGER, 
    enable_analysis BOOLEAN, 
    status VARCHAR(20), 
    total_chapters INTEGER, 
    completed_chapters INTEGER, 
    failed_chapters JSON, 
    current_chapter_id VARCHAR(36), 
    current_chapter_number INTEGER, 
    current_retry_count INTEGER, 
    max_retries INTEGER, 
    created_at TIMESTAMP WITHOUT TIME ZONE DEFAULT now(), 
    started_at TIMESTAMP WITHOUT TIME ZONE, 
    completed_at TIMESTAMP WITHOUT TIME ZONE, 
    error_message VARCHAR(500), 
    PRIMARY KEY (id)
);

COMMENT ON COLUMN batch_generation_tasks.project_id IS '项目ID';

COMMENT ON COLUMN batch_generation_tasks.user_id IS '用户ID';

COMMENT ON COLUMN batch_generation_tasks.start_chapter_number IS '起始章节序号';

COMMENT ON COLUMN batch_generation_tasks.chapter_count IS '生成章节数量';

COMMENT ON COLUMN batch_generation_tasks.chapter_ids IS '待生成的章节ID列表';

COMMENT ON COLUMN batch_generation_tasks.style_id IS '使用的写作风格ID';

COMMENT ON COLUMN batch_generation_tasks.target_word_count IS '目标字数';

COMMENT ON COLUMN batch_generation_tasks.enable_analysis IS '是否启用同步分析';

COMMENT ON COLUMN batch_generation_tasks.status IS '任务状态: pending/running/completed/failed/cancelled';

COMMENT ON COLUMN batch_generation_tasks.total_chapters IS '总章节数';

COMMENT ON COLUMN batch_generation_tasks.completed_chapters IS '已完成章节数';

COMMENT ON COLUMN batch_generation_tasks.failed_chapters IS '失败的章节信息列表';

COMMENT ON COLUMN batch_generation_tasks.current_chapter_id IS '当前正在生成的章节ID';

COMMENT ON COLUMN batch_generation_tasks.current_chapter_number IS '当前正在生成的章节序号';

COMMENT ON COLUMN batch_generation_tasks.current_retry_count IS '当前章节重试次数';

COMMENT ON COLUMN batch_generation_tasks.max_retries IS '最大重试次数';

COMMENT ON COLUMN batch_generation_tasks.created_at IS '创建时间';

COMMENT ON COLUMN batch_generation_tasks.started_at IS '开始时间';

COMMENT ON COLUMN batch_generation_tasks.completed_at IS '完成时间';

COMMENT ON COLUMN batch_generation_tasks.error_message IS '错误信息';

CREATE TABLE mcp_plugins (
    id VARCHAR(36) NOT NULL, 
    user_id VARCHAR(50) NOT NULL, 
    plugin_name VARCHAR(100) NOT NULL, 
    display_name VARCHAR(200) NOT NULL, 
    description TEXT, 
    plugin_type VARCHAR(50), 
    server_url VARCHAR(500), 
    command VARCHAR(500), 
    args JSON, 
    env JSON, 
    headers JSON, 
    config JSON, 
    tools JSON, 
    enabled BOOLEAN, 
    status VARCHAR(50), 
    last_error TEXT, 
    last_test_at TIMESTAMP WITHOUT TIME ZONE, 
    category VARCHAR(100), 
    sort_order INTEGER, 
    created_at TIMESTAMP WITHOUT TIME ZONE DEFAULT now(), 
    updated_at TIMESTAMP WITHOUT TIME ZONE DEFAULT now(), 
    PRIMARY KEY (id)
);

COMMENT ON COLUMN mcp_plugins.user_id IS '用户ID';

COMMENT ON COLUMN mcp_plugins.plugin_name IS '插件名称（唯一标识）';

COMMENT ON COLUMN mcp_plugins.display_name IS '显示名称';

COMMENT ON COLUMN mcp_plugins.description IS '插件描述';

COMMENT ON COLUMN mcp_plugins.plugin_type IS '插件类型：http/stdio';

COMMENT ON COLUMN mcp_plugins.server_url IS '服务器URL（HTTP类型）';

COMMENT ON COLUMN mcp_plugins.command IS '启动命令（stdio类型）';

COMMENT ON COLUMN mcp_plugins.args IS '命令参数（stdio类型）';

COMMENT ON COLUMN mcp_plugins.env IS '环境变量';

COMMENT ON COLUMN mcp_plugins.headers IS 'HTTP请求头';

COMMENT ON COLUMN mcp_plugins.config IS '插件特定配置（JSON）';

COMMENT ON COLUMN mcp_plugins.tools IS '提供的工具列表';

COMMENT ON COLUMN mcp_plugins.enabled IS '是否启用';

COMMENT ON COLUMN mcp_plugins.status IS '状态：active/inactive/error';

COMMENT ON COLUMN mcp_plugins.last_error IS '最后错误信息';

COMMENT ON COLUMN mcp_plugins.last_test_at IS '最后测试时间';

COMMENT ON COLUMN mcp_plugins.category IS '分类';

COMMENT ON COLUMN mcp_plugins.sort_order IS '排序顺序';

COMMENT ON COLUMN mcp_plugins.created_at IS '创建时间';

COMMENT ON COLUMN mcp_plugins.updated_at IS '更新时间';

CREATE INDEX idx_user_enabled ON mcp_plugins (user_id, enabled);

CREATE UNIQUE INDEX idx_user_plugin ON mcp_plugins (user_id, plugin_name);

CREATE INDEX ix_mcp_plugins_user_id ON mcp_plugins (user_id);

CREATE TABLE projects (
    id VARCHAR(36) NOT NULL, 
    user_id VARCHAR(100) NOT NULL, 
    title VARCHAR(200) NOT NULL, 
    description TEXT, 
    theme TEXT, 
    genre VARCHAR(50), 
    target_words INTEGER, 
    current_words INTEGER, 
    status VARCHAR(20), 
    wizard_status VARCHAR(20), 
    wizard_step INTEGER, 
    outline_mode VARCHAR(20) NOT NULL, 
    world_time_period TEXT, 
    world_location TEXT, 
    world_atmosphere TEXT, 
    world_rules TEXT, 
    chapter_count INTEGER, 
    narrative_perspective VARCHAR(50), 
    character_count INTEGER, 
    created_at TIMESTAMP WITHOUT TIME ZONE DEFAULT now(), 
    updated_at TIMESTAMP WITHOUT TIME ZONE DEFAULT now(), 
    PRIMARY KEY (id), 
    CONSTRAINT check_outline_mode CHECK (outline_mode IN ('one-to-one', 'one-to-many'))
);

COMMENT ON COLUMN projects.user_id IS '用户ID';

COMMENT ON COLUMN projects.title IS '项目标题';

COMMENT ON COLUMN projects.description IS '项目简介';

COMMENT ON COLUMN projects.theme IS '主题';

COMMENT ON COLUMN projects.genre IS '小说类型';

COMMENT ON COLUMN projects.target_words IS '目标字数';

COMMENT ON COLUMN projects.current_words IS '当前字数';

COMMENT ON COLUMN projects.status IS '创作状态';

COMMENT ON COLUMN projects.wizard_status IS '向导完成状态: incomplete/completed';

COMMENT ON COLUMN projects.wizard_step IS '向导当前步骤: 0-4';

COMMENT ON COLUMN projects.outline_mode IS '大纲章节模式: one-to-one(传统模式) 或 one-to-many(细化模式)';

COMMENT ON COLUMN projects.world_time_period IS '时间背景';

COMMENT ON COLUMN projects.world_location IS '地理位置';

COMMENT ON COLUMN projects.world_atmosphere IS '氛围基调';

COMMENT ON COLUMN projects.world_rules IS '世界规则';

COMMENT ON COLUMN projects.chapter_count IS '章节数量';

COMMENT ON COLUMN projects.narrative_perspective IS '叙事视角：first_person/third_person/omniscient';

COMMENT ON COLUMN projects.character_count IS '角色数量';

COMMENT ON COLUMN projects.created_at IS '创建时间';

COMMENT ON COLUMN projects.updated_at IS '更新时间';

CREATE INDEX ix_projects_user_id ON projects (user_id);

CREATE TABLE prompt_templates (
    id VARCHAR(36) NOT NULL, 
    user_id VARCHAR(50) NOT NULL, 
    template_key VARCHAR(100) NOT NULL, 
    template_name VARCHAR(200) NOT NULL, 
    template_content TEXT NOT NULL, 
    description TEXT, 
    category VARCHAR(50), 
    parameters TEXT, 
    is_active BOOLEAN, 
    is_system_default BOOLEAN, 
    created_at TIMESTAMP WITHOUT TIME ZONE DEFAULT now(), 
    updated_at TIMESTAMP WITHOUT TIME ZONE DEFAULT now(), 
    PRIMARY KEY (id)
);

COMMENT ON COLUMN prompt_templates.user_id IS '用户ID';

COMMENT ON COLUMN prompt_templates.template_key IS '模板键名';

COMMENT ON COLUMN prompt_templates.template_name IS '模板显示名称';

COMMENT ON COLUMN prompt_templates.template_content IS '模板内容';

COMMENT ON COLUMN prompt_templates.description IS '模板描述';

COMMENT ON COLUMN prompt_templates.category IS '模板分类';

COMMENT ON COLUMN prompt_templates.parameters IS '模板参数定义(JSON)';

COMMENT ON COLUMN prompt_templates.is_active IS '是否启用';

COMMENT ON COLUMN prompt_templates.is_system_default IS '是否为系统默认模板';

COMMENT ON COLUMN prompt_templates.created_at IS '创建时间';

COMMENT ON COLUMN prompt_templates.updated_at IS '更新时间';

CREATE UNIQUE INDEX idx_user_template ON prompt_templates (user_id, template_key);

CREATE INDEX ix_prompt_templates_user_id ON prompt_templates (user_id);

CREATE TABLE relationship_types (
    id SERIAL NOT NULL, 
    name VARCHAR(50) NOT NULL, 
    category VARCHAR(20) NOT NULL, 
    reverse_name VARCHAR(50), 
    intimacy_range VARCHAR(20), 
    icon VARCHAR(50), 
    description TEXT, 
    created_at TIMESTAMP WITHOUT TIME ZONE DEFAULT now(), 
    PRIMARY KEY (id)
);

COMMENT ON COLUMN relationship_types.name IS '关系名称';

COMMENT ON COLUMN relationship_types.category IS '分类：family/social/hostile/professional';

COMMENT ON COLUMN relationship_types.reverse_name IS '反向关系名称';

COMMENT ON COLUMN relationship_types.intimacy_range IS '亲密度范围：high/medium/low';

COMMENT ON COLUMN relationship_types.icon IS '图标标识';

COMMENT ON COLUMN relationship_types.description IS '关系描述';

COMMENT ON COLUMN relationship_types.created_at IS '创建时间';

CREATE INDEX ix_relationship_types_id ON relationship_types (id);

CREATE TABLE settings (
    id VARCHAR(36) NOT NULL, 
    user_id VARCHAR(50) NOT NULL, 
    api_provider VARCHAR(50), 
    api_key VARCHAR(500), 
    api_base_url VARCHAR(500), 
    llm_model VARCHAR(100), 
    temperature FLOAT, 
    max_tokens INTEGER, 
    preferences TEXT, 
    created_at TIMESTAMP WITHOUT TIME ZONE DEFAULT now(), 
    updated_at TIMESTAMP WITHOUT TIME ZONE DEFAULT now(), 
    PRIMARY KEY (id)
);

COMMENT ON COLUMN settings.user_id IS '用户ID';

COMMENT ON COLUMN settings.api_provider IS 'API提供商';

COMMENT ON COLUMN settings.api_key IS 'API密钥';

COMMENT ON COLUMN settings.api_base_url IS '自定义API地址';

COMMENT ON COLUMN settings.llm_model IS '模型名称';

COMMENT ON COLUMN settings.temperature IS '温度参数';

COMMENT ON COLUMN settings.max_tokens IS '最大token数';

COMMENT ON COLUMN settings.preferences IS '其他偏好设置(JSON)';

COMMENT ON COLUMN settings.created_at IS '创建时间';

COMMENT ON COLUMN settings.updated_at IS '更新时间';

CREATE INDEX idx_user_id ON settings (user_id);

CREATE UNIQUE INDEX ix_settings_user_id ON settings (user_id);

CREATE TABLE user_passwords (
    user_id VARCHAR(100) NOT NULL, 
    username VARCHAR(100) NOT NULL, 
    password_hash TEXT NOT NULL,
    has_custom_password BOOLEAN, 
    created_at TIMESTAMP WITH TIME ZONE DEFAULT now(), 
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT now(), 
    PRIMARY KEY (user_id)
);

COMMENT ON COLUMN user_passwords.user_id IS '用户ID';

COMMENT ON COLUMN user_passwords.username IS '用户名';

COMMENT ON COLUMN user_passwords.password_hash IS '密码校验值（Argon2 PHC 或兼容的 legacy SHA256）';

COMMENT ON COLUMN user_passwords.has_custom_password IS '是否为自定义密码';

COMMENT ON COLUMN user_passwords.created_at IS '创建时间';

COMMENT ON COLUMN user_passwords.updated_at IS '更新时间';

CREATE INDEX ix_user_passwords_user_id ON user_passwords (user_id);

CREATE TABLE users (
    user_id VARCHAR(100) NOT NULL, 
    username VARCHAR(100) NOT NULL, 
    display_name VARCHAR(200) NOT NULL, 
    avatar_url VARCHAR(500), 
    trust_level INTEGER, 
    is_admin BOOLEAN, 
    linuxdo_id VARCHAR(100) NOT NULL, 
    created_at TIMESTAMP WITH TIME ZONE DEFAULT now(), 
    last_login TIMESTAMP WITH TIME ZONE DEFAULT now(), 
    PRIMARY KEY (user_id)
);

COMMENT ON COLUMN users.user_id IS '用户ID，格式：linuxdo_{id} 或 local_{id}';

COMMENT ON COLUMN users.username IS '用户名';

COMMENT ON COLUMN users.display_name IS '显示名称';

COMMENT ON COLUMN users.avatar_url IS '头像URL';

COMMENT ON COLUMN users.trust_level IS '信任等级（仅用于显示）';

COMMENT ON COLUMN users.is_admin IS '是否为管理员';

COMMENT ON COLUMN users.linuxdo_id IS 'LinuxDO用户ID或本地用户ID';

COMMENT ON COLUMN users.created_at IS '创建时间';

COMMENT ON COLUMN users.last_login IS '最后登录时间';

CREATE UNIQUE INDEX ix_users_linuxdo_id ON users (linuxdo_id);

CREATE INDEX ix_users_user_id ON users (user_id);

CREATE INDEX ix_users_username ON users (username);

CREATE TABLE careers (
    id VARCHAR(36) NOT NULL, 
    project_id VARCHAR(36) NOT NULL, 
    name VARCHAR(100) NOT NULL, 
    type VARCHAR(20) NOT NULL, 
    description TEXT, 
    category VARCHAR(50), 
    stages TEXT NOT NULL, 
    max_stage INTEGER NOT NULL, 
    requirements TEXT, 
    special_abilities TEXT, 
    worldview_rules TEXT, 
    attribute_bonuses TEXT, 
    source VARCHAR(20), 
    created_at TIMESTAMP WITHOUT TIME ZONE DEFAULT now(), 
    updated_at TIMESTAMP WITHOUT TIME ZONE DEFAULT now(), 
    PRIMARY KEY (id), 
    FOREIGN KEY(project_id) REFERENCES projects (id) ON DELETE CASCADE
);

COMMENT ON COLUMN careers.name IS '职业名称';

COMMENT ON COLUMN careers.type IS '职业类型: main(主职业)/sub(副职业)';

COMMENT ON COLUMN careers.description IS '职业描述';

COMMENT ON COLUMN careers.category IS '职业分类（如：战斗系、生产系、辅助系）';

COMMENT ON COLUMN careers.stages IS '职业阶段列表(JSON): [{level:1, name:'''', description:''''}, ...]';

COMMENT ON COLUMN careers.max_stage IS '最大阶段数';

COMMENT ON COLUMN careers.requirements IS '职业要求/限制';

COMMENT ON COLUMN careers.special_abilities IS '特殊能力描述';

COMMENT ON COLUMN careers.worldview_rules IS '世界观规则关联';

COMMENT ON COLUMN careers.attribute_bonuses IS '属性加成(JSON): {strength: ''+10%'', intelligence: ''+5%''}';

COMMENT ON COLUMN careers.source IS '来源: ai/manual';

COMMENT ON COLUMN careers.created_at IS '创建时间';

COMMENT ON COLUMN careers.updated_at IS '更新时间';

CREATE INDEX idx_project_id ON careers (project_id);

CREATE INDEX idx_type ON careers (type);

CREATE TABLE outlines (
    id VARCHAR(36) NOT NULL, 
    project_id VARCHAR(36) NOT NULL, 
    title VARCHAR(200) NOT NULL, 
    content TEXT, 
    structure TEXT, 
    order_index INTEGER, 
    created_at TIMESTAMP WITHOUT TIME ZONE DEFAULT now(), 
    updated_at TIMESTAMP WITHOUT TIME ZONE DEFAULT now(), 
    PRIMARY KEY (id), 
    FOREIGN KEY(project_id) REFERENCES projects (id) ON DELETE CASCADE
);

COMMENT ON COLUMN outlines.title IS '大纲标题';

COMMENT ON COLUMN outlines.content IS '大纲内容';

COMMENT ON COLUMN outlines.structure IS '结构化大纲数据(JSON)';

COMMENT ON COLUMN outlines.order_index IS '排序序号';

COMMENT ON COLUMN outlines.created_at IS '创建时间';

COMMENT ON COLUMN outlines.updated_at IS '更新时间';

CREATE TABLE writing_styles (
    id SERIAL NOT NULL, 
    user_id VARCHAR(255), 
    name VARCHAR(100) NOT NULL, 
    style_type VARCHAR(50) NOT NULL, 
    preset_id VARCHAR(50), 
    description TEXT, 
    prompt_content TEXT NOT NULL, 
    order_index INTEGER, 
    created_at TIMESTAMP WITHOUT TIME ZONE DEFAULT now(), 
    updated_at TIMESTAMP WITHOUT TIME ZONE DEFAULT now(), 
    PRIMARY KEY (id), 
    FOREIGN KEY(user_id) REFERENCES users (user_id) ON DELETE CASCADE
);

COMMENT ON COLUMN writing_styles.user_id IS '所属用户ID（NULL表示全局预设风格）';

COMMENT ON COLUMN writing_styles.name IS '风格名称';

COMMENT ON COLUMN writing_styles.style_type IS '风格类型：preset/custom';

COMMENT ON COLUMN writing_styles.preset_id IS '预设风格ID：natural/classical/modern等';

COMMENT ON COLUMN writing_styles.description IS '风格描述';

COMMENT ON COLUMN writing_styles.prompt_content IS '风格提示词内容';

COMMENT ON COLUMN writing_styles.order_index IS '排序序号';

COMMENT ON COLUMN writing_styles.created_at IS '创建时间';

COMMENT ON COLUMN writing_styles.updated_at IS '更新时间';

CREATE TABLE chapters (
    id VARCHAR(36) NOT NULL, 
    project_id VARCHAR(36) NOT NULL, 
    chapter_number INTEGER NOT NULL, 
    title VARCHAR(200) NOT NULL, 
    content TEXT, 
    summary TEXT, 
    word_count INTEGER, 
    status VARCHAR(20), 
    outline_id VARCHAR(36), 
    sub_index INTEGER, 
    expansion_plan TEXT, 
    created_at TIMESTAMP WITHOUT TIME ZONE DEFAULT now(), 
    updated_at TIMESTAMP WITHOUT TIME ZONE DEFAULT now(), 
    PRIMARY KEY (id), 
    FOREIGN KEY(outline_id) REFERENCES outlines (id) ON DELETE SET NULL, 
    FOREIGN KEY(project_id) REFERENCES projects (id) ON DELETE CASCADE
);

COMMENT ON COLUMN chapters.chapter_number IS '章节序号';

COMMENT ON COLUMN chapters.title IS '章节标题';

COMMENT ON COLUMN chapters.content IS '章节内容';

COMMENT ON COLUMN chapters.summary IS '章节摘要';

COMMENT ON COLUMN chapters.word_count IS '字数统计';

COMMENT ON COLUMN chapters.status IS '章节状态';

COMMENT ON COLUMN chapters.outline_id IS '关联的大纲ID';

COMMENT ON COLUMN chapters.sub_index IS '大纲下的子章节序号';

COMMENT ON COLUMN chapters.expansion_plan IS '展开规划详情(JSON): 包含key_events, character_focus, emotional_tone等';

COMMENT ON COLUMN chapters.created_at IS '创建时间';

COMMENT ON COLUMN chapters.updated_at IS '更新时间';

CREATE TABLE characters (
    id VARCHAR(36) NOT NULL, 
    project_id VARCHAR(36) NOT NULL, 
    name VARCHAR(100) NOT NULL, 
    age VARCHAR(50), 
    gender VARCHAR(50), 
    is_organization BOOLEAN, 
    role_type VARCHAR(50), 
    personality TEXT, 
    background TEXT, 
    appearance TEXT, 
    relationships TEXT, 
    organization_type VARCHAR(100), 
    organization_purpose VARCHAR(500), 
    organization_members TEXT, 
    main_career_id VARCHAR(36), 
    main_career_stage INTEGER, 
    sub_careers TEXT, 
    avatar_url VARCHAR(500), 
    traits TEXT, 
    created_at TIMESTAMP WITHOUT TIME ZONE DEFAULT now(), 
    updated_at TIMESTAMP WITHOUT TIME ZONE DEFAULT now(), 
    PRIMARY KEY (id), 
    FOREIGN KEY(main_career_id) REFERENCES careers (id) ON DELETE SET NULL, 
    FOREIGN KEY(project_id) REFERENCES projects (id) ON DELETE CASCADE
);

COMMENT ON COLUMN characters.name IS '角色/组织名称';

COMMENT ON COLUMN characters.age IS '年龄';

COMMENT ON COLUMN characters.gender IS '性别';

COMMENT ON COLUMN characters.is_organization IS '是否为组织';

COMMENT ON COLUMN characters.role_type IS '角色类型';

COMMENT ON COLUMN characters.personality IS '性格特点/组织特性';

COMMENT ON COLUMN characters.background IS '背景故事';

COMMENT ON COLUMN characters.appearance IS '外貌描述';

COMMENT ON COLUMN characters.relationships IS '人物关系(JSON)';

COMMENT ON COLUMN characters.organization_type IS '组织类型';

COMMENT ON COLUMN characters.organization_purpose IS '组织目的';

COMMENT ON COLUMN characters.organization_members IS '组织成员(JSON)';

COMMENT ON COLUMN characters.main_career_id IS '主职业ID';

COMMENT ON COLUMN characters.main_career_stage IS '主职业当前阶段';

COMMENT ON COLUMN characters.sub_careers IS '副职业列表(JSON): [{"career_id": "xxx", "stage": 3}, ...]';

COMMENT ON COLUMN characters.avatar_url IS '头像URL';

COMMENT ON COLUMN characters.traits IS '特征标签(JSON)';

COMMENT ON COLUMN characters.created_at IS '创建时间';

COMMENT ON COLUMN characters.updated_at IS '更新时间';

CREATE TABLE project_default_styles (
    id SERIAL NOT NULL, 
    project_id VARCHAR(36) NOT NULL, 
    style_id INTEGER NOT NULL, 
    created_at TIMESTAMP WITHOUT TIME ZONE DEFAULT now(), 
    updated_at TIMESTAMP WITHOUT TIME ZONE DEFAULT now(), 
    PRIMARY KEY (id), 
    FOREIGN KEY(project_id) REFERENCES projects (id) ON DELETE CASCADE, 
    FOREIGN KEY(style_id) REFERENCES writing_styles (id) ON DELETE CASCADE, 
    CONSTRAINT uix_project_default_style UNIQUE (project_id)
);

COMMENT ON COLUMN project_default_styles.project_id IS '项目ID';

COMMENT ON COLUMN project_default_styles.style_id IS '风格ID';

COMMENT ON COLUMN project_default_styles.created_at IS '创建时间';

COMMENT ON COLUMN project_default_styles.updated_at IS '更新时间';

CREATE TABLE analysis_tasks (
    id VARCHAR(36) NOT NULL, 
    chapter_id VARCHAR(36) NOT NULL, 
    user_id VARCHAR(50) NOT NULL, 
    project_id VARCHAR(36) NOT NULL, 
    status VARCHAR(20) NOT NULL, 
    progress INTEGER, 
    error_message TEXT, 
    created_at TIMESTAMP WITHOUT TIME ZONE DEFAULT now(), 
    started_at TIMESTAMP WITHOUT TIME ZONE, 
    completed_at TIMESTAMP WITHOUT TIME ZONE, 
    PRIMARY KEY (id), 
    FOREIGN KEY(chapter_id) REFERENCES chapters (id) ON DELETE CASCADE
);

COMMENT ON COLUMN analysis_tasks.id IS '任务ID';

COMMENT ON COLUMN analysis_tasks.chapter_id IS '章节ID';

COMMENT ON COLUMN analysis_tasks.user_id IS '用户ID';

COMMENT ON COLUMN analysis_tasks.project_id IS '项目ID';

COMMENT ON COLUMN analysis_tasks.status IS '任务状态: pending/running/completed/failed';

COMMENT ON COLUMN analysis_tasks.progress IS '进度 0-100';

COMMENT ON COLUMN analysis_tasks.error_message IS '错误信息';

COMMENT ON COLUMN analysis_tasks.created_at IS '创建时间';

COMMENT ON COLUMN analysis_tasks.started_at IS '开始执行时间';

COMMENT ON COLUMN analysis_tasks.completed_at IS '完成时间';

CREATE INDEX idx_chapter_id_created ON analysis_tasks (chapter_id, created_at);

CREATE INDEX idx_status ON analysis_tasks (status);

CREATE TABLE character_careers (
    id VARCHAR(36) NOT NULL, 
    character_id VARCHAR(36) NOT NULL, 
    career_id VARCHAR(36) NOT NULL, 
    career_type VARCHAR(20) NOT NULL, 
    current_stage INTEGER NOT NULL, 
    stage_progress INTEGER, 
    started_at VARCHAR(100), 
    reached_current_stage_at VARCHAR(100), 
    notes TEXT, 
    created_at TIMESTAMP WITHOUT TIME ZONE DEFAULT now(), 
    updated_at TIMESTAMP WITHOUT TIME ZONE DEFAULT now(), 
    PRIMARY KEY (id), 
    FOREIGN KEY(career_id) REFERENCES careers (id) ON DELETE CASCADE, 
    FOREIGN KEY(character_id) REFERENCES characters (id) ON DELETE CASCADE
);

COMMENT ON COLUMN character_careers.career_type IS 'main(主职业)/sub(副职业)';

COMMENT ON COLUMN character_careers.current_stage IS '当前阶段（对应职业中的数值）';

COMMENT ON COLUMN character_careers.stage_progress IS '阶段内进度（0-100）';

COMMENT ON COLUMN character_careers.started_at IS '开始修炼时间（小说时间线）';

COMMENT ON COLUMN character_careers.reached_current_stage_at IS '到达当前阶段时间';

COMMENT ON COLUMN character_careers.notes IS '备注（如：修炼心得、特殊事件）';

COMMENT ON COLUMN character_careers.created_at IS '创建时间';

COMMENT ON COLUMN character_careers.updated_at IS '更新时间';

CREATE INDEX idx_career_type ON character_careers (career_type);

CREATE UNIQUE INDEX idx_character_career ON character_careers (character_id, career_id);

CREATE INDEX idx_character_id ON character_careers (character_id);

CREATE TABLE character_relationships (
    id VARCHAR(36) NOT NULL, 
    project_id VARCHAR(36) NOT NULL, 
    character_from_id VARCHAR(36) NOT NULL, 
    character_to_id VARCHAR(36) NOT NULL, 
    relationship_type_id INTEGER, 
    relationship_name VARCHAR(100), 
    intimacy_level INTEGER, 
    status VARCHAR(20), 
    description TEXT, 
    started_at VARCHAR(100), 
    ended_at VARCHAR(100), 
    source VARCHAR(20), 
    created_at TIMESTAMP WITHOUT TIME ZONE DEFAULT now(), 
    updated_at TIMESTAMP WITHOUT TIME ZONE DEFAULT now(), 
    PRIMARY KEY (id), 
    FOREIGN KEY(character_from_id) REFERENCES characters (id) ON DELETE CASCADE, 
    FOREIGN KEY(character_to_id) REFERENCES characters (id) ON DELETE CASCADE, 
    FOREIGN KEY(project_id) REFERENCES projects (id) ON DELETE CASCADE, 
    FOREIGN KEY(relationship_type_id) REFERENCES relationship_types (id)
);

COMMENT ON COLUMN character_relationships.id IS '关系ID';

COMMENT ON COLUMN character_relationships.project_id IS '项目ID';

COMMENT ON COLUMN character_relationships.character_from_id IS '角色A的ID';

COMMENT ON COLUMN character_relationships.character_to_id IS '角色B的ID';

COMMENT ON COLUMN character_relationships.relationship_type_id IS '关系类型ID';

COMMENT ON COLUMN character_relationships.relationship_name IS '自定义关系名称';

COMMENT ON COLUMN character_relationships.intimacy_level IS '亲密度：-100到100';

COMMENT ON COLUMN character_relationships.status IS '状态：active/broken/past/complicated';

COMMENT ON COLUMN character_relationships.description IS '关系详细描述';

COMMENT ON COLUMN character_relationships.started_at IS '关系开始时间（故事时间）';

COMMENT ON COLUMN character_relationships.ended_at IS '关系结束时间（故事时间）';

COMMENT ON COLUMN character_relationships.source IS '来源：ai/manual/imported';

COMMENT ON COLUMN character_relationships.created_at IS '创建时间';

COMMENT ON COLUMN character_relationships.updated_at IS '更新时间';

CREATE INDEX ix_character_relationships_character_from_id ON character_relationships (character_from_id);

CREATE INDEX ix_character_relationships_character_to_id ON character_relationships (character_to_id);

CREATE INDEX ix_character_relationships_project_id ON character_relationships (project_id);

CREATE INDEX ix_character_relationships_relationship_type_id ON character_relationships (relationship_type_id);

CREATE TABLE generation_history (
    id VARCHAR(36) NOT NULL, 
    project_id VARCHAR(36) NOT NULL, 
    chapter_id VARCHAR(36), 
    prompt TEXT, 
    generated_content TEXT, 
    model VARCHAR(50), 
    tokens_used INTEGER, 
    generation_time FLOAT, 
    created_at TIMESTAMP WITHOUT TIME ZONE DEFAULT now(), 
    PRIMARY KEY (id), 
    FOREIGN KEY(chapter_id) REFERENCES chapters (id) ON DELETE SET NULL, 
    FOREIGN KEY(project_id) REFERENCES projects (id) ON DELETE CASCADE
);

COMMENT ON COLUMN generation_history.prompt IS '使用的提示词';

COMMENT ON COLUMN generation_history.generated_content IS '生成的内容';

COMMENT ON COLUMN generation_history.model IS '使用的模型';

COMMENT ON COLUMN generation_history.tokens_used IS '消耗的token数';

COMMENT ON COLUMN generation_history.generation_time IS '生成耗时(秒)';

COMMENT ON COLUMN generation_history.created_at IS '创建时间';

CREATE TABLE organizations (
    id VARCHAR(36) NOT NULL, 
    character_id VARCHAR(36) NOT NULL, 
    project_id VARCHAR(36) NOT NULL, 
    parent_org_id VARCHAR(36), 
    level INTEGER, 
    power_level INTEGER, 
    member_count INTEGER, 
    location TEXT, 
    motto VARCHAR(200), 
    color VARCHAR(100), 
    created_at TIMESTAMP WITHOUT TIME ZONE DEFAULT now(), 
    updated_at TIMESTAMP WITHOUT TIME ZONE DEFAULT now(), 
    PRIMARY KEY (id), 
    FOREIGN KEY(character_id) REFERENCES characters (id) ON DELETE CASCADE, 
    FOREIGN KEY(parent_org_id) REFERENCES organizations (id) ON DELETE SET NULL, 
    FOREIGN KEY(project_id) REFERENCES projects (id) ON DELETE CASCADE, 
    UNIQUE (character_id)
);

COMMENT ON COLUMN organizations.id IS '组织ID';

COMMENT ON COLUMN organizations.character_id IS '关联的角色ID';

COMMENT ON COLUMN organizations.project_id IS '项目ID';

COMMENT ON COLUMN organizations.parent_org_id IS '父组织ID';

COMMENT ON COLUMN organizations.level IS '组织层级';

COMMENT ON COLUMN organizations.power_level IS '势力等级：0-100';

COMMENT ON COLUMN organizations.member_count IS '成员数量';

COMMENT ON COLUMN organizations.location IS '所在地';

COMMENT ON COLUMN organizations.motto IS '宗旨/口号';

COMMENT ON COLUMN organizations.color IS '代表颜色';

COMMENT ON COLUMN organizations.created_at IS '创建时间';

COMMENT ON COLUMN organizations.updated_at IS '更新时间';

CREATE INDEX ix_organizations_project_id ON organizations (project_id);

CREATE TABLE plot_analysis (
    id VARCHAR(36) NOT NULL, 
    project_id VARCHAR(36) NOT NULL, 
    chapter_id VARCHAR(36) NOT NULL, 
    plot_stage VARCHAR(50), 
    conflict_level INTEGER, 
    conflict_types JSON, 
    emotional_tone VARCHAR(100), 
    emotional_intensity FLOAT, 
    emotional_curve JSON, 
    hooks JSON, 
    hooks_count INTEGER, 
    hooks_avg_strength FLOAT, 
    foreshadows JSON, 
    foreshadows_planted INTEGER, 
    foreshadows_resolved INTEGER, 
    plot_points JSON, 
    plot_points_count INTEGER, 
    character_states JSON, 
    scenes JSON, 
    pacing VARCHAR(50), 
    overall_quality_score FLOAT, 
    pacing_score FLOAT, 
    engagement_score FLOAT, 
    coherence_score FLOAT, 
    analysis_report TEXT, 
    suggestions JSON, 
    word_count INTEGER, 
    dialogue_ratio FLOAT, 
    description_ratio FLOAT, 
    created_at TIMESTAMP WITHOUT TIME ZONE DEFAULT now(), 
    PRIMARY KEY (id), 
    FOREIGN KEY(chapter_id) REFERENCES chapters (id) ON DELETE CASCADE, 
    FOREIGN KEY(project_id) REFERENCES projects (id) ON DELETE CASCADE
);

COMMENT ON COLUMN plot_analysis.plot_stage IS '剧情阶段: 开端/发展/高潮/结局/过渡';

COMMENT ON COLUMN plot_analysis.conflict_level IS '冲突强度 1-10';

COMMENT ON COLUMN plot_analysis.conflict_types IS '冲突类型列表: [''人与人'', ''人与己'', ''人与环境'']';

COMMENT ON COLUMN plot_analysis.emotional_tone IS '主导情感: 紧张/温馨/悲伤/激昂/平静';

COMMENT ON COLUMN plot_analysis.emotional_intensity IS '情感强度 0.0-1.0';

COMMENT ON COLUMN plot_analysis.emotional_curve IS '情感曲线: {start: 0.3, middle: 0.7, end: 0.5}';

COMMENT ON COLUMN plot_analysis.hooks IS '钩子列表 - 吸引读者的元素: [
        {
            "type": "悬念|情感|冲突|认知",
            "content": "具体内容",
            "strength": 8,
            "position": "开头|中段|结尾"
        }
    ]';

COMMENT ON COLUMN plot_analysis.hooks_count IS '钩子数量';

COMMENT ON COLUMN plot_analysis.hooks_avg_strength IS '钩子平均强度';

COMMENT ON COLUMN plot_analysis.foreshadows IS '伏笔列表: [
        {
            "content": "伏笔内容",
            "type": "planted|resolved",
            "strength": 7,
            "subtlety": 8,
            "reference_chapter": 3
        }
    ]';

COMMENT ON COLUMN plot_analysis.foreshadows_planted IS '本章埋下的伏笔数量';

COMMENT ON COLUMN plot_analysis.foreshadows_resolved IS '本章回收的伏笔数量';

COMMENT ON COLUMN plot_analysis.plot_points IS '情节点列表: [
        {
            "content": "情节点描述",
            "importance": 0.9,
            "type": "revelation|conflict|resolution|transition",
            "impact": "对故事的影响描述"
        }
    ]';

COMMENT ON COLUMN plot_analysis.plot_points_count IS '情节点数量';

COMMENT ON COLUMN plot_analysis.character_states IS '角色状态变化: [
        {
            "character_id": "xxx",
            "character_name": "张三",
            "state_before": "犹豫不决",
            "state_after": "坚定信念",
            "psychological_change": "内心描述",
            "key_event": "触发事件",
            "relationship_changes": {"李四": "关系变化"}
        }
    ]';

COMMENT ON COLUMN plot_analysis.scenes IS '场景列表: [{location: ''地点'', atmosphere: ''氛围'', duration: ''时长''}]';

COMMENT ON COLUMN plot_analysis.pacing IS '节奏: slow|moderate|fast|varied';

COMMENT ON COLUMN plot_analysis.overall_quality_score IS '整体质量评分 0.0-10.0';

COMMENT ON COLUMN plot_analysis.pacing_score IS '节奏评分 0.0-10.0';

COMMENT ON COLUMN plot_analysis.engagement_score IS '吸引力评分 0.0-10.0';

COMMENT ON COLUMN plot_analysis.coherence_score IS '连贯性评分 0.0-10.0';

COMMENT ON COLUMN plot_analysis.analysis_report IS '完整的文字分析报告';

COMMENT ON COLUMN plot_analysis.suggestions IS '改进建议列表: [''建议1'', ''建议2'']';

COMMENT ON COLUMN plot_analysis.word_count IS '章节字数';

COMMENT ON COLUMN plot_analysis.dialogue_ratio IS '对话占比 0.0-1.0';

COMMENT ON COLUMN plot_analysis.description_ratio IS '描写占比 0.0-1.0';

COMMENT ON COLUMN plot_analysis.created_at IS '分析时间';

CREATE UNIQUE INDEX ix_plot_analysis_chapter_id ON plot_analysis (chapter_id);

CREATE INDEX ix_plot_analysis_project_id ON plot_analysis (project_id);

CREATE TABLE regeneration_tasks (
    id VARCHAR(36) NOT NULL, 
    chapter_id VARCHAR(36) NOT NULL, 
    analysis_id VARCHAR(36), 
    user_id VARCHAR(50) NOT NULL, 
    project_id VARCHAR(36) NOT NULL, 
    modification_instructions TEXT NOT NULL, 
    original_suggestions JSON, 
    selected_suggestion_indices JSON, 
    custom_instructions TEXT, 
    style_id INTEGER, 
    target_word_count INTEGER, 
    focus_areas JSON, 
    preserve_elements JSON, 
    status VARCHAR(20), 
    progress INTEGER, 
    error_message TEXT, 
    original_content TEXT, 
    original_word_count INTEGER, 
    regenerated_content TEXT, 
    regenerated_word_count INTEGER, 
    version_number INTEGER, 
    version_note VARCHAR(500), 
    created_at TIMESTAMP WITHOUT TIME ZONE DEFAULT now(), 
    started_at TIMESTAMP WITHOUT TIME ZONE, 
    completed_at TIMESTAMP WITHOUT TIME ZONE, 
    PRIMARY KEY (id), 
    FOREIGN KEY(chapter_id) REFERENCES chapters (id) ON DELETE CASCADE
);

COMMENT ON COLUMN regeneration_tasks.analysis_id IS '关联的分析结果ID';

COMMENT ON COLUMN regeneration_tasks.modification_instructions IS '综合修改指令';

COMMENT ON COLUMN regeneration_tasks.original_suggestions IS '来自分析的原始建议列表';

COMMENT ON COLUMN regeneration_tasks.selected_suggestion_indices IS '用户选择的建议索引';

COMMENT ON COLUMN regeneration_tasks.custom_instructions IS '用户自定义修改意见';

COMMENT ON COLUMN regeneration_tasks.style_id IS '写作风格ID';

COMMENT ON COLUMN regeneration_tasks.target_word_count IS '目标字数';

COMMENT ON COLUMN regeneration_tasks.focus_areas IS '重点优化方向';

COMMENT ON COLUMN regeneration_tasks.preserve_elements IS '需要保留的元素配置';

COMMENT ON COLUMN regeneration_tasks.status IS 'pending/running/completed/failed';

COMMENT ON COLUMN regeneration_tasks.progress IS '进度 0-100';

COMMENT ON COLUMN regeneration_tasks.original_content IS '原始章节内容快照';

COMMENT ON COLUMN regeneration_tasks.original_word_count IS '原始字数';

COMMENT ON COLUMN regeneration_tasks.regenerated_content IS '重新生成的内容';

COMMENT ON COLUMN regeneration_tasks.regenerated_word_count IS '新内容字数';

COMMENT ON COLUMN regeneration_tasks.version_number IS '版本号';

COMMENT ON COLUMN regeneration_tasks.version_note IS '版本说明';

CREATE INDEX ix_regeneration_tasks_chapter_id ON regeneration_tasks (chapter_id);

CREATE INDEX ix_regeneration_tasks_project_id ON regeneration_tasks (project_id);

CREATE INDEX ix_regeneration_tasks_user_id ON regeneration_tasks (user_id);

CREATE TABLE story_memories (
    id VARCHAR(100) NOT NULL, 
    project_id VARCHAR(36) NOT NULL, 
    chapter_id VARCHAR(36), 
    memory_type VARCHAR(50) NOT NULL, 
    title VARCHAR(200), 
    content TEXT NOT NULL, 
    full_context TEXT, 
    related_characters JSON, 
    related_locations JSON, 
    tags JSON, 
    importance_score FLOAT, 
    story_timeline INTEGER NOT NULL, 
    chapter_position INTEGER, 
    text_length INTEGER, 
    is_foreshadow INTEGER, 
    foreshadow_resolved_at VARCHAR(100), 
    foreshadow_strength FLOAT, 
    vector_id VARCHAR(100), 
    embedding_model VARCHAR(100), 
    created_at TIMESTAMP WITHOUT TIME ZONE DEFAULT now(), 
    updated_at TIMESTAMP WITHOUT TIME ZONE DEFAULT now(), 
    PRIMARY KEY (id), 
    FOREIGN KEY(chapter_id) REFERENCES chapters (id) ON DELETE CASCADE, 
    FOREIGN KEY(foreshadow_resolved_at) REFERENCES chapters (id) ON DELETE SET NULL, 
    FOREIGN KEY(project_id) REFERENCES projects (id) ON DELETE CASCADE, 
    UNIQUE (vector_id)
);

COMMENT ON COLUMN story_memories.memory_type IS '
    记忆类型:
    - plot_point: 情节点
    - character_event: 角色事件
    - world_detail: 世界观细节
    - hook: 钩子(悬念/冲突)
    - foreshadow: 伏笔
    - dialogue: 重要对话
    - scene: 场景描写
    ';

COMMENT ON COLUMN story_memories.title IS '记忆标题/简述';

COMMENT ON COLUMN story_memories.content IS '记忆内容摘要(100-500字)';

COMMENT ON COLUMN story_memories.full_context IS '完整上下文(可选,用于详细记录)';

COMMENT ON COLUMN story_memories.related_characters IS '涉及角色ID列表: [''char_id_1'', ''char_id_2'']';

COMMENT ON COLUMN story_memories.related_locations IS '涉及地点列表: [''地点1'', ''地点2'']';

COMMENT ON COLUMN story_memories.tags IS '标签列表: [''悬念'', ''转折'', ''伏笔'', ''高潮'']';

COMMENT ON COLUMN story_memories.importance_score IS '重要性评分 0.0-1.0';

COMMENT ON COLUMN story_memories.story_timeline IS '故事时间线位置(章节序号)';

COMMENT ON COLUMN story_memories.chapter_position IS '章节内位置(字符位置)';

COMMENT ON COLUMN story_memories.text_length IS '文本长度(字符数)';

COMMENT ON COLUMN story_memories.is_foreshadow IS '伏笔状态: 0=普通记忆, 1=已埋下伏笔, 2=伏笔已回收';

COMMENT ON COLUMN story_memories.foreshadow_resolved_at IS '伏笔回收的章节ID';

COMMENT ON COLUMN story_memories.foreshadow_strength IS '伏笔强度 0.0-1.0';

COMMENT ON COLUMN story_memories.vector_id IS '向量数据库中的唯一ID';

COMMENT ON COLUMN story_memories.embedding_model IS '使用的embedding模型';

COMMENT ON COLUMN story_memories.created_at IS '创建时间';

COMMENT ON COLUMN story_memories.updated_at IS '更新时间';

CREATE INDEX ix_story_memories_chapter_id ON story_memories (chapter_id);

CREATE INDEX ix_story_memories_memory_type ON story_memories (memory_type);

CREATE INDEX ix_story_memories_project_id ON story_memories (project_id);

CREATE INDEX ix_story_memories_story_timeline ON story_memories (story_timeline);

CREATE TABLE organization_members (
    id VARCHAR(36) NOT NULL, 
    organization_id VARCHAR(36) NOT NULL, 
    character_id VARCHAR(36) NOT NULL, 
    position VARCHAR(100) NOT NULL, 
    rank INTEGER, 
    status VARCHAR(20), 
    joined_at VARCHAR(100), 
    left_at VARCHAR(100), 
    loyalty INTEGER, 
    contribution INTEGER, 
    source VARCHAR(20), 
    notes TEXT, 
    created_at TIMESTAMP WITHOUT TIME ZONE DEFAULT now(), 
    updated_at TIMESTAMP WITHOUT TIME ZONE DEFAULT now(), 
    PRIMARY KEY (id), 
    FOREIGN KEY(character_id) REFERENCES characters (id) ON DELETE CASCADE, 
    FOREIGN KEY(organization_id) REFERENCES organizations (id) ON DELETE CASCADE
);

COMMENT ON COLUMN organization_members.id IS '成员关系ID';

COMMENT ON COLUMN organization_members.organization_id IS '组织ID';

COMMENT ON COLUMN organization_members.character_id IS '角色ID';

COMMENT ON COLUMN organization_members.position IS '职位名称';

COMMENT ON COLUMN organization_members.rank IS '职位等级';

COMMENT ON COLUMN organization_members.status IS '状态：active/retired/expelled/deceased';

COMMENT ON COLUMN organization_members.joined_at IS '加入时间（故事时间）';

COMMENT ON COLUMN organization_members.left_at IS '离开时间（故事时间）';

COMMENT ON COLUMN organization_members.loyalty IS '忠诚度：0-100';

COMMENT ON COLUMN organization_members.contribution IS '贡献度：0-100';

COMMENT ON COLUMN organization_members.source IS '来源：ai/manual';

COMMENT ON COLUMN organization_members.notes IS '备注';

COMMENT ON COLUMN organization_members.created_at IS '创建时间';

COMMENT ON COLUMN organization_members.updated_at IS '更新时间';

CREATE INDEX ix_organization_members_character_id ON organization_members (character_id);

CREATE INDEX ix_organization_members_organization_id ON organization_members (organization_id);

INSERT INTO alembic_version (version_num) VALUES ('ee0a189f1532') RETURNING alembic_version.version_num;

COMMIT;

