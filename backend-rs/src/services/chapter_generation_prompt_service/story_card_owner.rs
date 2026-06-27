use std::collections::HashSet;

use serde_json::{json, Value};

use super::{
    creative_mode_spec, normalize_creative_mode, normalize_plot_stage, normalize_story_focus,
    plot_stage_label, story_focus_spec,
};

fn dedupe_static_prompt_items(items: Vec<&'static str>) -> Vec<&'static str> {
    let mut seen = HashSet::new();
    let mut result = Vec::new();
    for item in items {
        let text = item.trim();
        if text.is_empty() || !seen.insert(text) {
            continue;
        }
        result.push(text);
    }
    result
}

fn build_chapter_combo_text(
    creative_mode: &str,
    story_focus: &str,
    plot_stage: &str,
    fallback: &str,
) -> String {
    let mut labels = Vec::new();
    if let Some(label) = normalize_creative_mode(creative_mode)
        .and_then(creative_mode_spec)
        .map(|spec| spec.label)
    {
        labels.push(label);
    }
    if let Some(label) = normalize_story_focus(story_focus)
        .and_then(story_focus_spec)
        .map(|spec| spec.label)
    {
        labels.push(label);
    }
    if let Some(label) = normalize_plot_stage(plot_stage).and_then(plot_stage_label) {
        labels.push(label);
    }
    if labels.is_empty() {
        fallback.to_string()
    } else {
        labels.join(" / ")
    }
}

fn normalized_story_runtime_inputs(
    creative_mode: &str,
    story_focus: &str,
    plot_stage: &str,
) -> Option<(
    Option<&'static str>,
    Option<&'static str>,
    Option<&'static str>,
)> {
    let normalized_mode = normalize_creative_mode(creative_mode);
    let normalized_focus = normalize_story_focus(story_focus);
    let normalized_stage = normalize_plot_stage(plot_stage);
    if normalized_mode.is_none() && normalized_focus.is_none() && normalized_stage.is_none() {
        None
    } else {
        Some((normalized_mode, normalized_focus, normalized_stage))
    }
}

pub(crate) fn build_narrative_blueprint_block(
    creative_mode: &str,
    story_focus: &str,
    plot_stage: &str,
) -> String {
    let normalized_mode = normalize_creative_mode(creative_mode);
    let normalized_focus = normalize_story_focus(story_focus);
    let normalized_stage = normalize_plot_stage(plot_stage);

    if normalized_mode.is_none() && normalized_focus.is_none() && normalized_stage.is_none() {
        return String::new();
    }

    let mut priority_beats: Vec<&'static str> = Vec::new();
    let mut priority_risks: Vec<&'static str> = Vec::new();

    match normalized_mode {
        Some("hook") => {
            priority_beats.extend([
                "开场更早抛出异常、危险或未完成目标，先抓住读者注意力。",
                "尾段优先保留信息缺口、危险临门或选择未决，不要平收。",
            ]);
            priority_risks.push("不要只堆钩子和异常，却缺少实质推进。");
        }
        Some("emotion") => {
            priority_beats.extend([
                "关键转折后要写出人物情绪余震和关系反应，不只交代结果。",
                "让动作、停顿和对白共同承载情绪，而不是全靠抒情说明。",
            ]);
            priority_risks.push("不要让情绪独自悬空，必须落回选择与后果。");
        }
        Some("suspense") => {
            priority_beats.extend([
                "中前段持续制造信息差、误判或证据变化，让压力逐步抬升。",
                "每个阶段都给出一点新认知，但不要一次讲透底牌。",
            ]);
            priority_risks.push("避免把悬念写成纯遮掩，读者需要看到有效推进。");
        }
        Some("relationship") => {
            priority_beats.extend([
                "把关键冲突尽量落在人与人之间的立场差、亏欠感或试探上。",
                "安排一次关系位移，让后续行动因为关系变化而改道。",
            ]);
            priority_risks.push("不要只有关系情绪，没有行动层面的后续影响。");
        }
        Some("payoff") => {
            priority_beats.extend([
                "优先安排前文铺垫兑现、收获反馈或阶段性反转，给读者明确回报。",
                "兑现后顺手打开下一轮更大的目标或麻烦，不把气口写死。",
            ]);
            priority_risks.push("不要只顾爽点回收，忽略代价与后续空间。");
        }
        Some("balanced") => {
            priority_beats.push("推进、情绪、信息释放和回报要彼此穿插，不让单一节拍统治全文。");
        }
        _ => {}
    }

    match normalized_focus {
        Some("advance_plot") => {
            priority_beats.push("每个关键段都要写出行动结果和局势变化，避免原地解释。");
            priority_risks.push("避免设定说明和情绪回旋挤压主线推进。");
        }
        Some("deepen_character") => {
            priority_beats.push("至少安排一次能暴露人物弱点、执念或价值判断的选择。");
            priority_risks.push("不要把人物塑形写成静态介绍，必须落到行为上。");
        }
        Some("escalate_conflict") => {
            priority_beats.push("让阻力、代价和对立面逐段变强，形成持续抬压链条。");
            priority_risks.push("避免重复同级冲突，读者会觉得原地踏步。");
        }
        Some("reveal_mystery") => {
            priority_beats.push("优先安排线索出现、误导修正和认知刷新，至少推进一点真相。");
            priority_risks.push("不要把揭示写成解释堆叠，尽量通过事件和证据推进。");
        }
        Some("relationship_shift") => {
            priority_beats.push("对话、动作和站队变化都要服务关系转折，而不只是口头表态。");
            priority_risks.push("不要让关系变化只停留在情绪层，没有后续选择代价。");
        }
        Some("foreshadow_payoff") => {
            priority_beats.push("回收时既要兑现前文承诺，也要带出新的悬念或任务。");
            priority_risks.push("避免只用说明句回收伏笔，最好落在事件结果上。");
        }
        _ => {}
    }

    match normalized_stage {
        Some("development") => {
            priority_beats.push("当前阶段优先扩张局势、铺开变量，并把选择成本逐章抬高。");
            priority_risks.push("避免太早交底或提前透支高潮。");
        }
        Some("climax") => {
            priority_beats.push("当前阶段要让核心矛盾正面碰撞，把选择逼到无法拖延的节点。");
            priority_risks.push("避免高潮只有声量，没有清晰结果与代价。");
        }
        Some("ending") => {
            priority_beats.push("当前阶段要优先收束主承诺、主悬念和关键关系线，再留余味。");
            priority_risks.push("避免只顾收尾，忘了兑现前文最重要的铺垫。");
        }
        _ => {}
    }

    let mut beat_candidates = priority_beats;
    beat_candidates.extend([
        "开场尽快抛出异常、目标或受阻点，不做平铺导入。",
        "中段用连续动作推进局势，并让阻力或代价升级。",
        "后段安排一次局势改写、信息刷新或关系位移。",
        "结尾保留明确追读牵引，不要平收。",
    ]);
    let beats = dedupe_static_prompt_items(beat_candidates);

    let mut risk_candidates = priority_risks;
    risk_candidates.push("不要把节拍写成说明书，关键节点都要有动作和即时结果。");
    let risks = dedupe_static_prompt_items(risk_candidates);

    let mut lines = vec![format!(
        "【结构蓝图】本轮按“{}”组织章节节拍",
        build_chapter_combo_text(creative_mode, story_focus, plot_stage, "默认结构")
    )];
    lines.extend(beats.into_iter().take(4).map(|item| format!("- {}", item)));
    if let Some(risk) = risks.first() {
        lines.push(format!("- 重点避免：{}", risk));
    }
    format!("{}\n", lines.join("\n"))
}

pub(crate) fn build_story_objective_card_block(
    creative_mode: &str,
    story_focus: &str,
    plot_stage: &str,
) -> String {
    let Some((normalized_mode, normalized_focus, normalized_stage)) =
        normalized_story_runtime_inputs(creative_mode, story_focus, plot_stage)
    else {
        return String::new();
    };

    let mut objective = "让本章推动一个看得见的目标，不写空转段落。";
    let mut obstacle = "安排一次明确受阻、代价上升或信息错位。";
    let mut turn = "在中后段安排一次认知或局面改写。";
    let mut hook = "章尾留下追读牵引，不平收。";

    match normalized_mode {
        Some("hook") => {
            hook = "把钩子放在异常、危险或未决选择上，尽量做到前段抓人、尾段牵引。";
            turn = "转折优先用信息缺口扩大、危险临门或局势突然偏转来触发。";
        }
        Some("emotion") => {
            objective = "让本章既推进事件，也逼出人物情绪与关系反应。";
            turn = "转折优先落在情绪反噬、误伤、和解受阻或认知偏移上。";
            hook = "钩子留在情绪未落地、关系未说破或选择仍有余震处。";
        }
        Some("suspense") => {
            obstacle = "阻力优先来自信息差、误判、证据反噬或真相未全。";
            turn = "转折通过线索翻面、认知刷新、身份异动或危险升级完成。";
            hook = "钩子留在新疑点、半揭开的答案或更近一步的危险上。";
        }
        Some("relationship") => {
            objective = "让本章推动一次明确的关系位移，而不只是情绪点缀。";
            obstacle = "阻力来自立场差、亏欠、信任裂缝或试探失手。";
            turn = "转折优先用关系破裂、突然靠近、站队变化或误会反转来完成。";
            hook = "钩子留在关系未定、话没说透、立场悬空的地方。";
        }
        Some("payoff") => {
            objective = "让本章承担一次明确兑现，让读者感到回报落地。";
            turn = "转折优先让兑现带出更大代价、更高目标或新的麻烦。";
            hook = "钩子放在回报之后的新失衡上，而不是只停在爽点本身。";
        }
        _ => {}
    }

    match normalized_focus {
        Some("advance_plot") => {
            objective = "核心目标是把局势往前推一格，至少形成新的行动结果。";
        }
        Some("deepen_character") => {
            objective = "核心目标是让角色在选择里显形，暴露弱点、执念或价值判断。";
        }
        Some("escalate_conflict") => {
            obstacle = "阻力必须逐层变强，让代价和对立面都更具体。";
        }
        Some("reveal_mystery") => {
            turn = "转折优先通过线索出现、误导修正和认知刷新来完成。";
        }
        Some("relationship_shift") => {
            turn = "转折必须带来关系位移、立场重排或信任结构变化。";
        }
        Some("foreshadow_payoff") => {
            objective = "核心目标是兑现前文埋设，并顺手打开新的后续空间。";
            hook = "钩子留在兑现后的新承诺、新麻烦或更大代价上。";
        }
        _ => {}
    }

    match normalized_stage {
        Some("development") => {
            objective = "当前阶段先把局势和眼前目标推到更难的位置。";
        }
        Some("climax") => {
            obstacle = "阻力要逼近正面碰撞，选择代价必须明显抬高。";
            turn = "转折要接近核心碰撞点，不能只是小波动。";
        }
        Some("ending") => {
            objective = "当前阶段让本章承担主承诺或关键关系线的回收职责。";
            hook = "钩子更适合留余味、次级悬念或收束后的新失衡，不能抢走主收束。";
        }
        _ => {}
    }

    let combo_text = build_chapter_combo_text(creative_mode, story_focus, plot_stage, "默认任务");
    format!(
        "【章节目标卡】本轮按“{}”优先落实以下叙事任务\n- 目标：{}\n- 阻力：{}\n- 转折：{}\n- 钩子：{}\n",
        combo_text, objective, obstacle, turn, hook
    )
}

pub(crate) fn build_story_result_card_block(
    creative_mode: &str,
    story_focus: &str,
    plot_stage: &str,
) -> String {
    let Some((normalized_mode, normalized_focus, normalized_stage)) =
        normalized_story_runtime_inputs(creative_mode, story_focus, plot_stage)
    else {
        return String::new();
    };

    let mut progress = "这一章结束后，局势应明确前移，人物不能还停在原地。";
    let mut reveal = "至少交付一个新认知、新线索或一次有效兑现。";
    let mut relationship = "至少有一条人物关系线出现可见变化，而不是只说情绪。";
    let mut fallout = "章尾要留下一个会逼出下章动作的余波，而不是平稳收住。";

    match normalized_mode {
        Some("hook") => {
            progress = "本章结束后，局势必须被推到一个不继续看就会难受的节点。";
            fallout = "余波优先落在未决选择、临门危险或刚被挑开的异常上。";
        }
        Some("emotion") => {
            reveal = "结果里要能看到情绪代价、误伤、和解受阻或内心认知变化。";
            relationship = "关系结果要落到互动后果上，让人物之后的做法因此改变。";
        }
        Some("suspense") => {
            reveal = "至少留下一个更接近真相的新证据，同时制造新的误判空间。";
            fallout = "余波留在新疑点、身份异动或危险升级上，不能只剩空白遮掩。";
        }
        Some("relationship") => {
            relationship = "结果里必须出现一次明确的关系位移、立场变化或信任重排。";
            fallout = "余波最好落在关系未定、话未说透或站队未稳上。";
        }
        Some("payoff") => {
            reveal = "结果要让读者看到铺垫兑现、回报落地，并感到不是白等。";
            progress = "兑现之后，局势要被顺势推向更高目标或更大麻烦。";
        }
        _ => {}
    }

    match normalized_focus {
        Some("advance_plot") => {
            progress = "推进结果必须清晰可见：行动产生了后果，局势换了位置。";
        }
        Some("deepen_character") => {
            reveal = "结果要让人物的弱点、执念或价值判断真正显形，而非停在说明。";
            relationship = "人物变化要影响他与他人的互动方式或后续选择。";
        }
        Some("escalate_conflict") => {
            progress = "推进结果不是前进一步，而是把人推入更高代价的冲突区。";
            fallout = "余波要把冲突继续抬高，让下一轮没有轻松退路。";
        }
        Some("reveal_mystery") => {
            reveal = "揭示结果必须真实推进谜团，不只是制造更多模糊表述。";
        }
        Some("relationship_shift") => {
            relationship = "关系结果必须足够明确，能改变两人之后的说话方式、站位或合作条件。";
        }
        Some("foreshadow_payoff") => {
            reveal = "结果要让前文埋设获得兑现，同时打开新的后续空间。";
            fallout = "余波放在兑现后的新承诺、新代价或更大失衡上。";
        }
        _ => {}
    }

    match normalized_stage {
        Some("development") => {
            progress = "这一章结束后，故事要进入一个更难但更清晰的推进区。";
            fallout = "余波要把后续任务钉住，让读者知道下一章不是重复上一章。";
        }
        Some("climax") => {
            progress = "推进结果要逼近或触发正面碰撞，不能只是外围晃动。";
            reveal = "揭示结果要掀开关键底牌、核心真相或决定性误判。";
        }
        Some("ending") => {
            reveal = "揭示结果优先服务主承诺、主悬念与关键伏笔的回收。";
            relationship = "关系结果要体现收束、定局或带余温的最终位移。";
            fallout = "余波更适合留余味、后效和新失衡，不能抢走主收束。";
        }
        _ => {}
    }

    let combo_text = build_chapter_combo_text(creative_mode, story_focus, plot_stage, "默认结果");
    format!(
        "【章节结果卡】本轮写完后，至少让读者感知到以下结果变化（{}）\n- 推进：{}\n- 揭示：{}\n- 关系：{}\n- 余波：{}\n",
        combo_text, progress, reveal, relationship, fallout
    )
}

pub(crate) fn build_story_payoff_chain_card_block(
    creative_mode: &str,
    story_focus: &str,
    plot_stage: &str,
) -> String {
    let Some((normalized_mode, normalized_focus, normalized_stage)) =
        normalized_story_runtime_inputs(creative_mode, story_focus, plot_stage)
    else {
        return String::new();
    };

    let mut seed_point = "本章最好承接一个前文钩点，或提前挂出一个本章内/近章可回收的小铺垫。";
    let mut payoff_style = "回收要通过事件结果、人物反应或局势变化落地，不要只用解释句交代。";
    let mut after_payoff = "回收后顺手打开一个新的后续问题、代价或更高要求，形成继续推进。";
    let mut avoid_line = "不要为了制造“回收感”硬塞解释，真正的回报来自变化本身。";

    match normalized_mode {
        Some("hook") => {
            seed_point = "钩点最好和异常、风险或未完成目标绑定，方便本章快速形成追读回报。";
            after_payoff = "回收后最好立刻暴露更近一步的危险或未决选择。";
        }
        Some("emotion") => {
            payoff_style = "回收最好通过情绪兑现、关系反噬、迟来理解或误伤后果来落地。";
            after_payoff = "兑现后保留情绪余震和关系后效，而不是当场全部说透。";
        }
        Some("suspense") => {
            payoff_style = "回收最好通过线索翻面、误导修正、身份异常或答案半揭开的方式落地。";
            avoid_line = "不要把悬念回收写成一段纯说明，事件和证据要一起动。";
        }
        Some("relationship") => {
            payoff_style = "回收最好落在关系位置改变、站队重排或承诺兑现/破裂上。";
            after_payoff = "关系兑现后要带出新的靠近、裂缝或共同代价。";
        }
        Some("payoff") => {
            seed_point = "优先挑最值钱、读者最能感知的铺垫做兑现，不要把回报浪费在边角细节。";
            payoff_style = "兑现要让读者感到“终于等到”，最好有现场感和后果。";
        }
        _ => {}
    }

    match normalized_focus {
        Some("advance_plot") => {
            after_payoff = "回收后要把主线任务推到下一个更具体的推进位。";
        }
        Some("deepen_character") => {
            payoff_style = "回收最好顺带暴露人物真正想要什么、怕什么或会为什么付代价。";
        }
        Some("escalate_conflict") => {
            after_payoff = "兑现之后最好让冲突更难、更贵或更无法回头。";
        }
        Some("reveal_mystery") => {
            payoff_style = "回收最好服务谜团推进：解掉一层，同时抛出更深一层。";
        }
        Some("relationship_shift") => {
            payoff_style = "回收最好直接改写关系位置，让之前的铺垫变成站队或亲疏结果。";
        }
        Some("foreshadow_payoff") => {
            seed_point = "本轮优先选那些已经被反复提及、读者真正会记得的伏笔来兑现。";
        }
        _ => {}
    }

    match normalized_stage {
        Some("development") => {
            after_payoff = "发展阶段的回收更适合边兑现边扩张变量，而不是一次封口。";
        }
        Some("climax") => {
            seed_point = "高潮阶段优先回收最值钱的承诺和冲突，不要把主回报留到场外说明。";
            payoff_style = "高潮里的兑现必须有冲击、有代价、有局势改写。";
        }
        Some("ending") => {
            payoff_style = "结局阶段的回收要优先服务主承诺、主悬念和关键关系线。";
            after_payoff = "收束后可留余波，但不要用新主坑稀释已经完成的回收。";
        }
        _ => {}
    }

    let combo_text = build_chapter_combo_text(creative_mode, story_focus, plot_stage, "默认回收");
    format!(
        "【章节爽点回收卡】本轮尽量让回报链条可感知地落地（{}）\n- 铺垫挂点：{}\n- 回收方式：{}\n- 回收后续：{}\n- 避免：{}\n",
        combo_text, seed_point, payoff_style, after_payoff, avoid_line
    )
}

pub(crate) fn build_story_rule_grounding_card_block(
    creative_mode: &str,
    story_focus: &str,
    plot_stage: &str,
) -> String {
    let Some((normalized_mode, normalized_focus, normalized_stage)) =
        normalized_story_runtime_inputs(creative_mode, story_focus, plot_stage)
    else {
        return String::new();
    };

    let mut rule_anchor = "关键设定请通过动作、代价、限制和现场反馈落地，不要只在解释里出现。";
    let mut world_feedback = "让环境、秩序、资源或规则对人物行动产生真实反馈。";
    let mut reader_clarity = "读者应能在场景中自然理解设定怎样影响选择，而不是靠额外讲解补课。";
    let mut avoid_line = "不要把设定当百科说明插入正文，尤其不要在高压段落中长篇停讲。";

    match normalized_mode {
        Some("hook") => {
            rule_anchor = "设定落地优先服务异常和危险，让读者尽快知道“为什么这件事现在会出问题”。";
        }
        Some("emotion") => {
            world_feedback = "设定反馈最好还能压到人物情绪和关系上，让环境规则参与情绪代价。";
        }
        Some("suspense") => {
            reader_clarity = "设定落地优先通过异常细节、反常结果和不合常识的代价来提示。";
            avoid_line = "不要把谜团设定一口气讲透，先给读者能用的现场规则。";
        }
        Some("relationship") => {
            world_feedback = "设定反馈最好能影响阵营、身份边界、亲疏秩序或合作成本。";
        }
        Some("payoff") => {
            rule_anchor = "优先让曾铺垫过的设定规则在这章兑现作用，而不是只作为背景装饰。";
        }
        _ => {}
    }

    match normalized_focus {
        Some("advance_plot") => {
            rule_anchor = "设定落地要直接推动行动选择，而不是独立悬浮在剧情外。";
        }
        Some("deepen_character") => {
            world_feedback = "设定最好能逼人物做选择，让读者看到他怎样与世界规则发生摩擦。";
        }
        Some("escalate_conflict") => {
            world_feedback = "规则的代价、限制或反噬要把冲突抬高，而不是口头上更难。";
        }
        Some("reveal_mystery") => {
            reader_clarity = "设定落地最好通过一两个关键异常反馈把谜团范围缩小。";
        }
        Some("relationship_shift") => {
            world_feedback = "设定反馈最好连带改写人物之间的关系位置和说话底气。";
        }
        Some("foreshadow_payoff") => {
            rule_anchor = "优先回收曾提过的设定限制、规则漏洞或资源门槛。";
        }
        _ => {}
    }

    match normalized_stage {
        Some("development") => {
            reader_clarity = "发展阶段要把重要规则讲到够用、够推事，但别一次讲完。";
        }
        Some("climax") => {
            rule_anchor = "高潮阶段的设定落地要直接参与碰撞和结果，不要退回事后讲解。";
            avoid_line = "不要在高潮关键碰撞前后连续长讲设定";
        }
        Some("ending") => {
            rule_anchor = "结局阶段优先回收最关键的规则承诺、世界后果和秩序代价。";
        }
        _ => {}
    }

    let combo_text =
        build_chapter_combo_text(creative_mode, story_focus, plot_stage, "默认设定落地");
    format!(
        "【章节设定落地卡】请把设定真实压进章节推进里（{}）\n- 规则锚点：{}\n- 世界反馈：{}\n- 读者可感知性：{}\n- 避免：{}\n",
        combo_text, rule_anchor, world_feedback, reader_clarity, avoid_line
    )
}

pub(crate) fn build_story_information_release_card_block(
    creative_mode: &str,
    story_focus: &str,
    plot_stage: &str,
) -> String {
    let Some((normalized_mode, normalized_focus, normalized_stage)) =
        normalized_story_runtime_inputs(creative_mode, story_focus, plot_stage)
    else {
        return String::new();
    };

    let mut reveal_timing = "每个关键段尽量给一点新信息，但不要一次性倾倒全部答案。";
    let mut reveal_form = "信息最好通过动作、证据、对白、环境反馈或结果变化释放。";
    let mut conceal_line = "能暂时不讲透的部分先留白，只保证读者此刻拥有足够理解推进的信息。";
    let mut avoid_line = "不要在人物最该行动时突然进入整段解释模式。";

    match normalized_mode {
        Some("hook") => {
            reveal_timing = "前段尽快丢出一块能抓住人的异常信息，中后段再补关键解释。";
        }
        Some("emotion") => {
            reveal_form = "信息释放最好和人物情绪反应一起出现，让知情/误解都带有情感重量。";
        }
        Some("suspense") => {
            conceal_line = "保留底牌，但每次至少前进一点认知，不让悬念停在原地。";
            avoid_line = "不要把“先不说”当成唯一手段，读者需要真实推进。";
        }
        Some("relationship") => {
            reveal_form = "信息最好通过关系试探、对话缝隙、站队变化和沉默失误释放。";
        }
        Some("payoff") => {
            reveal_timing = "优先把能形成回收感的信息点放在读者最能感知的节点。";
        }
        _ => {}
    }

    match normalized_focus {
        Some("advance_plot") => {
            reveal_timing = "信息释放应服务动作推进，最好在人物做决定前后给出关键差异。";
        }
        Some("deepen_character") => {
            reveal_form = "信息释放最好能顺手显露人物价值判断、软肋或误判来源。";
        }
        Some("escalate_conflict") => {
            reveal_timing = "每轮信息释放都最好把代价抬高、误会加深或敌意推近。";
        }
        Some("reveal_mystery") => {
            conceal_line = "重点是缩小谜团范围，而不是单纯维持模糊。";
        }
        Some("relationship_shift") => {
            reveal_form = "信息最好通过关系现场爆出，让认知变化直接改写亲疏和立场。";
        }
        Some("foreshadow_payoff") => {
            reveal_timing = "优先选择那些能形成“原来前面是这个意思”的释放节点。";
        }
        _ => {}
    }

    match normalized_stage {
        Some("development") => {
            reveal_timing = "发展阶段信息最好边铺边推，让后续每章都有新认知可长。";
        }
        Some("climax") => {
            reveal_timing = "高潮阶段优先掀关键底牌、核心真相或决定性误判。";
            avoid_line = "不要在高潮关键碰撞前后连续长讲设定";
        }
        Some("ending") => {
            reveal_timing = "结局阶段优先释放那些承担回收主悬念和主承诺的信息。";
        }
        _ => {}
    }

    let combo_text =
        build_chapter_combo_text(creative_mode, story_focus, plot_stage, "默认信息投放");
    format!(
        "【章节信息投放卡】请控制信息释放节奏与形态（{}）\n- 节奏：{}\n- 载体：{}\n- 留白：{}\n- 避免：{}\n",
        combo_text, reveal_timing, reveal_form, conceal_line, avoid_line
    )
}

pub(crate) fn build_story_emotion_landing_card_block(
    creative_mode: &str,
    story_focus: &str,
    plot_stage: &str,
) -> String {
    let Some((normalized_mode, normalized_focus, normalized_stage)) =
        normalized_story_runtime_inputs(creative_mode, story_focus, plot_stage)
    else {
        return String::new();
    };

    let mut trigger_line = "情绪要有触发点：一次冒犯、失去、误会、靠近、失败或迟来的理解。";
    let mut embodiment_line = "情绪优先通过动作、停顿、声音、视线和反应显形，而不是直接盖章。";
    let mut aftermath_line = "高点之后留一点余震，让人物和关系因此发生后续偏移。";
    let mut avoid_line = "不要把情绪只写成统一口径的感叹或总结。";

    match normalized_mode {
        Some("hook") => {
            trigger_line = "情绪触发最好和异常、危险或未决选择绑在一起，形成更强牵引。";
        }
        Some("emotion") => {
            embodiment_line = "本轮优先写情绪如何压到身体、语言和关系距离上。";
            aftermath_line = "情绪高点后最好让人物马上做出一件带后果的事。";
        }
        Some("suspense") => {
            embodiment_line = "情绪最好由不确定、误判、恐惧和认知裂缝慢慢逼出。";
        }
        Some("relationship") => {
            trigger_line = "情绪触发优先来自试探、误伤、靠近失败、信任摇晃或站队变化。";
            aftermath_line = "余震最好直接改写人物之间的说话方式和位置。";
        }
        Some("payoff") => {
            aftermath_line = "回报之后要写读者等来的那口情绪落地感，而不是只交代结果。";
        }
        _ => {}
    }

    match normalized_focus {
        Some("advance_plot") => {
            aftermath_line = "情绪落点要顺手推行动，而不是让节奏停在抒情里。";
        }
        Some("deepen_character") => {
            embodiment_line = "重点写人物如何在情绪里暴露软肋、执念和真实底线。";
        }
        Some("escalate_conflict") => {
            trigger_line = "情绪触发最好随着冲突升级同步变尖，而不是重复同一档反应。";
        }
        Some("reveal_mystery") => {
            embodiment_line = "让人物对新认知的情绪反应成为谜团推进的一部分。";
        }
        Some("relationship_shift") => {
            aftermath_line = "余震要能改变两人的亲疏、信任或合作条件。";
        }
        Some("foreshadow_payoff") => {
            trigger_line = "伏笔兑现时，别忘了给人物一个真实的情绪回响。";
        }
        _ => {}
    }

    match normalized_stage {
        Some("development") => {
            aftermath_line = "发展阶段让情绪余震成为后续行动和关系变化的燃料。";
        }
        Some("climax") => {
            trigger_line = "高潮阶段情绪要跟着碰撞一起爆，而不是另开一条缓慢支线。";
            avoid_line = "不要让高潮情绪刚起势就被解释、复盘或过度总结冲掉。";
        }
        Some("ending") => {
            aftermath_line = "结局阶段的情绪落点更适合沉到余味、代价和人物状态新平衡里。";
        }
        _ => {}
    }

    let combo_text =
        build_chapter_combo_text(creative_mode, story_focus, plot_stage, "默认情绪落点");
    format!(
        "【章节情绪落点卡】请把情绪真实落在现场与后果中（{}）\n- 触发：{}\n- 显形：{}\n- 余震：{}\n- 避免：{}\n",
        combo_text, trigger_line, embodiment_line, aftermath_line, avoid_line
    )
}

pub(crate) fn build_story_action_rendering_card_block(
    creative_mode: &str,
    story_focus: &str,
    plot_stage: &str,
) -> String {
    let Some((normalized_mode, normalized_focus, normalized_stage)) =
        normalized_story_runtime_inputs(creative_mode, story_focus, plot_stage)
    else {
        return String::new();
    };

    let mut action_line = "关键动作段请尽量写成“动作 -> 反馈 -> 变化”，不要概述带过。";
    let mut sensory_line = "动作要有触感、空间感、时间感和阻力感，让读者看见现场。";
    let mut cause_line = "每个动作尽量带出明确结果或新困难，而不是原地表演。";
    let mut avoid_line = "不要在最该展示动作时用一段抽象总结把现场抹掉。";

    match normalized_mode {
        Some("hook") => {
            action_line = "开场动作最好直接咬住异常、危险或未决任务，让读者立刻进入事件。";
        }
        Some("emotion") => {
            sensory_line = "动作描写别忘了带出情绪压迫、犹豫、失控或压抑后的异样反应。";
        }
        Some("suspense") => {
            cause_line = "动作结果最好顺手制造更多认知偏差、危险信号或不确定后果。";
        }
        Some("relationship") => {
            action_line = "动作最好兼顾关系推拉，例如靠近、退让、试探、挡住或越界。";
        }
        Some("payoff") => {
            cause_line = "回报段的动作结果要可感知，别把最该爆的地方直接哑火。";
        }
        _ => {}
    }

    match normalized_focus {
        Some("advance_plot") => {
            cause_line = "动作段的首要标准是推动局势，不是单纯制造画面。";
        }
        Some("deepen_character") => {
            action_line = "通过人物怎样动、何时停、何时失手来暴露他的性格和底线。";
        }
        Some("escalate_conflict") => {
            sensory_line = "随着冲突升级，动作现场的阻力、风险和压迫感也要同步升级。";
        }
        Some("reveal_mystery") => {
            cause_line = "动作最好顺手揭出一个线索、异常结果或新的认知缝隙。";
        }
        Some("relationship_shift") => {
            action_line = "动作里最好含有人际站位变化，别把关系推进只留给对白。";
        }
        Some("foreshadow_payoff") => {
            cause_line = "回收伏笔时尽量让动作直接兑现读者预期，而不是口头认领。";
        }
        _ => {}
    }

    match normalized_stage {
        Some("development") => {
            cause_line = "发展阶段动作最好边推进边抬高代价，不要每次都只是试探。";
        }
        Some("climax") => {
            action_line = "高潮阶段动作段要贴着核心碰撞点写，不要绕开最值钱的现场。";
            avoid_line = "不要把高潮最该写细的动作压成一行概述，让最该爆的地方直接哑火。";
        }
        Some("ending") => {
            cause_line = "结局阶段动作结果更要承担收束职责，让读者看到“事情真的落地了”。";
        }
        _ => {}
    }

    let combo_text =
        build_chapter_combo_text(creative_mode, story_focus, plot_stage, "默认动作显影");
    format!(
        "【章节动作显影卡】关键动作段请写出真正的现场推进（{}）\n- 动作驱动：{}\n- 现场体感：{}\n- 结果牵引：{}\n- 避免：{}\n",
        combo_text, action_line, sensory_line, cause_line, avoid_line
    )
}

pub(crate) fn build_story_summary_tone_control_card_block(
    creative_mode: &str,
    story_focus: &str,
    plot_stage: &str,
) -> String {
    let Some((normalized_mode, normalized_focus, normalized_stage)) =
        normalized_story_runtime_inputs(creative_mode, story_focus, plot_stage)
    else {
        return String::new();
    };

    let mut summary_line = "尽量少用总括式结论句替代现场，让情绪、判断和关系从事件里自己长出来。";
    let mut metaphor_line = "金句和比喻只在真正需要时出现，避免每段都试图盖章。";
    let mut compression_line = "能靠动作、对白和细节完成的，不再补一层作者总结。";
    let mut avoid_line = "避免把现场冲击改写成作者感悟，或把人物反应统一总结成套话。";

    match normalized_mode {
        Some("hook") => {
            summary_line = "钩子模式下更要克制总结，让异常和危险自己带读者往前走。";
        }
        Some("emotion") => {
            metaphor_line = "情绪段可以更柔，但不要把每个情绪点都写成抒情收尾。";
        }
        Some("suspense") => {
            compression_line = "悬念段优先保留信息张力，少用解释性总结把空气抽干。";
        }
        Some("relationship") => {
            summary_line = "关系推进尽量让人物自己说、自己做，不要作者站出来解释亲疏。";
        }
        Some("payoff") => {
            avoid_line = "回收高潮不要靠一句“终于”概括，最好让回报在结果里自己发光。";
        }
        _ => {}
    }

    match normalized_focus {
        Some("advance_plot") => {
            compression_line = "推进优先级高时，更要少解释、多出结果。";
        }
        Some("deepen_character") => {
            summary_line = "人物显形尽量通过选择和失手表现，不要改写成心理小结。";
        }
        Some("escalate_conflict") => {
            metaphor_line = "冲突升级时少抒情复盘，别把张力写成旁白感悟。";
        }
        Some("reveal_mystery") => {
            compression_line = "谜团推进时避免重复概述旧信息，让新认知自己站住。";
        }
        Some("relationship_shift") => {
            avoid_line = "别把关系位移总结成一句抽象评价，让站位变化自己说话。";
        }
        Some("foreshadow_payoff") => {
            summary_line = "伏笔回收时少做“前后呼应”的作者提示，让读者自己感到兑现。";
        }
        _ => {}
    }

    match normalized_stage {
        Some("development") => {
            compression_line = "发展阶段多留可生长的现场，不要急着替读者总结意义。";
        }
        Some("climax") => {
            avoid_line = "高潮阶段不要把现场冲击改写成作者感悟或复盘说明。";
        }
        Some("ending") => {
            summary_line = "结局阶段可以适度留味，但仍要优先让结果自己说话。";
        }
        _ => {}
    }

    let combo_text =
        build_chapter_combo_text(creative_mode, story_focus, plot_stage, "默认总结克制");
    format!(
        "【章节总结腔抑制卡】请压低作者式总结感，保护现场感（{}）\n- 总括句控制：{}\n- 比喻/金句控制：{}\n- 现场替代总结：{}\n- 避免：{}\n",
        combo_text, summary_line, metaphor_line, compression_line, avoid_line
    )
}

pub(crate) fn build_story_repetition_control_card_block(
    creative_mode: &str,
    story_focus: &str,
    plot_stage: &str,
) -> String {
    let Some((normalized_mode, normalized_focus, normalized_stage)) =
        normalized_story_runtime_inputs(creative_mode, story_focus, plot_stage)
    else {
        return String::new();
    };

    let mut phrase_line = "相同语义、判断、情绪结论和环境形容尽量压缩，不要换词重复。";
    let mut beat_line = "同类节拍不要连续出现太多次，比如反复试探、反复解释、反复回忆。";
    let mut paragraph_line = "每一段尽量承担不同任务：推进、揭示、关系、情绪，不要段段同功能。";
    let mut avoid_line = "避免同一信息被人物、旁白和动作各说一遍。";

    match normalized_mode {
        Some("hook") => {
            beat_line = "钩子模式下避免一连串“又来一个异常”，要让每次异常性质不同。";
        }
        Some("emotion") => {
            phrase_line = "情绪推进避免反复描述同一种痛、慌或压抑，最好让反应层层变化。";
        }
        Some("suspense") => {
            beat_line = "悬念推进不要反复只做“感觉不对劲”，每轮都要有新证据或新偏差。";
        }
        Some("relationship") => {
            paragraph_line = "关系线不要总在争吵/沉默两种旧节拍里打转，尽量改变互动方式。";
        }
        Some("payoff") => {
            avoid_line = "回收阶段不要一边兑现一边重复解释为什么这算兑现。";
        }
        _ => {}
    }

    match normalized_focus {
        Some("advance_plot") => {
            paragraph_line = "推进优先时尤其要清掉重复铺垫和重复确认。";
        }
        Some("deepen_character") => {
            phrase_line = "人物塑形不要总靠同一种回忆、自责或执念句式反复加深。";
        }
        Some("escalate_conflict") => {
            beat_line = "冲突升级不能只是同级摩擦换场景重来，必须抬级。";
        }
        Some("reveal_mystery") => {
            avoid_line = "谜团推进别反复把旧疑点换种说法重提。";
        }
        Some("relationship_shift") => {
            paragraph_line = "关系变化要有新动作和新后果，不要只重复旧伤口。";
        }
        Some("foreshadow_payoff") => {
            beat_line = "伏笔回收不要一边兑现一边回顾太多旧内容。";
        }
        _ => {}
    }

    match normalized_stage {
        Some("development") => {
            paragraph_line = "发展阶段尤其要避免“搭变量”反复同法执行。";
        }
        Some("climax") => {
            avoid_line = "高潮阶段少复盘、少重复解释、少假动作。";
        }
        Some("ending") => {
            beat_line = "结局阶段避免把已收束的内容再收一遍。";
        }
        _ => {}
    }

    let combo_text = build_chapter_combo_text(creative_mode, story_focus, plot_stage, "默认压缩");
    format!(
        "【章节重复压缩卡】请主动压掉重复表达和重复节拍（{}）\n- 句法/表述：{}\n- 节拍：{}\n- 段落任务：{}\n- 避免：{}\n",
        combo_text, phrase_line, beat_line, paragraph_line, avoid_line
    )
}

pub(crate) fn build_story_viewpoint_discipline_card_block(
    creative_mode: &str,
    story_focus: &str,
    plot_stage: &str,
) -> String {
    let Some((normalized_mode, normalized_focus, normalized_stage)) =
        normalized_story_runtime_inputs(creative_mode, story_focus, plot_stage)
    else {
        return String::new();
    };

    let mut pov_line = "视角人物当下不知道的，不要直接写成既成事实。";
    let mut perception_line = "优先写视角人物看到、听到、猜到、误判到的东西，而不是上帝视角解释。";
    let mut switch_line = "若必须切换信息，也尽量通过对白、证据或外部反馈而不是偷偷换镜头。";
    let mut avoid_line = "避免一段里同时交代多人内心和全局真相。";

    match normalized_mode {
        Some("hook") => {
            perception_line = "开场钩子更适合贴着当下可感信息写，让异常从感知里冒出来。";
        }
        Some("emotion") => {
            pov_line = "情绪场面尤其要守住主视角，不要一激动就跳去替别人解释心思。";
        }
        Some("suspense") => {
            switch_line = "悬念场面更要克制换镜头，让未知保持在视角盲区里。";
        }
        Some("relationship") => {
            perception_line = "关系推进优先写人物如何误读、试探、猜测和修正对方。";
        }
        Some("payoff") => {
            avoid_line = "回报节点别因为急着交代而突然偷开全知镜头。";
        }
        _ => {}
    }

    match normalized_focus {
        Some("advance_plot") => {
            perception_line = "推进优先时也别为了快说明就破坏视角纪律。";
        }
        Some("deepen_character") => {
            pov_line = "人物塑形越重，越要让读者通过他的主观认知看见他是谁。";
        }
        Some("escalate_conflict") => {
            switch_line = "冲突升级时不要靠突然揭示别人的真实想法来取巧。";
        }
        Some("reveal_mystery") => {
            avoid_line = "谜团推进时不要在高潮节点频繁切镜头或偷发答案。";
        }
        Some("relationship_shift") => {
            perception_line = "关系位移应优先通过视角人物对对方的新判断显现。";
        }
        Some("foreshadow_payoff") => {
            switch_line = "伏笔回收也尽量通过主视角可感知证据完成。";
        }
        _ => {}
    }

    match normalized_stage {
        Some("development") => {
            pov_line = "发展阶段先把主视角站稳，方便后续情绪和谜团累积。";
        }
        Some("climax") => {
            avoid_line = "不要在高潮现场频繁切镜头、偷开全知或替反派解释。";
        }
        Some("ending") => {
            switch_line = "结局阶段即便需要回收信息，也尽量沿主视角或稳定叙事通道完成。";
        }
        _ => {}
    }

    let combo_text = build_chapter_combo_text(creative_mode, story_focus, plot_stage, "默认视角");
    format!(
        "【章节视角纪律卡】请守住视角与信息边界（{}）\n- 知识边界：{}\n- 感知通道：{}\n- 信息切换：{}\n- 避免：{}\n",
        combo_text, pov_line, perception_line, switch_line, avoid_line
    )
}

pub(crate) fn build_story_dialogue_advancement_card_block(
    creative_mode: &str,
    story_focus: &str,
    plot_stage: &str,
) -> String {
    let Some((normalized_mode, normalized_focus, normalized_stage)) =
        normalized_story_runtime_inputs(creative_mode, story_focus, plot_stage)
    else {
        return String::new();
    };

    let mut function_line = "对白要承担推进、试探、施压、揭示或关系变化，不要只是把信息说一遍。";
    let mut tension_line = "对话里尽量保留立场差、信息差和潜台词，不要人人都一句到位。";
    let mut rhythm_line = "长段对白之间最好穿插动作、停顿、打断或环境反馈，保持现场感。";
    let avoid_line = "避免把设定说明和前情提要整段塞进人物嘴里。";

    match normalized_mode {
        Some("hook") => {
            function_line = "钩子模式下对白更适合快速抛出任务、异常或威胁，而不是寒暄导入。";
        }
        Some("emotion") => {
            tension_line = "情绪场对白要有压抑、误伤、回避或靠近失败，不要都说满。";
        }
        Some("suspense") => {
            function_line = "悬念场对白优先制造认知偏差、半真半假和信息缺口。";
        }
        Some("relationship") => {
            tension_line = "关系场对白更要写试探、退让、误读和越界，不只是交换信息。";
        }
        Some("payoff") => {
            function_line = "回收节点的对白要服务兑现后的立场变化，而不只是解释兑现内容。";
        }
        _ => {}
    }

    match normalized_focus {
        Some("advance_plot") => {
            function_line = "对白的第一职责是推事，不能聊了很多却不改变行动。";
        }
        Some("deepen_character") => {
            tension_line = "对白最好暴露人物真实底色和价值判断，而不是只做说明通道。";
        }
        Some("escalate_conflict") => {
            rhythm_line = "冲突对白要越说越紧，不要在高压时变成长篇轮流陈述。";
        }
        Some("reveal_mystery") => {
            function_line = "谜团对白要像拆线索，不要像作者问答。";
        }
        Some("relationship_shift") => {
            tension_line = "关系位移要通过说话方式、称呼、分寸和沉默本身体现。";
        }
        Some("foreshadow_payoff") => {
            function_line = "对白可以承担伏笔回收，但最好顺带推动新的关系或行动后果。";
        }
        _ => {}
    }

    match normalized_stage {
        Some("development") => {
            rhythm_line = "发展阶段对白也要持续带出变量，而不是静态铺设信息。";
        }
        Some("climax") => {
            function_line = "高潮对白要逼近核心碰撞，不要在高潮对白里长篇复盘前情。";
        }
        Some("ending") => {
            tension_line = "结局对白更适合留有余味、未说破和代价感，而不必所有结论全说满。";
        }
        _ => {}
    }

    let combo_text =
        build_chapter_combo_text(creative_mode, story_focus, plot_stage, "默认对白推进");
    format!(
        "【章节对白推进卡】请让对白承担真正推进职责（{}）\n- 功能：{}\n- 张力：{}\n- 节奏：{}\n- 避免：{}\n",
        combo_text, function_line, tension_line, rhythm_line, avoid_line
    )
}

pub(crate) fn build_story_opening_hook_card_block(
    creative_mode: &str,
    story_focus: &str,
    plot_stage: &str,
) -> String {
    let Some((normalized_mode, normalized_focus, normalized_stage)) =
        normalized_story_runtime_inputs(creative_mode, story_focus, plot_stage)
    else {
        return String::new();
    };

    let mut hook_line = "开篇前 20%-25% 尽量抛出一个异常、任务、冲突或明显受阻点。";
    let mut clarity_line = "钩子要让读者迅速知道“为什么我现在该继续往下看”。";
    let mut tie_line = "钩子最好直接绑到本章主任务，而不是独立的小花活。";
    let mut avoid_line = "避免慢热导入太久，或先写一段氛围再想起推进。";

    match normalized_mode {
        Some("hook") => {
            hook_line = "本轮优先把最抓人的异常、危险或未决选择提前扔到读者面前。";
        }
        Some("emotion") => {
            clarity_line = "情绪向开篇也尽量通过关系裂缝、误伤余波或靠近失败来抓人。";
        }
        Some("suspense") => {
            hook_line = "悬念向开篇最好先给一个反常信号、错误结果或让人不安的证据。";
        }
        Some("relationship") => {
            tie_line = "关系向开篇最好直接扔出一次试探、对立、亏欠或站队张力。";
        }
        Some("payoff") => {
            hook_line = "回报向开篇可直接接到上一轮承诺、结果或兑现前夜。";
        }
        _ => {}
    }

    match normalized_focus {
        Some("advance_plot") => {
            tie_line = "钩子必须直接服务主线推进，不要用无关热闹开场。";
        }
        Some("deepen_character") => {
            clarity_line = "开篇最好通过人物选择或失控反应抓住读者，而不是先讲背景。";
        }
        Some("escalate_conflict") => {
            hook_line = "开篇最好把冲突直接抬到新的压力档位。";
        }
        Some("reveal_mystery") => {
            hook_line = "开篇优先丢一个能立刻缩小谜团范围的反常点。";
        }
        Some("relationship_shift") => {
            tie_line = "关系位移向开篇最好直接展示人物之间的异常新站位。";
        }
        Some("foreshadow_payoff") => {
            hook_line = "开篇可直接触发前文伏笔的兑现倒计时。";
        }
        _ => {}
    }

    match normalized_stage {
        Some("development") => {
            clarity_line = "发展阶段开篇除了抓人，还要尽快把本章任务方向立住。";
        }
        Some("climax") => {
            hook_line = "高潮阶段的开篇要延续既有高压，开篇前 20%-25% 尽快把人物推到主碰撞现场。";
            avoid_line = "不要在高潮开篇重新解释局势，直接回到最值钱的压力线上。";
        }
        Some("ending") => {
            tie_line = "结局阶段开篇更适合直接接主承诺、主悬念或关键关系线。";
        }
        _ => {}
    }

    let combo_text = build_chapter_combo_text(creative_mode, story_focus, plot_stage, "默认抓力");
    format!(
        "【章节开篇抓力卡】请让开篇快速形成阅读牵引（{}）\n- 钩子：{}\n- 读者明白什么：{}\n- 与本章任务绑定：{}\n- 避免：{}\n",
        combo_text, hook_line, clarity_line, tie_line, avoid_line
    )
}

pub(crate) fn build_story_execution_checklist_block(
    creative_mode: &str,
    story_focus: &str,
    plot_stage: &str,
) -> String {
    let Some((normalized_mode, normalized_focus, normalized_stage)) =
        normalized_story_runtime_inputs(creative_mode, story_focus, plot_stage)
    else {
        return String::new();
    };

    let mut checklist = vec![
        "开篇尽快进入异常、任务、冲突或受阻点。".to_string(),
        "中段至少形成一次局势推进或认知刷新。".to_string(),
        "安排一次可感知的情绪/关系后果。".to_string(),
        "结尾保留继续推进的牵引。".to_string(),
    ];

    match normalized_mode {
        Some("hook") => {
            checklist.push("检查开篇和结尾是否真的形成抓力，而不只是大声量。".to_string())
        }
        Some("emotion") => checklist.push("检查情绪高点是否落到动作、对白和余震里。".to_string()),
        Some("suspense") => {
            checklist.push("检查谜团是否真的推进，而不是只多一层模糊。".to_string())
        }
        Some("relationship") => {
            checklist.push("检查关系是否发生真实位移，而非情绪原地打转。".to_string())
        }
        Some("payoff") => checklist.push("检查本章是否兑现了一个读者能感到的承诺。".to_string()),
        _ => {}
    }

    match normalized_focus {
        Some("advance_plot") => {
            checklist.push("检查主线是否确实往前推进，而不是热闹空转。".to_string())
        }
        Some("deepen_character") => checklist.push("检查人物是否在选择里显形。".to_string()),
        Some("escalate_conflict") => checklist.push("检查冲突是否真的抬级。".to_string()),
        Some("reveal_mystery") => {
            checklist.push("检查谜团是否缩小范围或获得有效新认知。".to_string())
        }
        Some("relationship_shift") => {
            checklist.push("检查关系变化是否足以影响后续互动。".to_string())
        }
        Some("foreshadow_payoff") => checklist.push("检查伏笔回收是否真正落地。".to_string()),
        _ => {}
    }

    match normalized_stage {
        Some("development") => checklist.push("发展阶段检查变量是否真正铺开。".to_string()),
        Some("climax") => {
            checklist.push("高潮阶段开场尽快把人物推到主碰撞现场。".to_string());
            checklist.push("高潮阶段检查是否发生决定性碰撞或不可逆变化。".to_string());
        }
        Some("ending") => {
            checklist.push("结局阶段检查主承诺与关键关系线是否有效回收。".to_string())
        }
        _ => {}
    }

    let combo_text =
        build_chapter_combo_text(creative_mode, story_focus, plot_stage, "默认执行节奏");
    let mut lines = vec![format!(
        "【章节执行清单】写作过程中请逐项自检（{}）",
        combo_text
    )];
    lines.extend(checklist.into_iter().map(|item| format!("- {}", item)));
    format!("{}\n", lines.join("\n"))
}

pub(crate) fn build_story_scene_anchor_card_block(
    creative_mode: &str,
    story_focus: &str,
    plot_stage: &str,
) -> String {
    let Some((normalized_mode, normalized_focus, normalized_stage)) =
        normalized_story_runtime_inputs(creative_mode, story_focus, plot_stage)
    else {
        return String::new();
    };

    let mut anchor_line = "关键段落尽量有清晰场景锚点：人物在哪、眼前有什么、威胁来自哪里。";
    let mut movement_line = "场景调度里尽量让人物位置、距离和动线参与叙事，而不是纯背景。";
    let mut transition_line = "场景转换最好带功能：推进、揭示、情绪转场或关系位移。";
    let mut avoid_line = "不要在无锚点的空白空间里长时间对话或抒情。";

    match normalized_mode {
        Some("hook") => {
            anchor_line = "开场锚点要尽快给出异常位置或危险来源，让读者立刻抓住现场。";
        }
        Some("emotion") => {
            movement_line = "情绪场里的距离变化、靠近退后和动作停顿都应参与表达。";
        }
        Some("suspense") => {
            anchor_line = "悬念场更要把异常位置、视线遮挡和可疑物明确放出来。";
        }
        Some("relationship") => {
            movement_line = "关系场最好通过站位、坐立、高低、靠近和回避写出权力差。";
        }
        Some("payoff") => {
            transition_line = "回收场的场景转换最好直接服务兑现后的后效和下一步压力。";
        }
        _ => {}
    }

    match normalized_focus {
        Some("advance_plot") => {
            transition_line = "每次切场最好都把任务推进到新位置。";
        }
        Some("deepen_character") => {
            movement_line = "人物如何占据空间、如何绕开他人，也是在塑形。";
        }
        Some("escalate_conflict") => {
            anchor_line = "冲突场要尽量让环境本身参与施压。";
        }
        Some("reveal_mystery") => {
            anchor_line = "谜团场的可疑物、盲区和异样反馈要有明确锚点。";
        }
        Some("relationship_shift") => {
            movement_line = "关系位移最好同时体现为空间关系变化。";
        }
        Some("foreshadow_payoff") => {
            transition_line = "回收伏笔时场景最好能让读者感到“这里终于用上了”。";
        }
        _ => {}
    }

    match normalized_stage {
        Some("development") => {
            transition_line = "发展阶段切场最好扩张变量，不只是换背景。";
        }
        Some("climax") => {
            anchor_line = "高潮阶段镜头尽量贴近最核心的碰撞点。";
            avoid_line = "不要在高潮阶段切到低价值旁支场景稀释主冲击。";
        }
        Some("ending") => {
            transition_line = "结局阶段场景转换更适合服务收束和余味。";
        }
        _ => {}
    }

    let combo_text =
        build_chapter_combo_text(creative_mode, story_focus, plot_stage, "默认场景调度");
    format!(
        "【章节场景调度卡】请给关键段落明确场景锚点（{}）\n- 锚点：{}\n- 空间关系：{}\n- 场景转换：{}\n- 避免：{}\n",
        combo_text, anchor_line, movement_line, transition_line, avoid_line
    )
}

pub(crate) fn build_story_scene_density_card_block(
    creative_mode: &str,
    story_focus: &str,
    plot_stage: &str,
) -> String {
    let Some((normalized_mode, normalized_focus, normalized_stage)) =
        normalized_story_runtime_inputs(creative_mode, story_focus, plot_stage)
    else {
        return String::new();
    };

    let mut density_line =
        "场景里尽量同时承载动作、环境反馈、人物反应和推进结果，不要只剩单线程说明。";
    let mut ratio_line = "叙事密度优先向“现场化比例”倾斜，减少长段纯解释。";
    let mut layering_line =
        "一个段落里最好至少叠两层有效信息：动作 + 情绪、对白 + 推进、线索 + 后果等。";
    let avoid_line = "不要让高价值段落只剩交代，没有现场。";

    match normalized_mode {
        Some("hook") => {
            ratio_line = "钩子模式下前段现场化比例更高，尽快让读者进入事件。";
        }
        Some("emotion") => {
            layering_line = "情绪场也不要只有情绪，最好叠上动作或关系后果。";
        }
        Some("suspense") => {
            density_line = "悬念场最好同时给异常细节、风险反馈和认知偏差。";
        }
        Some("relationship") => {
            layering_line = "关系场最好叠上对白、动作和站位，而不是只写心理。";
        }
        Some("payoff") => {
            ratio_line = "回收场尽量用现场回报替代解释回报。";
        }
        _ => {}
    }

    match normalized_focus {
        Some("advance_plot") => {
            density_line = "每一段都尽量同时承担推进结果，不要只蓄势。";
        }
        Some("deepen_character") => {
            layering_line = "人物塑形尽量嵌在推进和互动里，少做独立介绍段。";
        }
        Some("escalate_conflict") => {
            density_line = "冲突升级时最好同时写出代价、动作和情绪压迫。";
        }
        Some("reveal_mystery") => {
            layering_line = "谜团推进最好叠上线索、误判和后果，而非只抛概念。";
        }
        Some("relationship_shift") => {
            layering_line = "关系位移最好同时体现为语言、动作和局势重排。";
        }
        Some("foreshadow_payoff") => {
            ratio_line = "回收伏笔时优先把兑现嵌进场景，而不是抽离说明。";
        }
        _ => {}
    }

    match normalized_stage {
        Some("development") => {
            density_line = "发展阶段也要提高单位段落产出，别靠篇幅堆推进。";
        }
        Some("climax") => {
            ratio_line = "高潮阶段要提高现场化比例，让高压真正落到眼前。";
        }
        Some("ending") => {
            layering_line = "结局阶段最好把收束、余味和代价叠在同一组结果里。";
        }
        _ => {}
    }

    let combo_text = build_chapter_combo_text(creative_mode, story_focus, plot_stage, "默认密度");
    format!(
        "【章节场景密度卡】请提高单位段落的信息与现场密度（{}）\n- 场景负载：{}\n- 现场化比例：{}\n- 叠层方式：{}\n- 避免：{}\n",
        combo_text, density_line, ratio_line, layering_line, avoid_line
    )
}

pub(crate) fn build_story_repetition_risk_block(
    creative_mode: &str,
    story_focus: &str,
    plot_stage: &str,
) -> String {
    let Some((normalized_mode, normalized_focus, normalized_stage)) =
        normalized_story_runtime_inputs(creative_mode, story_focus, plot_stage)
    else {
        return String::new();
    };

    let mut risk_1 = "检查是否又出现同一种开场节拍、同一种转折方式、同一种结尾悬停。";
    let mut risk_2 = "检查是否反复解释同一信息、同一情绪或同一关系判断。";
    let mut risk_3 = "检查是否多个段落承担了几乎一样的功能，只是换了措辞。";
    let avoid_line = "一旦发现重复，优先删掉次要段落而不是再换几个近义句。";

    match normalized_mode {
        Some("hook") => {
            risk_1 = "钩子模式下尤其要检查是否每章都靠同一种异常/危险起手。";
        }
        Some("emotion") => {
            risk_2 = "情绪模式下尤其要检查是否总用同一种沉默、自责或流泪推进。";
        }
        Some("suspense") => {
            risk_1 = "悬念模式下尤其要检查是否反复假装要揭示、却一直不前进。";
        }
        Some("relationship") => {
            risk_3 = "关系模式下尤其要检查是否总在重复争吵、冷场或嘴硬套路。";
        }
        Some("payoff") => {
            risk_2 = "回收模式下尤其要检查是否一边兑现一边重复回顾旧铺垫。";
        }
        _ => {}
    }

    match normalized_focus {
        Some("advance_plot") => {
            risk_3 = "推进优先时尤其要检查是否有多段都只在原地准备。";
        }
        Some("deepen_character") => {
            risk_2 = "人物塑形时尤其要检查是否反复讲同一个弱点。";
        }
        Some("escalate_conflict") => {
            risk_1 = "冲突升级时尤其要检查是否只是同级别摩擦换皮复用。";
        }
        Some("reveal_mystery") => {
            risk_2 = "谜团推进时尤其要检查是否把旧疑问换说法重提。";
        }
        Some("relationship_shift") => {
            risk_3 = "关系位移时尤其要检查是否反复强调同一裂缝而无新后果。";
        }
        Some("foreshadow_payoff") => {
            risk_1 = "伏笔回收时尤其要检查是否同一回收感被重复制造多次。";
        }
        _ => {}
    }

    match normalized_stage {
        Some("development") => {
            risk_3 = "发展阶段尤其要检查是否反复搭变量、却没有真的进入新局面。";
        }
        Some("climax") => {
            risk_1 = "高潮阶段不要反复假装要碰撞，却一直拖延真正碰撞。";
        }
        Some("ending") => {
            risk_2 = "结局阶段不要把已经收束过的情绪和判断再重复收一次。";
        }
        _ => {}
    }

    let combo_text = build_chapter_combo_text(creative_mode, story_focus, plot_stage, "默认避重");
    format!(
        "【章节重复风险卡】请主动排查这些复写风险（{}）\n- 节拍风险：{}\n- 信息风险：{}\n- 段落功能风险：{}\n- 处理建议：{}\n",
        combo_text, risk_1, risk_2, risk_3, avoid_line
    )
}

pub(crate) fn build_story_acceptance_card_block(
    creative_mode: &str,
    story_focus: &str,
    plot_stage: &str,
) -> String {
    let Some((normalized_mode, normalized_focus, normalized_stage)) =
        normalized_story_runtime_inputs(creative_mode, story_focus, plot_stage)
    else {
        return String::new();
    };

    let mut mission_check = "验收时先看本章是否完成了一个清晰主任务，而不是热闹但空转。";
    let mut change_check = "至少要看到局势、关系或认知有一项明确变化，不能原地踏步。";
    let mut freshness_check = "检查开场、加压、转折、收尾是否又落回同一种旧套路。";
    let mut closing_check = "章尾既要完成本章收束，也要留下合适的追读牵引或余味。";

    match normalized_mode {
        Some("hook") => {
            mission_check = "验收时重点看开场和章尾是否真正形成牵引，而不只是制造噪音。";
            closing_check = "结尾要让读者有继续读的冲动，但不能只有硬切和悬空。";
        }
        Some("emotion") => {
            change_check = "验收时要看到情绪余震和关系后果，而不是只有一段抒情。";
            freshness_check = "检查情绪推进是否又只是争吵、沉默或内心独白轮换。";
        }
        Some("suspense") => {
            change_check = "验收时至少要有一个有效线索、认知刷新或危险升级真正落地。";
            closing_check = "结尾要留下更尖锐的问题，但不能完全不给有效信息。";
        }
        Some("relationship") => {
            mission_check = "验收时看人物关系是否真的发生位移，而不是只多说了几句狠话。";
            change_check = "关系变化最好能改动人物之后的站位、合作或信任条件。";
        }
        Some("payoff") => {
            mission_check = "验收时要确认前文铺垫是否真正兑现，而不是只口头提到。";
            closing_check = "兑现之后要有后效和新失衡，不能只停在一次性爽点。";
        }
        _ => {}
    }

    match normalized_focus {
        Some("advance_plot") => {
            mission_check = "验收时先看主线是否实打实前进，而不是忙了很多事却没推局势。";
        }
        Some("deepen_character") => {
            change_check = "验收时看人物是否在选择里显形，而不是只补充背景说明。";
            freshness_check = "检查人物塑形是否又回到同一种回忆、自责或旁白总结。";
        }
        Some("escalate_conflict") => {
            change_check = "验收时要能看见代价升级、对立加深或冲突进入新层级。";
            closing_check = "本轮结束后人物应被留在更难的位置，而不是轻松退回安全区。";
        }
        Some("reveal_mystery") => {
            mission_check = "验收时必须确认谜团有真实推进，而不是只多堆了一层雾。";
        }
        Some("relationship_shift") => {
            change_check = "验收时看关系是否足以改变说话方式、行动选择或站队逻辑。";
        }
        Some("foreshadow_payoff") => {
            mission_check = "验收时确认伏笔是否兑现落地，同时打开了新的后续空间。";
        }
        _ => {}
    }

    match normalized_stage {
        Some("development") => {
            mission_check = "发展阶段验收重点是：有没有把局势、变量和主任务真正搭起来。";
            closing_check = "收尾应让下一轮任务更具体，而不是继续停留在准备态。";
        }
        Some("climax") => {
            change_check = "高潮阶段验收重点是：有没有形成决定性碰撞、底牌掀开或局势断裂。";
            freshness_check = "检查高潮是否只是声量更大，还是确实发生了不可逆变化。";
        }
        Some("ending") => {
            mission_check = "结局阶段验收重点是：主承诺、主悬念和关键关系线是否得到有效回收。";
            closing_check = "收尾应保留余味，但不能为了留白再次打散已经完成的收束。";
        }
        _ => {}
    }

    let combo_text = build_chapter_combo_text(creative_mode, story_focus, plot_stage, "默认验收");
    format!(
        "【章节验收卡】成稿前请用以下标准验收本轮是否真正达标（{}）\n- 任务命中：{}\n- 变化落地：{}\n- 新鲜度：{}\n- 收束质量：{}\n",
        combo_text, mission_check, change_check, freshness_check, closing_check
    )
}

pub(crate) fn build_story_cliffhanger_card_block(
    creative_mode: &str,
    story_focus: &str,
    plot_stage: &str,
) -> String {
    let Some((normalized_mode, normalized_focus, normalized_stage)) =
        normalized_story_runtime_inputs(creative_mode, story_focus, plot_stage)
    else {
        return String::new();
    };

    let mut unresolved_point =
        "章尾要留一个具体未决点：一个答案缺口、一个马上要做的选择，或一个刚翻面的新问题。";
    let mut next_push = "结尾最好把人物逼到下一步动作边缘，让读者自然想看下一章。";
    let mut aftertaste = "除了钩子，还要留一点情绪余味、代价回响或关系余震。";
    let mut stage_line = "";
    let mut avoid_line = "不要只靠突然打断、无信息硬切或机械性的“未完待续感”制造悬停。";

    match normalized_mode {
        Some("hook") => {
            unresolved_point =
                "未决点优先是迫近选择、倒计时危险或刚被掀开的麻烦，不要只做语气停顿。";
            next_push = "下一步逼力要明确到人物不得不马上应对，而不是以后再说。";
        }
        Some("emotion") => {
            aftertaste = "余味最好落在误伤后的沉默、靠近失败后的反弹，或关系未说破的震荡上。";
            avoid_line = "不要在情绪高点后立刻解释完、说透完，把回响全部冲掉。";
        }
        Some("suspense") => {
            unresolved_point = "未决点最好是线索翻面、认知裂缝、危险升级或答案只揭开半层。";
            aftertaste = "余味要让读者感到局势更深、更险，而不是只多了一个名词。";
        }
        Some("relationship") => {
            unresolved_point = "未决点最好和立场未定、关系悬空、合作破裂或信任临界绑定。";
            aftertaste = "余味应保留人物之间的温差、敌意、亏欠或迟到的理解。";
        }
        Some("payoff") => {
            unresolved_point = "兑现之后要留一个新失衡或新代价，说明故事没有在爽点处直接封口。";
            next_push = "下一步逼力最好来自兑现后的后效，而不是硬塞一个无关新坑。";
        }
        _ => {}
    }

    match normalized_focus {
        Some("advance_plot") => {
            next_push = "结尾逼力必须能接到主线下一步，不要只留下气氛而没有行动方向。";
        }
        Some("deepen_character") => {
            aftertaste = "余味最好让读者记住人物此刻的新伤口、新认知或新自我怀疑。";
        }
        Some("escalate_conflict") => {
            unresolved_point = "未决点应落在冲突升级后的更难位置：谁先出手、谁先失控、谁先付代价。";
            next_push = "下一步逼力要让人物无法轻松退回安全区。";
        }
        Some("reveal_mystery") => {
            unresolved_point = "未决点最好是刚拿到半个答案，却暴露出更关键的缺口或反常。";
        }
        Some("relationship_shift") => {
            aftertaste = "余味要落在关系新站位上，让读者感到他们再也回不到原来的相处方式。";
        }
        Some("foreshadow_payoff") => {
            unresolved_point =
                "未决点可以是旧伏笔兑现后的新空缺，说明兑现带来了新的问题而非彻底归零。";
        }
        _ => {}
    }

    match normalized_stage {
        Some("development") => {
            stage_line = "发展阶段的章尾/卷尾要把下一轮任务说得更具体，别总停在模糊愿景。";
        }
        Some("climax") => {
            stage_line = "高潮阶段的结尾要保持冲击余震与决战逼力，不要突然卸压。";
            avoid_line = "不要在高潮结尾处仓促复盘、解释一切或切回轻松缓冲，导致气势塌掉。";
        }
        Some("ending") => {
            stage_line =
                "结局阶段可以减少硬卖关子，更适合保留余波、代价、阴影或尚未完全愈合的裂口。";
            avoid_line = "不要为了续作感硬开全新主线；更适合留下收束后的余味和未尽代价。";
        }
        _ => {}
    }

    let combo_text = build_chapter_combo_text(creative_mode, story_focus, plot_stage, "默认悬停");
    let mut lines = vec![format!(
        "【章节结尾悬停卡】收尾请留下继续阅读/推进的牵引（{}）",
        combo_text
    )];
    lines.push(format!("- 未决点：{}", unresolved_point));
    lines.push(format!("- 下一步逼力：{}", next_push));
    lines.push(format!("- 余味：{}", aftertaste));
    lines.push("- 硬指标：最后一段至少落下 2 类尾钩信号（信息缺口 / 危险逼近 / 身份位移 / 待做选择 / 事态升级），最后一句不要复盘解释。".to_string());
    if !stage_line.is_empty() {
        lines.push(format!("- 阶段提醒：{}", stage_line));
    }
    lines.push(format!("- 避免：{}", avoid_line));
    format!("{}\n", lines.join("\n"))
}

pub(crate) fn build_story_character_arc_card_block(
    creative_mode: &str,
    story_focus: &str,
    plot_stage: &str,
) -> String {
    let Some((normalized_mode, normalized_focus, normalized_stage)) =
        normalized_story_runtime_inputs(creative_mode, story_focus, plot_stage)
    else {
        return String::new();
    };

    let mut external_line = "本章要让人物在外在线上做出能看见后果的动作，而不是被剧情拖着走。";
    let mut internal_line = "本章要逼出一次能暴露人物软肋、执念或底线的反应。";
    let mut relationship_line = "至少让一条关系线发生可见位移，而不只是多说几句情绪台词。";
    let mut arc_landing = "章尾要留下人物状态的新落点，让后续成长有承接。";

    match normalized_mode {
        Some("hook") => {
            external_line = "人物外在线最好和迫近危险、未决选择或新任务直接绑定，让他不得不动。";
            arc_landing = "弧光落点要落在人物被推入新处境上，而不只是事件悬空。";
        }
        Some("emotion") => {
            internal_line = "内在线重点看人物如何被情绪反噬、误伤他人或压抑失败。";
            relationship_line = "关系线最好呈现安慰失败、靠近受阻或误伤后的余震。";
        }
        Some("suspense") => {
            external_line = "人物外在线尽量和追查、判断、求生或拆解异常绑定。";
            internal_line = "通过误判、恐惧和认知落差暴露人物真正的盲区与偏执。";
        }
        Some("relationship") => {
            relationship_line = "关系线必须承担主推进，最好出现站队变化、信任重排或亲疏重估。";
            arc_landing = "落点应让人物在关系位置上进入一个再也回不到原点的新阶段。";
        }
        Some("payoff") => {
            external_line = "人物外在线要和旧承诺兑现、旧目标回收或能力回报直接挂钩。";
            arc_landing = "落点要让人物因为兑现获得成长回报，或承担兑现带来的新责任。";
        }
        _ => {}
    }

    match normalized_focus {
        Some("advance_plot") => {
            external_line = "人物外在线必须和主线推进同频，行动要真的改变局势而非走流程。";
        }
        Some("deepen_character") => {
            internal_line = "内在线要让人物在选择里显形，看见他的软肋、执念和价值判断。";
            arc_landing = "落点最好形成一次人物自我认知偏移，而不只是事件结束。";
        }
        Some("escalate_conflict") => {
            internal_line = "冲突升级时要逼出人物底线，看看他在更高代价下会怎么变。";
            relationship_line = "更强冲突最好同步改写人物之间的站位与依赖结构。";
        }
        Some("reveal_mystery") => {
            external_line = "人物外在线最好围绕调查、判断和选择展开，而不是旁观真相自己掉下来。";
            internal_line = "认知刷新应反照人物偏见、恐惧或执念，而不是只补世界观信息。";
        }
        Some("relationship_shift") => {
            relationship_line =
                "关系线验收重点是：人物之后的说话方式、站位和合作条件是否真的变了。";
        }
        Some("foreshadow_payoff") => {
            arc_landing = "人物应因为伏笔兑现进入新的自我认知、责任位置或情感阶段。";
        }
        _ => {}
    }

    match normalized_stage {
        Some("development") => {
            external_line = "发展阶段先把人物眼前要争什么、躲什么、赌什么摆清楚。";
            arc_landing = "落点应把人物推入更难但更清晰的成长压力链。";
        }
        Some("climax") => {
            internal_line = "高潮阶段要逼出人物真正底线、真实选择或最不愿面对的自我。";
            relationship_line = "高潮中的关系变化最好是定向性变化，而不是小幅试探。";
        }
        Some("ending") => {
            relationship_line = "结局阶段要让关键关系线出现收束、定局或带余温的最终位移。";
            arc_landing = "落点要给人物阶段性定局、余味或代价后的新平衡。";
        }
        _ => {}
    }

    let combo_text = build_chapter_combo_text(creative_mode, story_focus, plot_stage, "默认弧光");
    format!(
        "【章节角色弧光卡】本轮至少让人物弧光出现以下推进（{}）\n- 外在线：{}\n- 内在线：{}\n- 关系线：{}\n- 落点：{}\n",
        combo_text, external_line, internal_line, relationship_line, arc_landing
    )
}

pub(crate) fn build_story_card_owner_contract() -> Value {
    json!({
        "owner": "chapter_generation_prompt_service::story_card_owner",
        "scope": "narrative_blueprint_and_story_card_runtime_prompt_block_family",
        "python_source_map": [],
        "rust_owner_map": [
            "backend-rs/src/services/chapter_generation_prompt_service.rs",
            "backend-rs/src/services/chapter_generation_prompt_service/story_card_owner.rs"
        ],
        "behavior_contract": {
            "entrypoints": [
                "build_narrative_blueprint_block",
                "build_story_objective_card_block",
                "build_story_result_card_block",
                "build_story_payoff_chain_card_block",
                "build_story_rule_grounding_card_block",
                "build_story_information_release_card_block",
                "build_story_emotion_landing_card_block",
                "build_story_action_rendering_card_block",
                "build_story_summary_tone_control_card_block",
                "build_story_repetition_control_card_block",
                "build_story_viewpoint_discipline_card_block",
                "build_story_dialogue_advancement_card_block",
                "build_story_opening_hook_card_block",
                "build_story_execution_checklist_block",
                "build_story_scene_anchor_card_block",
                "build_story_scene_density_card_block",
                "build_story_repetition_risk_block",
                "build_story_acceptance_card_block",
                "build_story_cliffhanger_card_block",
                "build_story_character_arc_card_block"
            ],
            "shared_inputs": [
                "creative_mode",
                "story_focus",
                "plot_stage"
            ],
            "shared_helpers": [
                "normalized_story_runtime_inputs",
                "build_chapter_combo_text",
                "dedupe_static_prompt_items"
            ]
        },
        "active_consumers": [
            "chapter_generation_prompt_service::build_prompt_params_with_provider_payload",
            "chapter_single_generation_prepare_service",
            "chapter_batch_generation_runtime_state_service"
        ],
        "validation_boundary": [
            "cargo test chapter_generation_prompt_service",
            "cargo check --manifest-path backend-rs/Cargo.toml"
        ],
        "rollback_boundary": {
            "source_map_policy": "production_python_story_prompt_block_builders_deleted_after_rust_owner_validation",
            "split_owner_note": "narrative blueprint plus the full story card prompt builder family are Rust-owned; historical Python parity fixtures live only under backend/tests/test_support/story_prompt_block_test_support.py",
            "runtime_contract": "story card block keys and combination semantics remain stable for shared prompt consumers"
        }
    })
}

#[cfg(test)]
mod tests {
    use super::build_story_card_owner_contract;

    #[test]
    fn should_publish_split_story_card_python_source_map_contract() {
        let contract = build_story_card_owner_contract();

        assert_eq!(
            contract["python_source_map"]
                .as_array()
                .expect("python source map")
                .len(),
            0
        );
        assert_eq!(
            contract["rollback_boundary"]["source_map_policy"],
            "production_python_story_prompt_block_builders_deleted_after_rust_owner_validation"
        );
        assert_eq!(
            contract["rollback_boundary"]["split_owner_note"],
            "narrative blueprint plus the full story card prompt builder family are Rust-owned; historical Python parity fixtures live only under backend/tests/test_support/story_prompt_block_test_support.py"
        );
    }
}
