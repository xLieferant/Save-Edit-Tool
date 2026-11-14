exec((__import__('pathlib').Path(__file__).with_name('simplify_logging.py')).read_text(encoding='utf-8').split('old = ')[0])
p = backend / 'service.rs'
s = p.read_text(encoding='utf-8')
# Remove the obsolete report-building path, including private profile/save name collection.
start = s.index('pub fn build_support_report(')
end = s.index('pub fn export_user_logs(', start)
s = s[:start] + '''pub fn build_support_report(_profile_state: &AppProfileState) -> Result<String, String> {
    logs::diagnostic::active().ok_or("diagnostics.errors.unavailable")?.read_text()
}

''' + s[end:]
start = s.index('    let report_id = format!(', s.index('fn record_entry('))
end = s.index('fn open_runtime_connection', start)
s = s[:start] + '''    let detail = technical_details.map(|cause| format!("{}\\nReason: {}", user_message, cause))
        .unwrap_or_else(|| user_message.to_owned());
    log_to_user_log(level, action, &detail)
}

''' + s[end:]
start = s.index('fn resolve_profile_name_from_path(')
end = s.index('pub fn redact_path(', start)
s = s[:start] + s[end:]
s = s.replace('use std::fs;\n', '').replace('use chrono::{Local, Utc};\n', '').replace('use rusqlite::{Connection, params};', 'use rusqlite::Connection;').replace('use tauri_plugin_dialog::DialogExt;\n', '').replace('use uuid::Uuid;\n', '')
for text in ['use crate::shared::decrypt::decrypt_if_needed;\n', 'use crate::shared::extract::extract_profile_name;\n', 'use crate::shared::extract_save_name::extract_save_name;\n', 'use crate::shared::paths::{game_sii_from_save, info_sii_from_save};\n', 'const REPORT_EXPORT_NAME: &str = "ets2_tool_logs.txt";\n']:
    s = s.replace(text, '')
write(p, s)

# Keep every original translation byte-for-byte; discard only our superseded, unused additions.
import subprocess, json
errors = {
 'unavailable': 'The logging service is unavailable.', 'unsafe_path': 'The selected path is unsafe or contains a link.',
 'storage_limit': 'The logging storage limit was exceeded.', 'writer_busy': 'The log writer is busy. Try again shortly.',
 'invalid_cleanup': 'Only temporary cache cleanup is supported.', 'confirmation_required': 'Explicit confirmation is required.',
 'cleanup_changed': 'The cache changed. Try clearing it again.'
}
for language in ['en', 'de']:
    p = root / f'src-tauri/locales/{language}.json'
    simple = json.loads(p.read_text(encoding='utf-8'))['simple_logging']
    data = subprocess.check_output(['git', 'show', f'HEAD:ets2-tool/src-tauri/locales/{language}.json'], cwd=root.parent)
    header = b'"diagnostics": {'
    index = data.index(header) + len(header)
    values = errors if language == 'en' else {k: '[NEEDS TRANSLATION] ' + v for k, v in errors.items()}
    block = json.dumps({'errors': values}, indent=2, ensure_ascii=False)[1:-1].strip('\n')
    data = data[:index] + b'\n' + block.encode('utf-8') + b',' + data[index:]
    index = data.rfind(b'}')
    block = json.dumps({'simple_logging': simple}, ensure_ascii=False, indent=2)[1:-1].strip('\n')
    data = data[:index].rstrip() + b',\n' + block.encode('utf-8') + b'\n' + data[index:]
    p.write_bytes(data)

p = root.parent / '.gitignore'
s = p.read_text(encoding='utf-8')
if '**/logs/archive/application-*.txt' not in s:
    s += '\n**/logs/archive/application-*.txt\n**/logs/archive/emergency-*.txt\n'
write(p, s)
p = root / 'src-tauri/build.rs'
s = p.read_text(encoding='utf-8').replace('if name.ends_with(".log") || name.contains(".log.") || name.ends_with(".jsonl") {', 'if name.ends_with(".log") || name.contains(".log.") || name.ends_with(".jsonl")\n                || ((name.starts_with("application-") || name.starts_with("emergency-")) && name.ends_with(".txt")) {')
write(p, s)
