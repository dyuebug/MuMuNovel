# -*- coding: utf-8 -*-
"""生成第三版提示词融合验收报告（自动化 + 人工联调清单）。

用途：
1. 运行关键回归测试（inspiration/chapters/fusion-coverage）。
2. 静态检查关键模板是否保留第三版追踪标签。
3. 静态检查关键模板是否纳入受管同步规则。
4. 输出 Markdown 与 JSON 报告，供发布前验收留档。
"""

from __future__ import annotations

import argparse
import json
import re
import subprocess
import time
from dataclasses import dataclass
from datetime import datetime
from pathlib import Path
from typing import Dict, List, Tuple


SCRIPT_DIR = Path(__file__).resolve().parent
BACKEND_ROOT = SCRIPT_DIR.parent
REPO_ROOT = BACKEND_ROOT.parent
PROMPT_SERVICE_PATH = BACKEND_ROOT / "app/services/prompt_service.py"
SYNC_RULES_PATH = BACKEND_ROOT / "app/services/prompt_template_sync_service.py"
V3_TAG = "rule_v3_fusion_20260303"

REQUIRED_TEMPLATES = [
    "OUTLINE_CREATE",
    "OUTLINE_CONTINUE",
    "CHAPTER_GENERATION_ONE_TO_MANY",
    "CHAPTER_GENERATION_ONE_TO_MANY_NEXT",
    "CHAPTER_GENERATION_ONE_TO_ONE",
    "CHAPTER_GENERATION_ONE_TO_ONE_NEXT",
    "PARTIAL_REGENERATE",
    "INSPIRATION_TITLE_SYSTEM",
    "INSPIRATION_DESCRIPTION_SYSTEM",
    "INSPIRATION_THEME_SYSTEM",
    "INSPIRATION_GENRE_SYSTEM",
    "INSPIRATION_QUICK_COMPLETE",
    "AI_DENOISING",
    "PLOT_ANALYSIS",
    "OUTLINE_EXPAND_SINGLE",
    "OUTLINE_EXPAND_MULTI",
    "AUTO_CHARACTER_ANALYSIS",
    "AUTO_ORGANIZATION_ANALYSIS",
]

REQUIRED_SYNC_KEYS = {
    "AI_DENOISING",
    "OUTLINE_CREATE",
    "OUTLINE_CONTINUE",
    "CHAPTER_GENERATION_ONE_TO_MANY",
    "CHAPTER_GENERATION_ONE_TO_MANY_NEXT",
    "CHAPTER_GENERATION_ONE_TO_ONE",
    "CHAPTER_GENERATION_ONE_TO_ONE_NEXT",
    "PARTIAL_REGENERATE",
    "PLOT_ANALYSIS",
    "OUTLINE_EXPAND_SINGLE",
    "OUTLINE_EXPAND_MULTI",
    "AUTO_CHARACTER_ANALYSIS",
    "AUTO_ORGANIZATION_ANALYSIS",
    "INSPIRATION_TITLE_SYSTEM",
    "INSPIRATION_DESCRIPTION_SYSTEM",
    "INSPIRATION_THEME_SYSTEM",
    "INSPIRATION_GENRE_SYSTEM",
    "INSPIRATION_QUICK_COMPLETE",
}

DEFAULT_TEST_COMMANDS = [
    [
        "pytest",
        "tests/test_api/test_inspiration.py",
        "tests/test_api/test_chapters.py",
        "tests/test_services/test_prompt_v3_fusion_coverage.py",
    ]
]


@dataclass
class CommandResult:
    command: List[str]
    exit_code: int
    duration_sec: float
    stdout: str
    stderr: str

    @property
    def ok(self) -> bool:
        return self.exit_code == 0


def run_command(command: List[str], cwd: Path) -> CommandResult:
    start = time.time()
    completed = subprocess.run(
        command,
        cwd=str(cwd),
        text=True,
        capture_output=True,
        encoding="utf-8",
        errors="replace",
        check=False,
    )
    return CommandResult(
        command=command,
        exit_code=completed.returncode,
        duration_sec=round(time.time() - start, 2),
        stdout=completed.stdout or "",
        stderr=completed.stderr or "",
    )


def _extract_template_block(source: str, template_key: str) -> str:
    triple_pattern = re.compile(
        rf"^\s*{re.escape(template_key)}\s*=\s*\"\"\"(.*?)\"\"\"",
        re.S | re.M,
    )
    single_pattern = re.compile(
        rf"^\s*{re.escape(template_key)}\s*=\s*\"([^\n]*)\"",
        re.M,
    )

    triple_match = triple_pattern.search(source)
    if triple_match:
        return triple_match.group(1)

    single_match = single_pattern.search(source)
    if single_match:
        return single_match.group(1)

    raise ValueError(f"模板未找到: {template_key}")


def check_template_v3_tags(prompt_service_source: str) -> Tuple[List[str], List[str]]:
    present: List[str] = []
    missing: List[str] = []
    for key in REQUIRED_TEMPLATES:
        try:
            block = _extract_template_block(prompt_service_source, key)
        except ValueError:
            missing.append(key)
            continue
        if V3_TAG in block:
            present.append(key)
        else:
            missing.append(key)
    return present, missing


def check_sync_rules(sync_source: str) -> Tuple[List[str], List[str]]:
    sync_keys = set(re.findall(r'"([A-Z0-9_]+)"\s*:\s*TemplateSyncRule', sync_source))
    present = sorted(k for k in REQUIRED_SYNC_KEYS if k in sync_keys)
    missing = sorted(k for k in REQUIRED_SYNC_KEYS if k not in sync_keys)
    return present, missing


def get_git_context() -> Dict[str, str]:
    branch = run_command(["git", "rev-parse", "--abbrev-ref", "HEAD"], REPO_ROOT)
    commit = run_command(["git", "rev-parse", "HEAD"], REPO_ROOT)
    status = run_command(["git", "status", "--short"], REPO_ROOT)
    return {
        "branch": branch.stdout.strip() if branch.ok else "unknown",
        "commit": commit.stdout.strip() if commit.ok else "unknown",
        "working_tree": status.stdout.strip() if status.ok else "unknown",
    }


def build_markdown_report(
    *,
    timestamp: str,
    git_ctx: Dict[str, str],
    command_results: List[CommandResult],
    template_present: List[str],
    template_missing: List[str],
    sync_present: List[str],
    sync_missing: List[str],
    overall_ok: bool,
) -> str:
    lines: List[str] = []
    lines.append(f"# 第三版提示词融合验收报告（{timestamp}）")
    lines.append("")
    lines.append("## 1. 执行摘要")
    lines.append(f"- 总体结果：{'PASS' if overall_ok else 'FAIL'}")
    lines.append(f"- 分支：`{git_ctx['branch']}`")
    lines.append(f"- 提交：`{git_ctx['commit']}`")
    lines.append("")
    lines.append("## 2. 自动化检查结果")
    lines.append("| 检查项 | 结果 | 说明 |")
    lines.append("|---|---|---|")
    lines.append(
        f"| 关键回归测试 | {'PASS' if all(r.ok for r in command_results) else 'FAIL'} | 共执行 {len(command_results)} 条命令 |"
    )
    lines.append(
        f"| 第三版模板标签覆盖 | {'PASS' if not template_missing else 'FAIL'} | 命中 {len(template_present)}/{len(REQUIRED_TEMPLATES)} |"
    )
    lines.append(
        f"| 同步规则覆盖 | {'PASS' if not sync_missing else 'FAIL'} | 命中 {len(sync_present)}/{len(REQUIRED_SYNC_KEYS)} |"
    )
    lines.append("")
    lines.append("## 3. 命令执行明细")
    for idx, result in enumerate(command_results, start=1):
        lines.append(f"### 3.{idx} `{' '.join(result.command)}`")
        lines.append(f"- 退出码：`{result.exit_code}`")
        lines.append(f"- 耗时：`{result.duration_sec}s`")
        lines.append("")
        lines.append("```text")
        lines.append(result.stdout.strip() or "(no stdout)")
        if result.stderr.strip():
            lines.append("")
            lines.append("[stderr]")
            lines.append(result.stderr.strip())
        lines.append("```")
        lines.append("")

    lines.append("## 4. 覆盖详情")
    lines.append(f"- 模板标签缺失：{template_missing if template_missing else '无'}")
    lines.append(f"- 同步规则缺失：{sync_missing if sync_missing else '无'}")
    lines.append("")
    lines.append("## 5. 人工联调待办")
    lines.append("- 按 `docs/11-第三版提示词融合验收.md` 执行真实模型联调。")
    lines.append("- 核查旧默认模板副本自动同步命中率（重点关注高频用户）。")
    lines.append("- 抽样检查是否仍出现流程化元文本泄漏。")
    lines.append("")
    lines.append("## 6. 工作区状态")
    lines.append("```text")
    lines.append(git_ctx["working_tree"] or "(clean)")
    lines.append("```")
    lines.append("")
    return "\n".join(lines)


def main() -> int:
    parser = argparse.ArgumentParser(description="生成第三版提示词融合验收报告")
    parser.add_argument(
        "--output-dir",
        default=str(BACKEND_ROOT / "logs/qa/v3_fusion"),
        help="报告输出目录（默认: backend/logs/qa/v3_fusion）",
    )
    parser.add_argument(
        "--skip-tests",
        action="store_true",
        help="跳过 pytest 执行，仅生成静态检查报告",
    )
    args = parser.parse_args()

    output_root = Path(args.output_dir)
    timestamp = datetime.now().strftime("%Y-%m-%d_%H-%M-%S")
    run_dir = output_root / timestamp
    run_dir.mkdir(parents=True, exist_ok=True)

    command_results: List[CommandResult] = []
    if not args.skip_tests:
        for command in DEFAULT_TEST_COMMANDS:
            command_results.append(run_command(command, BACKEND_ROOT))

    prompt_source = PROMPT_SERVICE_PATH.read_text(encoding="utf-8")
    sync_source = SYNC_RULES_PATH.read_text(encoding="utf-8")
    template_present, template_missing = check_template_v3_tags(prompt_source)
    sync_present, sync_missing = check_sync_rules(sync_source)

    tests_ok = all(r.ok for r in command_results) if command_results else True
    overall_ok = tests_ok and (not template_missing) and (not sync_missing)
    git_ctx = get_git_context()

    report_markdown = build_markdown_report(
        timestamp=timestamp,
        git_ctx=git_ctx,
        command_results=command_results,
        template_present=template_present,
        template_missing=template_missing,
        sync_present=sync_present,
        sync_missing=sync_missing,
        overall_ok=overall_ok,
    )

    report_md_path = run_dir / "acceptance_report.md"
    report_json_path = run_dir / "acceptance_report.json"
    report_md_path.write_text(report_markdown, encoding="utf-8")

    report_payload = {
        "timestamp": timestamp,
        "overall_ok": overall_ok,
        "git": git_ctx,
        "commands": [
            {
                "command": result.command,
                "exit_code": result.exit_code,
                "duration_sec": result.duration_sec,
                "stdout": result.stdout,
                "stderr": result.stderr,
            }
            for result in command_results
        ],
        "template_tag_check": {
            "required_count": len(REQUIRED_TEMPLATES),
            "present": template_present,
            "missing": template_missing,
        },
        "sync_rule_check": {
            "required_count": len(REQUIRED_SYNC_KEYS),
            "present": sync_present,
            "missing": sync_missing,
        },
    }
    report_json_path.write_text(
        json.dumps(report_payload, ensure_ascii=False, indent=2),
        encoding="utf-8",
    )

    print(f"[v3-fusion] report_dir={run_dir}")
    print(f"[v3-fusion] markdown={report_md_path}")
    print(f"[v3-fusion] json={report_json_path}")
    print(f"[v3-fusion] overall={'PASS' if overall_ok else 'FAIL'}")
    return 0 if overall_ok else 1


if __name__ == "__main__":
    raise SystemExit(main())
