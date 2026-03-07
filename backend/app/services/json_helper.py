"""JSON 处理工具类"""
import json
import re
from typing import Any, Dict, List, Union
from app.logger import get_logger

logger = get_logger(__name__)


_CONTROL_CHAR_ESCAPES = {
    "\b": "\\b",
    "\f": "\\f",
    "\n": "\\n",
    "\r": "\\r",
    "\t": "\\t",
}


def _extract_balanced_json(text: str, start: int) -> str:
    stack = []
    i = start
    end = -1
    in_string = False

    while i < len(text):
        c = text[i]

        if c == '"':
            if not in_string:
                in_string = True
            else:
                num_backslashes = 0
                j = i - 1
                while j >= start and text[j] == '\\':
                    num_backslashes += 1
                    j -= 1

                if num_backslashes % 2 == 0:
                    in_string = False

            i += 1
            continue

        if in_string:
            i += 1
            continue

        if c in ('{', '['):
            stack.append(c)
        elif c == '}':
            if stack and stack[-1] == '{':
                stack.pop()
                if not stack:
                    end = i + 1
                    break
        elif c == ']':
            if stack and stack[-1] == '[':
                stack.pop()
                if not stack:
                    end = i + 1
                    break

        i += 1

    if end > 0:
        return text[start:end]

    return text[start:]


def _escape_invalid_string_chars(text: str) -> str:
    result: List[str] = []
    in_string = False
    escaped = False

    for char in text:
        if in_string:
            if escaped:
                result.append(char)
                escaped = False
                continue

            if char == "\\":
                result.append(char)
                escaped = True
                continue

            if char == '"':
                result.append(char)
                in_string = False
                continue

            if ord(char) < 0x20:
                result.append(_CONTROL_CHAR_ESCAPES.get(char, f"\\u{ord(char):04x}"))
                continue

            result.append(char)
            continue

        result.append(char)
        if char == '"':
            in_string = True

    return "".join(result)


def clean_json_response(text: str) -> str:
    """清洗 AI 返回的 JSON（改进版 - 流式安全）"""
    try:
        if not text:
            logger.warning("⚠️ clean_json_response: 输入为空")
            return text
        
        original_length = len(text)
        logger.debug(f"🔍 开始清洗JSON，原始长度: {original_length}")
        
        # 去除 markdown 代码块
        text = re.sub(r'^```json\s*\n?', '', text, flags=re.MULTILINE | re.IGNORECASE)
        text = re.sub(r'^```\s*\n?', '', text, flags=re.MULTILINE)
        text = re.sub(r'\n?```\s*$', '', text, flags=re.MULTILINE)
        text = text.strip()
        
        if len(text) != original_length:
            logger.debug(f"   移除markdown后长度: {len(text)}")
        
        # 尝试直接解析（快速路径）
        try:
            json.loads(text)
            logger.debug(f"✅ 直接解析成功，无需清洗")
            return text
        except:
            pass
        
        candidate_starts = [index for index, char in enumerate(text) if char in ('{', '[')]

        if not candidate_starts:
            logger.warning(f"⚠️ 未找到JSON起始符号 {{ 或 [")
            logger.debug(f"   文本预览: {text[:200]}")
            return text

        result = text
        best_valid_candidate = ""
        for start in candidate_starts:
            if start > 0:
                logger.debug(f"   尝试从第{start}个字符提取JSON")

            candidate = _extract_balanced_json(text, start)
            repaired_candidate = _escape_invalid_string_chars(candidate)
            if repaired_candidate != candidate:
                logger.warning("⚠️ 检测到字符串中的非法控制字符，已自动转义")
                candidate = repaired_candidate

            try:
                json.loads(candidate)
                if len(candidate) > len(best_valid_candidate):
                    best_valid_candidate = candidate
                continue
            except json.JSONDecodeError:
                continue

        if best_valid_candidate:
            result = best_valid_candidate
            logger.debug(f"✅ JSON清洗完成，结果长度: {len(result)}")
        else:
            start = candidate_starts[0]
            if start > 0:
                logger.debug(f"   跳过前{start}个字符")
            result = _extract_balanced_json(text, start)
            repaired_result = _escape_invalid_string_chars(result)
            if repaired_result != result:
                logger.warning("⚠️ 检测到字符串中的非法控制字符，已自动转义")
                result = repaired_result
            logger.warning(f"⚠️ 未找到可直接解析的JSON片段，返回首个候选（长度: {len(result)}）")
        
        repaired_result = _escape_invalid_string_chars(result)
        if repaired_result != result:
            logger.warning("⚠️ 检测到字符串中的非法控制字符，已自动转义")
            result = repaired_result

        # 验证清洗后的结果
        try:
            json.loads(result)
            logger.debug(f"✅ 清洗后JSON验证成功")
        except json.JSONDecodeError as e:
            logger.error(f"❌ 清洗后JSON仍然无效: {e}")
            logger.debug(f"   结果预览: {result[:500]}")
            logger.debug(f"   结果结尾: ...{result[-200:]}")
        
        return result
        
    except Exception as e:
        logger.error(f"❌ clean_json_response 出错: {e}")
        logger.error(f"   文本长度: {len(text) if text else 0}")
        logger.error(f"   文本预览: {text[:200] if text else 'None'}")
        raise


def parse_json(text: str) -> Union[Dict, List]:
    """解析 JSON"""
    cleaned = ""
    try:
        cleaned = clean_json_response(text)
        return json.loads(cleaned)
    except Exception as e:
        logger.error(f"❌ parse_json 出错: {e}")
        logger.error(f"   原始文本长度: {len(text) if text else 0}")
        logger.error(f"   清洗后文本长度: {len(cleaned) if cleaned else 0}")
        raise
