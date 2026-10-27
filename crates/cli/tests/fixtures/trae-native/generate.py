"""Generate synthetic native-contract rows with the official SQLCipher shell."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess

parser = argparse.ArgumentParser()
parser.add_argument('--sqlcipher', required=True, type=Path)
parser.add_argument('--native-lib', required=True, type=Path)
parser.add_argument('--out', required=True, type=Path)
args = parser.parse_args()
native = args.native_lib.read_bytes()
expected = '750360db5dac81e74cd8b4d38ff35e3c8b6a336aea77701ba3aa49857b4c5692'
if hashlib.sha256(native).hexdigest() != expected:
    parser.error('expected the pinned official TRAE 3.6.2 Linux arm64 library')
args.out.mkdir(parents=True, exist_ok=True)
if any((args.out / name).exists() for name in ['success.db', 'failed.db']):
    parser.error('output fixtures already exist; use an empty directory')
password = bytes(c ^ b'rust'[i % 4] ^ b'cpp'[i % 3] ^ b'electron'[i % 8]
                 for i, c in enumerate(native[0x50ab22c:0x50ab24c]))
key = hashlib.pbkdf2_hmac('sha256', password, native[0x54a86b8:0x54a86c8], 100000, 32)
schema = '''
CREATE TABLE local_project(project_id TEXT,absolute_path TEXT,deleted_at INTEGER);
INSERT INTO local_project VALUES('p','/private/tmp/creditia-trae-native-project',0);
CREATE TABLE chat_session(session_id TEXT,project_id TEXT,created_at INTEGER,updated_at INTEGER,deleted_at INTEGER);
INSERT INTO chat_session VALUES('native-s','p',1720000000,1720000010,0);
CREATE TABLE chat_message(session_id TEXT,message_id TEXT,message_type TEXT,message_role TEXT,message_index INTEGER,reply_to_message_id TEXT,is_archived INTEGER,user_message_context TEXT,created_at INTEGER,deleted_at INTEGER);
INSERT INTO chat_message VALUES('native-s','u','general','user',0,'',0,'{}',1720000000,0);
INSERT INTO chat_message VALUES('native-s','a','task','assistant',1,'u',0,'{}',1720000001,0);
CREATE TABLE chat_message_task(message_id TEXT,content TEXT,deleted_at INTEGER);
CREATE TABLE chat_turn(response_message_id TEXT,context TEXT,deleted_at INTEGER);
INSERT INTO chat_turn VALUES('a','{"model_smart_selection_meta":{"config_name":"fixture-model"}}',0);
'''
for name, status in [('success.db', 'success'), ('failed.db', 'failed')]:
    content = {'task_id': 'task-1', 'user_message_id': 'u', 'messages': [{
        'id': 'item-1', 'message_type': 'plan_item', 'plan_item': {
            'id': 'plan-1', 'timing': {'tool_call_finished_at_ms': 1720000005000},
            'tool_call_info': {'id': 'write-1', 'name': 'Write',
                'params': {'file_path': '/private/tmp/creditia-trae-native-project/a.rs',
                           'content': 'requested fake line'},
                'result': {'status': status, 'data': {'changes': [{
                    'file_path': '/private/tmp/creditia-trae-native-project/a.rs',
                    'file_action': 'modified',
                    'old_content': 'unchanged line\nremoved line',
                    'new_content': 'unchanged line\nactual native fixture line'
                }]}}}
        }
    }]}
    value = json.dumps(content, separators=(',', ':')).replace("'", "''")
    sql = 'PRAGMA key = "x\'' + key.hex() + '\'";\n' + schema
    sql += "INSERT INTO chat_message_task VALUES('a','" + value + "',0);\nPRAGMA integrity_check;\n"
    target = args.out / name
    result = subprocess.run([str(args.sqlcipher), str(target)], input=sql.encode(),
                            stdout=subprocess.PIPE, stderr=subprocess.PIPE, check=True)
    if not result.stdout.splitlines() or any(line != b'ok' for line in result.stdout.splitlines()):
        raise SystemExit('reference SQLCipher integrity check did not return ok')
    print(json.dumps({'file': name, 'sha256': hashlib.sha256(target.read_bytes()).hexdigest()}))
