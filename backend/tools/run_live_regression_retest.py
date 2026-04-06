
import json
import time
import http.client
import http.cookiejar
import urllib.request
import urllib.error
from pathlib import Path

BASE_URL = 'http://localhost:8003'
ROOT = Path(__file__).resolve().parents[2]
TMP_LIVE_DIR = ROOT / 'tmp' / 'live'
ENV_PATH = ROOT / '.env'


def load_env_value(key: str) -> str:
    for line in ENV_PATH.read_text(encoding='utf-8').splitlines():
        if not line or line.lstrip().startswith('#') or '=' not in line:
            continue
        current_key, value = line.split('=', 1)
        if current_key.strip() == key:
            return value.strip().strip('"').strip("'")
    raise KeyError(key)


USERNAME = load_env_value('LOCAL_AUTH_USERNAME')
PASSWORD = load_env_value('LOCAL_AUTH_PASSWORD')

cookie_jar = http.cookiejar.CookieJar()
opener = urllib.request.build_opener(urllib.request.HTTPCookieProcessor(cookie_jar))
opener.addheaders = [('User-Agent', 'codex-live-retest/1.0')]


def request(method: str, path: str, payload=None, timeout=60):
    url = BASE_URL + path
    headers = {}
    data = None
    if payload is not None:
        data = json.dumps(payload, ensure_ascii=False).encode('utf-8')
        headers['Content-Type'] = 'application/json'
    req = urllib.request.Request(url, data=data, headers=headers, method=method.upper())
    started = time.perf_counter()
    incomplete_read = False
    incomplete_read_error = ''
    try:
        with opener.open(req, timeout=timeout) as resp:
            try:
                raw = resp.read()
            except http.client.IncompleteRead as exc:
                raw = exc.partial or b''
                incomplete_read = True
                incomplete_read_error = str(exc)
            status = resp.getcode()
            content_type = resp.headers.get('Content-Type', '')
    except urllib.error.HTTPError as exc:
        raw = exc.read()
        status = exc.code
        content_type = exc.headers.get('Content-Type', '')
    elapsed_ms = round((time.perf_counter() - started) * 1000, 2)
    text = raw.decode('utf-8', errors='replace')
    if 'application/json' in content_type:
        try:
            body = json.loads(text)
        except json.JSONDecodeError:
            body = text
    else:
        body = text
    return {
        'status_code': status,
        'elapsed_ms': elapsed_ms,
        'content_type': content_type,
        'body': body,
        'body_length': len(text),
        'incomplete_read': incomplete_read,
        'incomplete_read_error': incomplete_read_error,
    }


def parse_sse_text(text: str):
    events = []
    current_lines = []
    for raw_line in text.splitlines():
        line = raw_line.rstrip('\r')
        if not line:
            if current_lines:
                event = {}
                data_lines = []
                for item in current_lines:
                    if item.startswith('event:'):
                        event['event'] = item[6:].strip()
                    elif item.startswith('data:'):
                        data_lines.append(item[5:].strip())
                if data_lines:
                    payload_text = '\n'.join(data_lines)
                    try:
                        payload = json.loads(payload_text)
                    except json.JSONDecodeError:
                        payload = payload_text
                    if isinstance(payload, dict):
                        event.update(payload)
                    else:
                        event['data'] = payload
                events.append(event)
                current_lines = []
            continue
        current_lines.append(line)
    if current_lines:
        event = {}
        data_lines = []
        for item in current_lines:
            if item.startswith('event:'):
                event['event'] = item[6:].strip()
            elif item.startswith('data:'):
                data_lines.append(item[5:].strip())
        if data_lines:
            payload_text = '\n'.join(data_lines)
            try:
                payload = json.loads(payload_text)
            except json.JSONDecodeError:
                payload = payload_text
            if isinstance(payload, dict):
                event.update(payload)
            else:
                event['data'] = payload
        events.append(event)
    return events


def request_sse(path: str, payload, timeout=240):
    response = request('POST', path, payload=payload, timeout=timeout)
    text = response['body'] if isinstance(response['body'], str) else ''
    events = parse_sse_text(text)
    chunk_text = ''.join(
        event.get('content')
        for event in events
        if event.get('type') == 'chunk' and isinstance(event.get('content'), str)
    )
    chunk_json = None
    if chunk_text.strip():
        try:
            chunk_json = json.loads(chunk_text)
        except json.JSONDecodeError:
            chunk_json = None
    response['event_count'] = len(events)
    response['event_types'] = [event.get('type') for event in events]
    response['last_progress'] = next((event for event in reversed(events) if event.get('type') == 'progress'), None)
    response['result'] = next((event for event in reversed(events) if event.get('type') == 'result'), None)
    response['error_event'] = next((event for event in reversed(events) if event.get('type') == 'error'), None)
    response['done_event'] = next((event for event in reversed(events) if event.get('type') == 'done'), None)
    response['quality_metrics_event'] = next((event for event in reversed(events) if event.get('type') == 'quality_metrics'), None)
    response['quality_gate_blocked_event'] = next((event for event in reversed(events) if event.get('type') == 'quality_gate_blocked'), None)
    response['chunk_text_length'] = len(chunk_text)
    response['chunk_json'] = chunk_json
    return response


def normalize_outline_items(payload):
    if isinstance(payload, list):
        return [item for item in payload if isinstance(item, dict)]
    if isinstance(payload, dict):
        nested_data = payload.get('data') if isinstance(payload.get('data'), dict) else {}
        for candidate in (payload.get('outlines'), nested_data.get('outlines')):
            if isinstance(candidate, list):
                return [item for item in candidate if isinstance(item, dict)]
        if payload.get('id'):
            return [payload]
    return []


def fetch_outline_list(project_id: str, *, attempts: int = 10, delay_seconds: float = 2.0):
    last_response = None
    last_items = []
    for attempt_index in range(attempts):
        last_response = request('GET', f'/api/outlines/project/{project_id}', timeout=30)
        last_items = normalize_outline_items(last_response.get('body'))
        if last_items:
            return last_response, last_items
        if attempt_index + 1 < attempts:
            time.sleep(delay_seconds)
    return last_response or {'body': []}, last_items


def extract_counts(outline_body):
    if not isinstance(outline_body, dict):
        return 0, 0
    char_count = 0
    org_count = 0
    for key in ('characters', 'characters_context', 'auto_characters'):
        value = outline_body.get(key)
        if isinstance(value, list):
            char_count = max(char_count, len(value))
    for key in ('organizations', 'organizations_context', 'auto_organizations'):
        value = outline_body.get(key)
        if isinstance(value, list):
            org_count = max(org_count, len(value))
    structure_text = outline_body.get('structure')
    if isinstance(structure_text, str) and structure_text.strip():
        try:
            structure = json.loads(structure_text)
        except json.JSONDecodeError:
            structure = {}
        if isinstance(structure, dict):
            structure_characters = structure.get('characters')
            if isinstance(structure_characters, list):
                parsed_char_count = 0
                parsed_org_count = 0
                for item in structure_characters:
                    if not isinstance(item, dict):
                        continue
                    item_type = str(item.get('type') or '').strip().lower()
                    if item_type == 'organization':
                        parsed_org_count += 1
                    elif item_type:
                        parsed_char_count += 1
                char_count = max(char_count, parsed_char_count)
                org_count = max(org_count, parsed_org_count)
    return char_count, org_count


summary = {
    'base_url': BASE_URL,
    'started_at': time.strftime('%Y-%m-%dT%H:%M:%S'),
}
output_path = None

try:
    summary['readyz'] = request('GET', '/readyz', timeout=30)
    summary['login'] = request('POST', '/api/auth/local/login', payload={'username': USERNAME, 'password': PASSWORD}, timeout=30)
    summary['login']['username'] = USERNAME
    summary['login']['password_hint'] = '*' * len(PASSWORD)

    summary['settings'] = request('GET', '/api/settings', timeout=30)
    settings_body = summary['settings'].get('body') or {}
    settings_payload = {
        'api_key': settings_body.get('api_key', ''),
        'api_base_url': settings_body.get('api_base_url', ''),
        'provider': settings_body.get('api_provider') or settings_body.get('provider_type') or 'openai',
        'llm_model': settings_body.get('llm_model') or 'gpt-5.4',
        'temperature': settings_body.get('temperature'),
        'max_tokens': 1024,
    }
    summary['settings_snapshot'] = settings_payload
    summary['settings_test'] = request('POST', '/api/settings/test', payload=settings_payload, timeout=180)
    summary['function_calling_probe'] = request('POST', '/api/settings/check-function-calling', payload=settings_payload, timeout=180)

    project_payload = {
        'title': f'Codex Live Recheck {int(time.time())}',
        'description': 'Codex live regression for chapter budget and function calling.',
        'theme': '\u90fd\u5e02\u5f02\u8c61\u76f4\u64ad\u4e0e\u73b0\u5b9e\u6821\u9a8c',
        'genre': 'urban_fantasy',
        'target_words': 120000,
        'default_creative_mode': 'hook',
        'default_story_focus': 'advance_plot',
        'default_plot_stage': 'development',
        'default_story_creation_brief': '\u4ee5\u90fd\u5e02\u76f4\u64ad\u5f02\u8c61\u4e3a\u4e3b\u7ebf\uff0c\u56f4\u7ed5\u73b0\u5b9e\u6821\u9a8c\u3001\u8eab\u4efd\u7591\u70b9\u4e0e\u5012\u8ba1\u65f6\u538b\u529b\u6301\u7eed\u5347\u7ea7\u51b2\u7a81\uff1b\u4f18\u5148\u5199\u53ef\u89c1\u76ee\u6807\u3001\u53d7\u963b\u3001\u4ee3\u4ef7\u548c\u53cd\u8f6c\uff0c\u4e0d\u8981\u6cdb\u6cdb\u89e3\u91ca\u3002',
        'default_quality_preset': 'plot_drive',
        'default_quality_notes': '\u5f3a\u94a9\u5b50\u5f00\u573a-\u89c4\u5219\u843d\u5730-\u51b2\u7a81\u5347\u7ea7-\u7ae0\u5c3e\u53cd\u8f6c\u94a9\u5b50',
        'outline_mode': 'one-to-many',
    }
    summary['project_create'] = request('POST', '/api/projects', payload=project_payload, timeout=30)
    project_body = summary['project_create'].get('body') or {}
    project_id = project_body.get('id')
    summary['project_id'] = project_id
    if not project_id:
        raise RuntimeError(f'project create failed: {summary["project_create"]}')

    outline_payload = {
        'project_id': project_id,
        'theme': '\u90fd\u5e02\u5f02\u8c61\u76f4\u64ad\u4e0e\u73b0\u5b9e\u6821\u9a8c',
        'genre': 'urban_fantasy',
        'chapter_count': 1,
        'narrative_perspective': 'third_person',
        'creative_mode': 'hook',
        'story_focus': 'advance_plot',
        'plot_stage': 'development',
        'story_creation_brief': '\u4ee5\u90fd\u5e02\u76f4\u64ad\u5f02\u8c61\u4e3a\u4e3b\u7ebf\uff0c\u56f4\u7ed5\u73b0\u5b9e\u6821\u9a8c\u3001\u8eab\u4efd\u7591\u70b9\u4e0e\u5012\u8ba1\u65f6\u538b\u529b\u6301\u7eed\u5347\u7ea7\u51b2\u7a81\uff1b\u6bcf\u7ae0\u90fd\u8981\u7ed9\u51fa\u660e\u786e\u76ee\u6807\u3001\u53d7\u963b\u3001\u4ee3\u4ef7\u548c\u65b0\u94a9\u5b50\u3002',
        'quality_preset': 'plot_drive',
        'quality_notes': '\u5f00\u7bc7 30 \u79d2\u5185\u629b\u51fa\u5f02\u5e38\u4e0e\u4efb\u52a1\uff0c\u89c4\u5219\u8981\u76f4\u63a5\u6539\u53d8\u884c\u52a8\u7ed3\u679c\uff0c\u7ae0\u5c3e\u5fc5\u987b\u7559\u4e0b\u65b0\u5931\u8861\u6216\u8eab\u4efd\u7591\u70b9\u3002',
        'enable_mcp': True,
    }
    summary['outline_generate_stream'] = request_sse('/api/outlines/generate-stream', outline_payload, timeout=240)
    outline_stream = summary['outline_generate_stream']
    outline_error_event = outline_stream.get('error_event') or {}
    outline_result = normalize_outline_items((((outline_stream.get('result') or {}).get('data') or {}).get('outlines') or []))
    if not outline_result:
        outline_result = normalize_outline_items(outline_stream.get('chunk_json'))
    summary['outline_stream_outline_count'] = len(outline_result)
    summary['outline_stream_chunk_json_available'] = outline_stream.get('chunk_json') is not None
    outline_item = outline_result[0] if outline_result else {}
    outline_id = outline_item.get('id')
    summary['outline_id'] = outline_id
    if not outline_id:
        outline_list_response, outline_list_items = fetch_outline_list(project_id)
        summary['outline_list'] = outline_list_response
        summary['outline_list_count'] = len(outline_list_items)
        if outline_list_items:
            outline_item = outline_list_items[0]
            outline_id = outline_item.get('id')
            summary['outline_id'] = outline_id
    else:
        summary['outline_list'] = request('GET', f'/api/outlines/project/{project_id}', timeout=30)
        summary['outline_list_count'] = len(normalize_outline_items((summary.get('outline_list') or {}).get('body')))
    if not outline_id:
        last_progress = outline_stream.get('last_progress') or {}
        error_message = outline_error_event.get('message') or outline_error_event.get('error') or outline_error_event.get('detail') or 'outline generate did not return outline_id'
        error_message = (
            f"{error_message}; incomplete_read={outline_stream.get('incomplete_read')}; "
            f"event_count={outline_stream.get('event_count')}; "
            f"chunk_text_length={outline_stream.get('chunk_text_length')}; "
            f"last_progress={last_progress.get('message') or ''}"
        )
        raise RuntimeError(f'outline generate failed: {error_message}')

    visibility_started = time.perf_counter()
    visibility = {'samples': []}
    character_first_visible_ms = None
    organization_first_visible_ms = None
    final_character_count = 0
    final_organization_count = 0
    for _ in range(35):
        detail = request('GET', f'/api/outlines/{outline_id}', timeout=30)
        body = detail.get('body') or {}
        char_count, org_count = extract_counts(body)
        elapsed_ms = round((time.perf_counter() - visibility_started) * 1000, 2)
        visibility['samples'].append({
            'elapsed_ms': elapsed_ms,
            'character_count': char_count,
            'organization_count': org_count,
        })
        if char_count > 0 and character_first_visible_ms is None:
            character_first_visible_ms = elapsed_ms
        if org_count > 0 and organization_first_visible_ms is None:
            organization_first_visible_ms = elapsed_ms
        final_character_count = char_count
        final_organization_count = org_count
        if char_count > 0 and org_count > 0:
            break
        time.sleep(3)
    visibility['character_first_visible_ms'] = character_first_visible_ms
    visibility['organization_first_visible_ms'] = organization_first_visible_ms
    visibility['final_character_count'] = final_character_count
    visibility['final_organization_count'] = final_organization_count
    summary['outline_postprocess_visibility'] = visibility

    summary['chapter_create'] = request('POST', f'/api/outlines/{outline_id}/create-single-chapter', payload=None, timeout=30)
    chapter_body = summary['chapter_create'].get('body') or {}
    chapter_id = chapter_body.get('id') if isinstance(chapter_body, dict) else None
    if not chapter_id:
        manual_chapter_payload = {
            'project_id': project_id,
            'title': outline_item.get('title') or '?1?',
            'chapter_number': 1,
            'summary': outline_item.get('content') or '',
            'outline_id': outline_id,
            'sub_index': 1,
        }
        summary['chapter_create_fallback'] = request('POST', '/api/chapters', payload=manual_chapter_payload, timeout=30)
        fallback_body = summary['chapter_create_fallback'].get('body') or {}
        chapter_id = fallback_body.get('id') if isinstance(fallback_body, dict) else None
    summary['chapter_id'] = chapter_id
    if not chapter_id:
        raise RuntimeError(f'chapter create did not return chapter_id: {summary.get("chapter_create")}, fallback={summary.get("chapter_create_fallback")}')

    chapter_payload = {
        'target_word_count': 1200,
        'enable_analysis': True,
        'enable_mcp': True,
        'creative_mode': 'hook',
        'story_focus': 'advance_plot',
        'plot_stage': 'development',
        'quality_preset': 'plot_drive',
        'quality_notes': '\u4fdd\u6301\u5f3a\u94a9\u5b50\u5f00\u573a\u3001\u89c4\u5219\u843d\u5730\u3001\u51b2\u7a81\u5347\u7ea7\uff0c\u7ed3\u5c3e\u7559\u4e0b\u660e\u786e\u65b0\u5931\u8861\u6216\u65b0\u8eab\u4efd\u7ebf\u7d22\u3002',
    }
    summary['chapter_generate_stream'] = request_sse(f'/api/chapters/{chapter_id}/generate-stream', chapter_payload, timeout=420)
    summary['chapter_detail_after_generate'] = request('GET', f'/api/chapters/{chapter_id}', timeout=30)
    chapter_stream_result = (summary.get('chapter_generate_stream') or {}).get('result') or {}
    chapter_result_data = chapter_stream_result.get('data') if isinstance(chapter_stream_result, dict) else {}
    chapter_result_candidate_draft = chapter_result_data.get('candidate_draft') if isinstance(chapter_result_data, dict) else None
    quality_gate_action = chapter_result_data.get('quality_gate_action') if isinstance(chapter_result_data, dict) else None
    if quality_gate_action in {'retry', 'manual_review'}:
        summary['chapter_candidate_draft'] = request('GET', f'/api/chapters/{chapter_id}/analysis/candidate-draft', timeout=30)
        chapter_candidate_draft_body = (((summary.get('chapter_candidate_draft') or {}).get('body') or {}).get('candidate_draft'))
        if isinstance(chapter_result_candidate_draft, dict) and isinstance(chapter_candidate_draft_body, dict):
            summary['chapter_candidate_draft_consistency'] = {
                'attempt_id_match': chapter_result_candidate_draft.get('attempt_id') == chapter_candidate_draft_body.get('attempt_id'),
                'word_count_match': chapter_result_candidate_draft.get('word_count') == chapter_candidate_draft_body.get('word_count'),
                'can_apply_match': chapter_result_candidate_draft.get('can_apply') == chapter_candidate_draft_body.get('can_apply'),
                'sse_word_count': chapter_result_candidate_draft.get('word_count'),
                'detail_word_count': chapter_candidate_draft_body.get('word_count'),
            }
except Exception as exc:
    summary['error'] = {'type': type(exc).__name__, 'message': str(exc)}
finally:
    summary['finished_at'] = time.strftime('%Y-%m-%dT%H:%M:%S')
    TMP_LIVE_DIR.mkdir(parents=True, exist_ok=True)
    output_path = TMP_LIVE_DIR / f'tmp_live_test_summary_recheck_{int(time.time())}.json'
    output_path.write_text(json.dumps(summary, ensure_ascii=False, indent=2), encoding='utf-8', newline='\n')
    (TMP_LIVE_DIR / 'tmp_live_test_summary_latest.json').write_text(json.dumps(summary, ensure_ascii=False, indent=2), encoding='utf-8', newline='\n')

print(str(output_path))
print(json.dumps({
    'error': summary.get('error'),
    'settings_test_ms': ((summary.get('settings_test') or {}).get('body') or {}).get('response_time_ms') if isinstance((summary.get('settings_test') or {}).get('body'), dict) else None,
    'function_calling_supported': ((summary.get('function_calling_probe') or {}).get('body') or {}).get('supported') if isinstance((summary.get('function_calling_probe') or {}).get('body'), dict) else None,
    'outline_elapsed_ms': (summary.get('outline_generate_stream') or {}).get('elapsed_ms'),
    'outline_character_first_visible_ms': (summary.get('outline_postprocess_visibility') or {}).get('character_first_visible_ms'),
    'outline_organization_first_visible_ms': (summary.get('outline_postprocess_visibility') or {}).get('organization_first_visible_ms'),
    'chapter_result_word_count': ((((summary.get('chapter_generate_stream') or {}).get('result') or {}).get('data') or {}).get('word_count')),
    'chapter_quality_gate_decision': (((((summary.get('chapter_generate_stream') or {}).get('result') or {}).get('data') or {}).get('quality_metrics') or {}).get('quality_gate', {}) or {}).get('decision'),
    'chapter_candidate_pool_summary': (((((summary.get('chapter_generate_stream') or {}).get('result') or {}).get('data') or {}).get('quality_metrics') or {}).get('candidate_pool_summary')),
    'chapter_repair_seed_candidate_index': (((((summary.get('chapter_generate_stream') or {}).get('result') or {}).get('data') or {}).get('quality_metrics') or {}).get('candidate_selection', {}) or {}).get('repair_seed_candidate_index'),
    'chapter_repair_seed_generation_path': (((((summary.get('chapter_generate_stream') or {}).get('result') or {}).get('data') or {}).get('quality_metrics') or {}).get('candidate_selection', {}) or {}).get('repair_seed_generation_path'),
    'chapter_repair_seed_attempt_kind': (((((summary.get('chapter_generate_stream') or {}).get('result') or {}).get('data') or {}).get('quality_metrics') or {}).get('candidate_selection', {}) or {}).get('repair_seed_attempt_kind'),
    'chapter_saved_word_count': ((summary.get('chapter_detail_after_generate') or {}).get('body') or {}).get('word_count') if isinstance((summary.get('chapter_detail_after_generate') or {}).get('body'), dict) else None,
    'chapter_candidate_sse_word_count': (((((summary.get('chapter_generate_stream') or {}).get('result') or {}).get('data') or {}).get('candidate_draft') or {}).get('word_count')),
    'chapter_candidate_sse_can_apply': (((((summary.get('chapter_generate_stream') or {}).get('result') or {}).get('data') or {}).get('candidate_draft') or {}).get('can_apply')),
    'chapter_candidate_word_count': ((((summary.get('chapter_candidate_draft') or {}).get('body') or {}).get('candidate_draft') or {}).get('word_count')) if isinstance((summary.get('chapter_candidate_draft') or {}).get('body'), dict) else None,
    'chapter_candidate_has_full_content': ((((summary.get('chapter_candidate_draft') or {}).get('body') or {}).get('candidate_draft') or {}).get('has_full_content')) if isinstance((summary.get('chapter_candidate_draft') or {}).get('body'), dict) else None,
    'chapter_candidate_consistency': summary.get('chapter_candidate_draft_consistency'),
}, ensure_ascii=False, indent=2))
