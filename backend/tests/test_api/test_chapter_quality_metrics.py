from types import SimpleNamespace

from tests.test_support.batch_generation_single_chapter_wiring_test_adapter import (
    _build_chapter_generation_request_options,
    _calculate_chapter_generation_max_tokens,
)
from tests.test_support.story_quality_metrics_aggregation_test_support import (
    extract_outline_anchor_lines,
)
from tests.test_support.story_quality_metrics_aggregation_test_support import (
    compute_story_quality_metrics,
)

chapters_api = SimpleNamespace(
    _calculate_chapter_generation_max_tokens=_calculate_chapter_generation_max_tokens,
    _build_chapter_generation_request_options=_build_chapter_generation_request_options,
    compute_story_quality_metrics=compute_story_quality_metrics,
    _extract_outline_anchor_lines=extract_outline_anchor_lines,
)


def test_should_calculate_chapter_generation_max_tokens_with_tighter_budget():
    assert chapters_api._calculate_chapter_generation_max_tokens(500) == 700
    assert chapters_api._calculate_chapter_generation_max_tokens(1600) == 960
    assert chapters_api._calculate_chapter_generation_max_tokens(3000) == 1800


def test_should_build_generation_request_options_for_responses_provider():
    ai_service = SimpleNamespace(
        api_provider='openai_responses',
        config=SimpleNamespace(retry=SimpleNamespace(max_retries=5)),
    )

    options = chapters_api._build_chapter_generation_request_options(ai_service)

    assert options == {
        'prefer_chat_completions': True,
        'transport_max_retries': 2,
        'first_chunk_timeout': 20.0,
        'allow_non_stream_fallback': False,
    }


def test_should_skip_generation_request_options_for_non_responses_provider():
    ai_service = SimpleNamespace(api_provider='openai')

    assert chapters_api._build_chapter_generation_request_options(ai_service) is None


def test_should_skip_rule_grounding_when_world_rules_missing():
    metrics = chapters_api.compute_story_quality_metrics(
        content='零点刚过，直播间忽然断电。"别接。"林知白伸手来抢手机，楼梯口的脚步声越来越近。',
        chapter_outline=None,
        world_rules=None,
    )

    rule_grounding = metrics["details"]["rule_grounding"]
    assert rule_grounding["applicable"] is False
    assert rule_grounding["expected_count"] == 0
    assert rule_grounding["skipped_reason"] == "no_world_rules"


def test_should_match_outline_alignment_by_multi_token_overlap():
    chapter_outline = """【章节概要】
- 凌晨零点直播间突然断电，沈砚被迫重开直播。
- 林知白推动他前往负一层，寻找四十四号寄存柜里的旧手机。
【关键事件】
- 沈砚利用喷水和导电短路反锁自动扶梯门。
- 死者来电警告不要让林知白看到第一段视频。
"""
    content = """零点的报时刚跳出来，顶层直播间先黑了。沈砚还没来得及关播，林知白已经催他下到负一层，去找四十四号寄存柜里的旧手机。

喷淋炸开后，沈砚借着积水和外露电线做出导电短路，反锁了自动扶梯门。那部旧手机随即响起，电话那头的人只说一句：不要让林知白看到第一段视频。"""

    metrics = chapters_api.compute_story_quality_metrics(
        content=content,
        chapter_outline=chapter_outline,
        world_rules=None,
    )

    outline_alignment = metrics["details"]["outline_alignment"]
    assert outline_alignment["applicable"] is True
    assert outline_alignment["hit_count"] >= 1
    assert outline_alignment["matched_anchors"]
    assert metrics["outline_alignment_rate"] > 0.0



def test_should_fallback_to_outline_rule_hints_when_world_rules_missing():
    chapter_outline = """\u3010\u89c4\u5219\u5f71\u54cd\u70b9\u3011
- 回检规则触发后会迫使接触者登记，否则现实被改写。
"""
    content = "他刚念出第一页，回检规则触发后会迫使接触者登记，因此他只能先掀掉直播画面。"

    metrics = chapters_api.compute_story_quality_metrics(
        content=content,
        chapter_outline=chapter_outline,
        world_rules=None,
    )

    rule_grounding = metrics["details"]["rule_grounding"]
    assert rule_grounding["applicable"] is True
    assert rule_grounding["hit_count"] >= 1
    assert "skipped_reason" not in rule_grounding
    assert metrics["rule_grounding_hit_rate"] > 0.0



def test_should_detect_conflict_chain_for_trapped_choice_and_cost_sequence():
    content = (
        "铁链猛地收紧，直接把他困住在站台中央。"
        "他只能硬着头皮当场重念第一行文字。"
        "结果回检速度被拖慢，他的位置也因此暴露。"
    )

    metrics = chapters_api.compute_story_quality_metrics(
        content=content,
        chapter_outline=None,
        world_rules=None,
    )

    conflict = metrics["details"]["conflict_chain"]
    assert conflict["applicable"] is True
    assert conflict["hit_count"] >= 1
    assert metrics["conflict_chain_hit_rate"] > 0.0



def test_should_detect_conflict_chain_when_obstacle_and_choice_share_same_sentence():
    content = (
        "规则立刻卡住了所有人的选择，闻川只能咬牙用校对笔改写句子。"
        "代价是他自己身上的字痕继续往上爬，位置也彻底暴露。"
    )

    metrics = chapters_api.compute_story_quality_metrics(
        content=content,
        chapter_outline=None,
        world_rules=None,
    )

    conflict = metrics["details"]["conflict_chain"]
    assert conflict["hit_count"] >= 1
    assert metrics["conflict_chain_hit_rate"] > 0.0



def test_should_detect_rule_grounding_from_rule_cue_when_outline_rules_exist():
    chapter_outline = """\u3010\u89c4\u5219\u5f71\u54cd\u70b9\u3011
- 异常文本连续读满特定行数后，现实会被改写。
"""
    content = "规则立刻卡住了所有人的选择，因此他们只能先断电封锁。"

    metrics = chapters_api.compute_story_quality_metrics(
        content=content,
        chapter_outline=chapter_outline,
        world_rules=None,
    )

    rule_grounding = metrics["details"]["rule_grounding"]
    assert rule_grounding["applicable"] is True
    assert rule_grounding["hit_rate"] > 0.0
    assert metrics["rule_grounding_hit_rate"] > 0.0



def test_should_match_outline_alignment_for_prose_outline_summary():
    chapter_outline = (
        "闻折的目标很简单，做完重检拿钱走人。"
        "阻力却来得很快，林照认定他身上带着未登记的页印。"
        "反馈立刻到来，地板下弹出染水的借书证，直播间弹幕刷满“开始校对第一页”。"
    )
    content = (
        "他只想做完重检拿钱走人，可刚进门就被逼到不能后退。"
        "林照盯着他手背的旧伤，认定那就是未登记的页印。"
        "等他掀开木板，地板下弹出一张染水的借书证，直播间弹幕只剩“开始校对第一页”。"
    )

    metrics = chapters_api.compute_story_quality_metrics(
        content=content,
        chapter_outline=chapter_outline,
        world_rules=None,
    )

    outline_alignment = metrics["details"]["outline_alignment"]
    assert outline_alignment["applicable"] is True
    assert outline_alignment["hit_count"] >= 1
    assert metrics["outline_alignment_rate"] > 0.0





def test_should_prioritize_structured_outline_bullets_over_section_headers():
    chapter_outline = """【章节概要】
- 旧城区停电，沈见川被迫边修手机边盯着异常直播。
【场景设定】
- 老周坚持报警，沈见川坚持先按直播避险。
- 许雾突然闯进来抢走手机，又折返回店里。
【情节要点】
- 世界规则首次落地，提前看到的内容不能被直接说破。
- 目标受阻，沈见川想赚修机钱并脱身，却被异常拖住。
- 核心配角许雾先抢手机像是敌人，随后却反预期折返救人，因为她需要锚点活着。
- 章尾钩子是直播画面里出现沈见川的尸体。
【叙事目标】
- 建立主角与关键配角的被迫同盟。
"""

    anchors = chapters_api._extract_outline_anchor_lines(chapter_outline, max_lines=6)

    assert anchors
    assert all(not anchor.endswith((":", "：")) for anchor in anchors)
    assert any("许雾" in anchor and "锚点" in anchor for anchor in anchors)


def test_should_detect_conflict_chain_for_blocked_retreat_choice_and_cost():
    content = (
        "周启一把扣住门框，说今晚谁都别进。"
        "撤出去账号也一样死，闻折还是抢过备用镜头，边播边往里走。"
        "代价是回声很快锁定了他的声音，退路也被当场锁死。"
    )

    metrics = chapters_api.compute_story_quality_metrics(
        content=content,
        chapter_outline=None,
        world_rules=None,
    )

    conflict = metrics["details"]["conflict_chain"]
    assert conflict["applicable"] is True
    assert conflict["hit_count"] >= 1
    assert metrics["conflict_chain_hit_rate"] > 0.0



def test_should_detect_payoff_chain_from_outline_hints_and_feedback():
    chapter_outline = (
        "前面埋下页印和旧直播机位两个钩子。"
        "小爽点是闻折找到旧直播机位，逼得回声稳定下来。"
        "反馈是地板下弹出借书证，直播间弹幕刷满“开始校对第一页”。"
    )
    content = (
        "林照早就认定他身上有页印，旧直播机位一直被藏在儿童阅览区下面。"
        "闻折猛地踹开书架，终于找到旧直播机位，逼得回声画面稳定下来。"
        "紧接着借书证从地板下弹出，观众数瞬间破万，弹幕只剩“开始校对第一页”。"
    )

    metrics = chapters_api.compute_story_quality_metrics(
        content=content,
        chapter_outline=chapter_outline,
        world_rules=None,
        quality_runtime_context={"foreshadow_payoff_plan": []},
    )

    payoff = metrics["details"]["payoff_chain"]
    assert payoff["applicable"] is True
    assert payoff["hit_count"] >= 1
    assert metrics["payoff_chain_rate"] > 0.0



def test_should_detect_opening_hook_for_error_code_and_live_incident():
    content = (
        "凌晨便利店快打烊时，所有屏幕一起亮了一下。"
        "猴红字幕压到画面正中：欢迎进入现场复核，错误编号1774721191。"
        "那是三年前那场直播事故的存档号，也是七名观众失踪的起点。"
    )

    metrics = chapters_api.compute_story_quality_metrics(
        content=content,
        chapter_outline=None,
        world_rules=None,
    )

    opening = metrics["details"]["opening_hook"]
    assert opening["applicable"] is True
    assert opening["matched_markers"]
    assert metrics["opening_hook_rate"] >= 50.0



def test_should_detect_opening_hook_for_recheck_acceptance_and_reality_rewrite():
    content = (
        "\u51cc\u6668\u4e24\u70b9\uff0c\u4ed6\u628a\u65e7\u75c5\u5386\u538b\u8fdb\u626b\u63cf\u673a\u3002"
        "\u5c4f\u5e55\u4e0a\u7329\u7ea2\u6821\u9a8c\u5b57\u731b\u5730\u8df3\u51fa\uff1a\u3010\u91cd\u68c0\u7533\u8bf7\u5df2\u53d7\u7406\uff1a\u5bf9\u8c61\uff0c\u6c88\u781a\u3002\u3011"
        "\u6302\u5386\u5f00\u59cb\u5012\u9000\uff0c\u62a5\u7eb8\u6807\u9898\u76f8\u4e92\u541e\u5b57\uff0c\u4ed6\u624b\u80cc\u7684\u7584\u75d5\u4e00\u70b9\u70b9\u53d8\u6d45\u3002"
    )

    metrics = chapters_api.compute_story_quality_metrics(
        content=content,
        chapter_outline=None,
        world_rules=None,
    )

    opening = metrics["details"]["opening_hook"]
    assert opening["applicable"] is True
    assert "\u5f02\u5e38" in opening["matched_markers"]
    assert "\u5371\u9669" in opening["matched_markers"]
    assert metrics["opening_hook_rate"] >= 67.0


def test_should_detect_cliffhanger_for_deadline_and_citywide_release():
    content = (
        "卷帘门外的拍门声整齐地响起，三年前失踪的观众一个不少地站在玻璃外。"
        "平板震动，复核界面跳出新提示：主持人请于二十分钟内前往原始拍摄地完成现场更正。"
        "逾时未更正，本次复核将默认全城公开。"
    )

    metrics = chapters_api.compute_story_quality_metrics(
        content=content,
        chapter_outline=None,
        world_rules=None,
    )

    cliffhanger = metrics["details"]["cliffhanger"]
    assert cliffhanger["applicable"] is True
    assert cliffhanger["matched_markers"]
    assert metrics["cliffhanger_rate"] > 0.0


def test_should_detect_cliffhanger_for_locked_exit_and_page_one_trigger():
    content = (
        "借阅日期写着：明天。"
        "书认主了。今晚谁都不能让他出去。"
        "倒计时归零。"
        "观众数疯了一样往上跳，瞬间破万。"
        "弹幕没有名字，只有同一句话，密密麻麻往上滚。"
        "开始校对第一页。"
    )

    metrics = chapters_api.compute_story_quality_metrics(
        content=content,
        chapter_outline=None,
        world_rules=None,
    )

    cliffhanger = metrics["details"]["cliffhanger"]
    assert cliffhanger["applicable"] is True
    assert len(cliffhanger["matched_markers"]) >= 2
    assert metrics["cliffhanger_rate"] >= 67.0


def test_should_detect_cliffhanger_for_second_review_and_witness_joined():
    content = (
        "\u4ed6\u770b\u89c1\u56de\u6267\u80cc\u9762\u6d6e\u51fa\u4e00\u884c\u65b0\u7684\u6eda\u52a8\u5b57\uff0c\u50cf\u6709\u4eba\u9694\u7a7a\u5728\u53e6\u4e00\u7aef\u5b9e\u65f6\u6279\u6ce8\u3002"
        "\u4e0a\u9762\u5199\u7740\uff0c\u91cd\u68c0\u9a73\u56de\u5931\u8d25\uff0c\u8fdb\u5165\u7b2c\u4e8c\u8f6e\u590d\u6838\uff0c\u89c1\u8bc1\u4eba\u5df2\u52a0\u5165\u3002"
    )

    metrics = chapters_api.compute_story_quality_metrics(
        content=content,
        chapter_outline=None,
        world_rules=None,
    )

    cliffhanger = metrics["details"]["cliffhanger"]
    assert cliffhanger["applicable"] is True
    assert len(cliffhanger["matched_markers"]) >= 2
    assert metrics["cliffhanger_rate"] >= 67.0


def test_should_detect_opening_hook_for_hot_search_warning_and_identity_confirmation():
    content = (
        "热榜忽然被一条红色提示顶穿，消防通道门口拉起警戒线，直播画面却还在往里晃。"
        "屏幕中央弹窗，只有一句话：请确认母本身份。"
    )

    metrics = chapters_api.compute_story_quality_metrics(
        content=content,
        chapter_outline=None,
        world_rules=None,
    )

    opening_hook = metrics["details"]["opening_hook"]
    assert opening_hook["applicable"] is True
    assert "异常" in opening_hook["matched_markers"]
    assert "任务" in opening_hook["matched_markers"]
    assert metrics["opening_hook_rate"] >= 67.0


def test_should_detect_cliffhanger_for_new_annotation_and_public_recertification():
    content = (
        "屏幕最后跳出新的标注：异常叙事追认记录已开启。"
        "下一行紧跟着写着：见证人被追认为源头。"
        "楼下广播同时开始全网同步。"
    )

    metrics = chapters_api.compute_story_quality_metrics(
        content=content,
        chapter_outline=None,
        world_rules=None,
    )

    cliffhanger = metrics["details"]["cliffhanger"]
    assert cliffhanger["applicable"] is True
    assert "info_gap" in cliffhanger["matched_markers"]
    assert "identity_twist" in cliffhanger["matched_markers"]
    assert metrics["cliffhanger_rate"] >= 67.0


def test_should_detect_cliffhanger_for_self_lit_phone_and_past_timestamp_message():
    content = (
        "空气一下收紧。"
        "周祁口袋里的备用机自己亮了，锁屏弹出一条定时消息。"
        "发件人：周祁。"
        "发送时间：三年前，澄河商场火灾当晚。"
    )

    metrics = chapters_api.compute_story_quality_metrics(
        content=content,
        chapter_outline=None,
        world_rules=None,
    )

    cliffhanger = metrics["details"]["cliffhanger"]
    assert cliffhanger["applicable"] is True
    assert "info_gap" in cliffhanger["matched_markers"]
    assert "danger" in cliffhanger["matched_markers"]
    assert metrics["cliffhanger_rate"] >= 67.0


def test_should_detect_cliffhanger_for_empty_owner_info_and_next_preview():
    content = (
        "死角里立着一部旧手机，镜头正对他们，没人碰，屏幕却自己亮起。"
        "机主信息全空，下一段预告自动跳出。"
        "楼梯间里，林见川站在正中，胸口一片血。"
    )

    metrics = chapters_api.compute_story_quality_metrics(
        content=content,
        chapter_outline=None,
        world_rules=None,
    )

    cliffhanger = metrics["details"]["cliffhanger"]
    assert cliffhanger["applicable"] is True
    assert "info_gap" in cliffhanger["matched_markers"]
    assert "danger" in cliffhanger["matched_markers"]
    assert metrics["cliffhanger_rate"] >= 67.0


def test_should_detect_cliffhanger_for_voice_behind_and_real_figure_manifesting():
    content = (
        "林叙刚喘了口气，耳边却响起一个很轻的女声。"
        "不是手机里出来的。"
        "是在他背后。"
        "他猛地转身。扶梯口那片黑里，真的站着一个同样穿校服的女孩，胸口一片血。"
    )

    metrics = chapters_api.compute_story_quality_metrics(
        content=content,
        chapter_outline=None,
        world_rules=None,
    )

    cliffhanger = metrics["details"]["cliffhanger"]
    assert cliffhanger["applicable"] is True
    assert "info_gap" in cliffhanger["matched_markers"]
    assert metrics["cliffhanger_rate"] >= 67.0


def test_should_detect_rule_grounding_for_second_evidence_and_suspended_ban():
    content = (
        "许栀语速飞快：三源交叉，直播、运动相机、反光镜，得让它同时落到第二媒介上，平台才认。"
        "林叙照做后，系统提示猛地跳出：第二证据成立。"
        "封禁倒计时暂缓。"
    )

    metrics = chapters_api.compute_story_quality_metrics(
        content=content,
        chapter_outline=None,
        world_rules="异象直播需要第二媒介交叉校验，平台不认单一链路，违规会触发封禁倒计时。",
    )

    rule_grounding = metrics["details"]["rule_grounding"]
    assert rule_grounding["applicable"] is True
    assert rule_grounding["hit_count"] >= 1
    assert metrics["rule_grounding_hit_rate"] >= 66.0


def test_should_detect_cliffhanger_for_name_reveal_and_system_escalation():
    content = (
        "\u63d0\u793a\u521a\u5f39\u51fa\uff0c\u5c31\u88ab\u65b0\u7684\u7ea2\u5b57\u9876\u6389\u3002"
        "\u5de5\u724c\u4e0a\u7684\u540d\u5b57\u88ab\u955c\u5934\u62c9\u6e05\u3002"
        "\u955c\u4e2d\u4eba\u80f8\u524d\u7684\u65e7\u5de5\u724c\u53ea\u5269\u4e24\u4e2a\u5b57\uff1a\u6c88\u4e34\u3002"
        "\u800c\u540e\u53f0\u8fd8\u5728\u5237\u65b0\uff1a\u3010\u4e8c\u7ea7\u5173\u6ce8\u5bf9\u8c61\u5df2\u6807\u8bb0\u3011\u3010\u57ce\u52a1\u53f8\u91cd\u68c0\u79d1\u63a5\u5165\u4e2d\u2026\u2026\u3011"
    )

    metrics = chapters_api.compute_story_quality_metrics(
        content=content,
        chapter_outline=None,
        world_rules=None,
    )

    cliffhanger = metrics["details"]["cliffhanger"]
    assert cliffhanger["applicable"] is True
    assert len(cliffhanger["matched_markers"]) >= 2
    assert metrics["cliffhanger_rate"] >= 67.0


def test_should_detect_live_recheck_bridge_metrics():
    content = (
        "见证人数超过阈值，异常固化倒计时：06:59。"
        "许见川把手机重新举正：‘借他们的脑子，给这玩意编个解释。’"
        "他对着镜头硬挤出笑，说让他们信这不是鬼门。"
        "在线人数掉到一百五十九，红字边缘开始发灰。"
        "二次复核启动，解释成立度61%，触发底稿追索。"
        "绑定见证人：许见川。"
        "屏幕里的红字重新渗了出来，鲜得刺眼。"
    )

    metrics = chapters_api.compute_story_quality_metrics(
        content=content,
        chapter_outline=None,
        world_rules=None,
    )

    assert metrics["details"]["conflict_chain"]["hit_count"] >= 1
    assert metrics["details"]["payoff_chain"]["hit_count"] >= 1
    assert len(metrics["details"]["cliffhanger"]["matched_markers"]) >= 2
    assert metrics["conflict_chain_hit_rate"] > 0.0
    assert metrics["payoff_chain_rate"] > 0.0
    assert metrics["cliffhanger_rate"] >= 67.0


def test_should_detect_rule_grounding_for_live_contract_and_error_duplicate():
    content = (
        "\u3010\u6821\u9a8c\u5f00\u59cb\uff1a\u4e3b\u64ad\u4e0d\u5f97\u8f6c\u8ff0\u672a\u7ecf\u76ee\u51fb\u7684\u4fe1\u606f\uff0c\u8fdd\u8005\u5931\u58f0\u3002\u3011"
        "\u76f4\u64ad\u753b\u9762\u5fc5\u987b\u4e0e\u73b0\u5b9e\u5bf9\u5e94\uff0c\u9519\u8ba4\u4e00\u6b21\uff0c\u5c31\u4f1a\u591a\u51fa\u4e00\u4e2a\u9519\u8bef\u526f\u672c\u3002"
        "\u4ed6\u5f20\u53e3\u8f6c\u8ff0\uff0c\u55d3\u5b50\u679c\u7136\u50cf\u88ab\u522e\u8fc7\u4e00\u6837\u5f53\u573a\u5931\u58f0\u3002"
    )
    world_rules = (
        "\u6d3b\u9875\u534f\u8bae\u4f1a\u9650\u5236\u4e3b\u64ad\u4e0d\u5f97\u8f6c\u8ff0\u672a\u76ee\u51fb\u4fe1\u606f\uff0c\u8fdd\u8005\u5931\u58f0\u3002"
        "\u76f4\u64ad\u753b\u9762\u5fc5\u987b\u4e0e\u73b0\u5b9e\u5bf9\u5e94\uff0c\u9519\u8ba4\u4f1a\u751f\u6210\u9519\u8bef\u526f\u672c\u3002"
    )

    metrics = chapters_api.compute_story_quality_metrics(
        content=content,
        chapter_outline=None,
        world_rules=world_rules,
    )

    rule_grounding = metrics["details"]["rule_grounding"]
    assert rule_grounding["applicable"] is True
    assert rule_grounding["hit_count"] >= 1
    assert metrics["rule_grounding_hit_rate"] >= 66.0


def test_should_detect_cliffhanger_for_wrong_name_and_second_mismatch():
    content = (
        "\u5468\u6155\u76ef\u7740\u955c\u5934\u8bf4\uff0c\u4f60\u7ec8\u4e8e\u627e\u5230\u7b2c\u4e00\u4e2a\u9519\u4f4d\u8005\u4e86\uff0c\u90a3\u7b2c\u4e8c\u4e2a\uff0c\u4e3a\u4ec0\u4e48\u662f\u4f60\u3002"
        "\u95fb\u4fee\u80f8\u524d\u5de5\u4f5c\u8bc1\u7684\u5851\u5c01\u8fb9\u81ea\u5df1\u7ffb\u5f00\uff0c\u91cc\u9762\u9732\u51fa\u4e00\u5f20\u65e7\u7248\u7ad9\u52a1\u5458\u8bc1\u4ef6\u7167\u3002"
        "\u7167\u7247\u4e0a\u7684\u4eba\u662f\u4ed6\uff0c\u540d\u5b57\u5374\u4e0d\u662f\u95fb\u4fee\u3002"
    )

    metrics = chapters_api.compute_story_quality_metrics(
        content=content,
        chapter_outline=None,
        world_rules=None,
    )

    cliffhanger = metrics["details"]["cliffhanger"]
    assert cliffhanger["applicable"] is True
    assert "identity_twist" in cliffhanger["matched_markers"]
    assert metrics["cliffhanger_rate"] >= 67.0


def test_should_detect_rule_grounding_and_cliffhanger_for_public_reading_trigger():
    content = (
        "只要文件被正式宣读，他就是见证人，反噬会先咬他。"
        "公证处二楼白灯一亮，就是审读准备。"
        "谁在里面把脏句子念全，现实就得照着改。"
        "审读室亮起一片惨白。别让他们读。"
    )
    world_rules = (
        "公开宣读会触发现实改写，反噬会先落在见证人身上。"
        "白灯一亮代表审读开始。"
    )

    metrics = chapters_api.compute_story_quality_metrics(
        content=content,
        chapter_outline=None,
        world_rules=world_rules,
    )

    assert metrics["rule_grounding_hit_rate"] > 0.0
    assert metrics["cliffhanger_rate"] >= 67.0
    assert len(metrics["details"]["cliffhanger"]["matched_markers"]) >= 2


def test_should_detect_cliffhanger_for_upload_port_and_forced_choice():
    content = (
        "图样一样。"
        "上传端不在商场，在——"
        "开始校对第一页。"
        "现在要么你继续改，要么我上去抓人。"
        "选，快。"
    )

    metrics = chapters_api.compute_story_quality_metrics(
        content=content,
        chapter_outline=None,
        world_rules=None,
    )

    cliffhanger = metrics["details"]["cliffhanger"]
    assert "info_gap" in cliffhanger["matched_markers"]
    assert "choice_pending" in cliffhanger["matched_markers"]
    assert metrics["cliffhanger_rate"] >= 67.0


def test_should_detect_dialogue_pressure_for_short_urgent_quotes():
    content = (
        '"你谁？"'
        '"还我。"'
        '"还你，你继续浪费七分钟？"'
        '"为什么非得我碰？"'
        '"因为死的是你。"'
        '"低头！"'
        '"别挂。"'
        '"现在选，快。"'
    )

    metrics = chapters_api.compute_story_quality_metrics(
        content=content,
        chapter_outline=None,
        world_rules=None,
    )

    dialogue = metrics["details"]["dialogue"]
    assert dialogue["pressure_ratio"] > 0.0
    assert metrics["dialogue_naturalness_rate"] >= 69.0


def test_should_detect_rule_grounding_for_heat_amplification_and_backlash():
    chapter_outline = """【规则影响点】
- 多人同步观看会让异常从幻象变成可伤人的实体，热度没断前强拆会反噬。
"""
    content = (
        "人数破万，玻璃上的黑影开始往外长。"
        "怪事一旦被十个人同时看见、同时记下来，就不再只是幻觉。"
        "热度没断，强拆会反噬，只能先控它。"
    )

    metrics = chapters_api.compute_story_quality_metrics(
        content=content,
        chapter_outline=chapter_outline,
        world_rules=None,
    )

    rule_grounding = metrics["details"]["rule_grounding"]
    assert rule_grounding["applicable"] is True
    assert rule_grounding["hit_count"] >= 1
    assert metrics["rule_grounding_hit_rate"] >= 66.0



def test_should_detect_conflict_chain_for_heat_lock_and_binding_cost():
    content = (
        "他想马上断流，可手机彻底失灵，直播间人数也瞬间破万。"
        "热度没断，强拆会反噬。"
        "他只能把镜头对准玻璃，硬逼那团黑影后退。"
        "代价是断页跟他的账号绑上了，住址也被一起翻了出来。"
    )

    metrics = chapters_api.compute_story_quality_metrics(
        content=content,
        chapter_outline=None,
        world_rules=None,
    )

    conflict = metrics["details"]["conflict_chain"]
    assert conflict["applicable"] is True
    assert conflict["hit_count"] >= 1
    assert metrics["conflict_chain_hit_rate"] > 0.0



def test_should_detect_cliffhanger_for_address_deadline_and_home_risk():
    content = (
        "断页认主后，屏幕上慢慢浮出陌生地址和时间—04:44。"
        "“天亮前去这儿。晚一步，它先到你家。”"
        "封控线外有人大喊热度回来了，手机里的那串字又渗出新的血。"
    )

    metrics = chapters_api.compute_story_quality_metrics(
        content=content,
        chapter_outline=None,
        world_rules=None,
    )

    cliffhanger = metrics["details"]["cliffhanger"]
    assert cliffhanger["applicable"] is True
    assert "choice_pending" in cliffhanger["matched_markers"]
    assert metrics["cliffhanger_rate"] >= 67.0


def test_should_detect_cliffhanger_for_countdown_identity_confirmation():
    content = (
        "商场所有屏幕切成同一张旧照片。"
        "那孩子是他，女人是他母亲年轻时的样子。"
        "直播界面再次刷新，猙红倒计时从六十秒开始跳。"
        "新的任务只有一行字【请确认母本身份】。"
    )

    metrics = chapters_api.compute_story_quality_metrics(
        content=content,
        chapter_outline=None,
        world_rules=None,
    )

    cliffhanger = metrics["details"]["cliffhanger"]
    assert cliffhanger["applicable"] is True
    assert "choice_pending" in cliffhanger["matched_markers"]
    assert metrics["cliffhanger_rate"] >= 67.0


def test_should_detect_opening_hook_for_anomalous_live_feed_without_dead_words():
    content = (
        "十一点四十七分，闻川蹲在旧城区天桥下，手机支在膝盖上，嘴里还在念今天的开场词。"
        "桥上堵车，桥下便利店的灯牌闪得发白，按理说镜头里该是这副烂俗夜景。"
        "可直播画面一顿，街景被硬生生切成了另一块——一个空十字路口，红灯亮着，路中央连个人影都没有。"
        "他低头看导航，附近五百米内根本没这个路口。"
        "弹幕整屏刷过去：别回头，先校验。"
    )

    metrics = chapters_api.compute_story_quality_metrics(
        content=content,
        chapter_outline=None,
        world_rules=None,
    )

    opening_hook = metrics["details"]["opening_hook"]
    assert opening_hook["applicable"] is True
    assert "异常" in opening_hook["matched_markers"]
    assert metrics["opening_hook_rate"] >= 50.0


def test_should_detect_cliffhanger_for_locked_witness_and_double_self_reveal():
    content = (
        "监控屏幕轻轻闪了下，画面自己倒退三秒。"
        "门外多出来的路口边，站着另一个闻川。"
        "那人和他穿一样的外套，隔着监控抬起手指压在唇前。"
        "座机突然自己亮起，来电显示只有一行字：城市异常处理中心：见证人已锁定。"
    )

    metrics = chapters_api.compute_story_quality_metrics(
        content=content,
        chapter_outline=None,
        world_rules=None,
    )

    cliffhanger = metrics["details"]["cliffhanger"]
    assert cliffhanger["applicable"] is True
    assert "info_gap" in cliffhanger["matched_markers"]
    assert "identity_twist" in cliffhanger["matched_markers"]
    assert metrics["cliffhanger_rate"] >= 67.0


def test_should_detect_rule_grounding_for_live_calibration_constraint_chain():
    chapter_outline = """【规则影响点】
- 直播画面必须通过现实校验，否则不只是封号，异象还会顺着错误画面反咬主播。
- 现场复核需要把同一目标拉进两条实时链，才能钉出异常破绽。
"""
    content = (
        "后台立刻切进校验页。"
        "你拍到的东西，别的镜头也得站得住。"
        "过不了，不只是封号，异象还可能顺着错误画面反咬主播。"
        "闻修只好冲向封闭通道，去做现场复核。"
    )

    metrics = chapters_api.compute_story_quality_metrics(
        content=content,
        chapter_outline=chapter_outline,
        world_rules=None,
    )

    rule_grounding = metrics["details"]["rule_grounding"]
    assert rule_grounding["applicable"] is True
    assert rule_grounding["hit_count"] >= 1
    assert metrics["rule_grounding_hit_rate"] >= 50.0


def test_should_detect_cliffhanger_for_second_round_calibration_and_mismatch_headcount():
    content = (
        "旧钟后面传来沉闷的机械响。"
        "直播标题自动刷新，红字比刚才更刺眼。"
        "【第二轮现实校验已开启】"
        "许枚冲到街口，猛地抬头。"
        "她手机直播画面里，钟楼窗内站着三个林检。"
        "可现实的窗后，分明只有两个人。"
    )

    metrics = chapters_api.compute_story_quality_metrics(
        content=content,
        chapter_outline=None,
        world_rules=None,
    )

    cliffhanger = metrics["details"]["cliffhanger"]
    assert cliffhanger["applicable"] is True
    assert "info_gap" in cliffhanger["matched_markers"]
    assert "escalation" in cliffhanger["matched_markers"]
    assert metrics["cliffhanger_rate"] >= 67.0
