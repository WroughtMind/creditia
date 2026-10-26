//! Comate/Zulu native histories, verified read-only from official VS Code
//! BaiduComate.comate 4.14.1, dist/comate-engine/server.js. ChatSessionManager,
//! SessionPaths and V2SessionRepository persist chat_session_<id> JSON and SHA-256
//! blobs. WriteFile/EditFile/ApplyPatch await editor saves before `executed`.
//! Rejected approvals also return normally: state=rejected/accepted=false is not
//! success. Selected payload.model can change on retries, so remains unknown.

use crate::{added_lines, any_ts, Activity, Health, Stats};
use creditia_core::model::{fingerprint, FileGroup, PathMap, Session, Src, Store};
use creditia_core::util::date_cn;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

fn root() -> PathBuf {
    std::env::var("CREDITIA_COMATE_DIR").ok().filter(|s| !s.is_empty())
        .map(PathBuf::from).unwrap_or_else(|| crate::home().join(".comate-engine/store"))
}
fn valid_id(id: &str) -> bool {
    !id.is_empty() && id != "." && id != ".."
        && id.bytes().all(|c| c.is_ascii_alphanumeric() || b"._-".contains(&c))
        && !id.ends_with(".deleted")
}
fn files() -> Vec<PathBuf> {
    let mut out: Vec<_> = std::fs::read_dir(root()).into_iter().flatten().flatten().filter_map(|e| {
        let p = e.path(); let name = e.file_name().to_string_lossy().to_string();
        let id = name.strip_prefix("chat_session_")?;
        (p.is_file() && valid_id(id) && !PathBuf::from(format!("{}.deleted",p.display())).exists()).then_some(p)
    }).collect(); out.sort(); out
}
fn read(path: &Path) -> Result<Value, String> {
    serde_json::from_slice(&std::fs::read(path).map_err(|e| e.to_string())?).map_err(|e| e.to_string())
}
fn blobs(file: &Path) -> PathBuf {
    file.parent().unwrap_or(Path::new("")).join("blobs")
        .join(format!("{}.d", file.file_name().unwrap_or_default().to_string_lossy()))
}
fn decode(v: &Value, dir: &Path) -> Result<Value, String> {
    if let Some(blob) = v.get("$blob") {
        let id = blob.as_str().and_then(|s| s.strip_prefix("sha256:"))
            .filter(|s| s.len() == 64 && s.bytes().all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c)))
            .ok_or("invalid Comate BlobRef digest")?;
        if v["version"] != 1 || v["compression"] != "none" { return Err("unsupported Comate BlobRef format".into()); }
        let path = dir.join(id); let meta = std::fs::symlink_metadata(&path).map_err(|e| e.to_string())?;
        if !meta.is_file() || meta.file_type().is_symlink() { return Err("Comate blob is not a regular file".into()); }
        let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
        if v["byteLength"].as_u64() != Some(bytes.len() as u64) || format!("{:x}", Sha256::digest(&bytes)) != id {
            return Err("Comate blob checksum/length mismatch".into());
        }
        return match v["encoding"].as_str() {
            Some("utf8") => String::from_utf8(bytes).map(Value::String).map_err(|e| e.to_string()),
            Some("json") => serde_json::from_slice(&bytes).map_err(|e| e.to_string()),
            _ => Err("Comate edit receipt uses a non-text blob encoding".into()),
        };
    }
    if let Some(a) = v.as_array() { return a.iter().map(|v| decode(v, dir)).collect::<Result<Vec<_>,_>>().map(Value::Array); }
    if let Some(o) = v.as_object() {
        return o.iter().map(|(k,v)| Ok((k.clone(),decode(v,dir)?))).collect::<Result<serde_json::Map<_,_>,String>>().map(Value::Object);
    }
    Ok(v.clone())
}
#[derive(Default)]
struct Parsed { id: String, cwd: String, start: i64, end: i64, prompts: i64, edits: Vec<(String, String, i64)> }
fn abs(cwd: &str, path: &str) -> String {
    let path = crate::file_uri_path(path);
    if Path::new(&path).is_absolute() || path.get(1..2) == Some(":") { path }
    else if cwd.is_empty() { String::new() }
    else { format!("{}/{path}",cwd.trim_end_matches('/')) }
}
fn receipt(meta: &Value, cwd: &str, moved: bool) -> Option<(String,String)> {
    let after = meta["content"].as_str()?;
    let before = if meta.get("originalContent")?.is_null() { "" } else { meta["originalContent"].as_str()? };
    let path = if moved { meta["movePath"].as_str().filter(|s| !s.is_empty()).or(meta["absolutePath"].as_str())? }
        else { meta["absolutePath"].as_str()? };
    let path = abs(cwd,path); let text = added_lines(before,after);
    (!path.is_empty() && !text.is_empty()).then_some((path,text))
}
fn tools(node: &Value, cwd: &str, ts: i64, dir: &Path, seen: &mut HashSet<String>, out: &mut Vec<(String,String,i64)>) -> Result<(),String> {
    if node["discard"] == true { return Ok(()); }
    if node["type"] == "TOOL" && node["toolState"] == "executed" && node["accepted"] != false && node["error"].is_null() {
        let name = node["toolName"].as_str().unwrap_or("");
        if matches!(name,"write_file"|"edit_file"|"Write"|"Edit"|"apply_patch") {
            let id = node["id"].as_str().filter(|s| !s.is_empty()).ok_or("Comate receipt has no tool ID")?;
            let meta = decode(&node["result"]["metadata"],dir)?;
            if meta["state"] != "rejected" && meta["success"] != false && seen.insert(id.into()) {
                if name == "apply_patch" {
                    for c in meta["changes"].as_array().into_iter().flatten() {
                        if let Some((path,text)) = receipt(c,cwd,true) { out.push((path,text,ts)); }
                    }
                } else if let Some((path,text)) = receipt(&meta,cwd,false) { out.push((path,text,ts)); }
            }
        }
    }
    for c in node["children"].as_array().into_iter().flatten() { tools(c,cwd,ts,dir,seen,out)?; }
    Ok(())
}
fn parse(v: &Value, dir: &Path) -> Result<Parsed,String> {
    let id = v["sessionUuid"].as_str().filter(|s| valid_id(s)).ok_or("invalid Comate session ID")?;
    // Imported Codex/Claude/Dush and remote histories retain their own source.
    if !native(v) {
        return Err("Comate imported/remote history is not native Zulu evidence".into());
    }
    let messages = v["messages"].as_array().ok_or("invalid Comate messages")?;
    let mut p = Parsed { id:id.into(),cwd:v["workspaceDirectory"].as_str().unwrap_or("").into(),
        start:any_ts(&v["ctime"]).unwrap_or(0),end:any_ts(&v["utime"]).unwrap_or(0),..Default::default() };
    let mut seen = HashSet::new(); let mut requested = 0;
    // markForkPointMessage tags the final inherited assistant message. Earlier
    // copied messages may have remapped IDs and are still not new executions.
    let inherited=messages.iter().rposition(|m|m["forkedFromSessionUuid"].is_string());
    for (index,m) in messages.iter().enumerate() {
        if inherited.is_some_and(|cut|index<=cut){continue;}
        if m["discard"] == true { continue; }
        if m["role"] == "user" {
            p.prompts += 1; requested = any_ts(&m["requestedAt"]).unwrap_or(p.start);
            if requested > 0 { p.start = if p.start == 0 { requested } else { p.start.min(requested) }; }
        } else if m["role"] == "assistant" {
            let ts = any_ts(&m["completedAt"]).unwrap_or(requested.max(p.start)); p.end = p.end.max(ts);
            for turn in m["elements"].as_array().into_iter().flatten() { tools(turn,&p.cwd,ts,dir,&mut seen,&mut p.edits)?; }
        }
    }
    Ok(p)
}
fn native(v:&Value)->bool{
    matches!(v["source"].as_str(),None|Some("AgentConversation"|"SpecConversation"))&&!v["externalSource"].is_string()
}
fn load(file: &Path) -> Result<Option<Parsed>,String> {
    let v = read(file)?; let id = file.file_name().unwrap_or_default().to_string_lossy();
    if v["sessionUuid"].as_str() != id.strip_prefix("chat_session_") { return Err("Comate session filename/ID mismatch".into()); }
    if !native(&v){return Ok(None);}
    let dir = blobs(file); let manifest = dir.join("manifest.json");
    if manifest.exists() {
        let m = read(&manifest)?;
        if m["formatVersion"] != 2 || m["state"] != "complete" || m["sessionUuid"] != v["sessionUuid"] {
            return Err("Comate V2 manifest is incomplete or mismatched".into());
        }
    } else if contains_blob(&v) {
        return Err("Comate BlobRefs require a complete V2 manifest".into());
    }
    parse(&v,&dir).map(Some)
}
fn contains_blob(v:&Value)->bool{
    match v {
        Value::Object(o)=>o.contains_key("$blob")||o.values().any(contains_blob),
        Value::Array(a)=>a.iter().any(contains_blob),
        _=>false,
    }
}
fn fp(file: &Path) -> (u64,u64) {
    let mut result = fingerprint(file);
    for e in walkdir::WalkDir::new(blobs(file)).follow_links(false).into_iter().filter_map(Result::ok).filter(|e|e.file_type().is_file()) {
        let f = fingerprint(e.path()); result.0 = result.0.max(f.0); result.1 += f.1;
    }
    result
}
pub fn scan(store: &mut Store, pm: &PathMap, st: &mut Stats) {
    for file in files() {
        let key = format!("comate-native:{}",file.display()); let fp = fp(&file);
        if store.groups.get(&key).is_some_and(|g|g.fp==fp) { continue; }
        let p = match load(&file) { Ok(Some(p))=>p,Ok(None)=>continue,Err(e)=>{st.warnings.push(format!("comate: {}: {e}",file.display()));continue;} };
        let mut g = FileGroup{fp,..Default::default()};
        for (path,text,ts) in &p.edits { crate::push_text(&mut g.evs,pm,path,text,Src::Comate,"",&p.id,*ts); }
        if g.evs.is_empty() && !pm.in_project(&p.cwd) { continue; }
        g.sessions.push(Session{src:Src::Comate,id:p.id,model:String::new(),start:p.start,end:p.end,tokens:0,prompts:p.prompts,branch:None,reviewer:false,has_content:true,provider:None});
        store.groups.insert(key,g);
    }
}
pub fn discover() -> Vec<Activity> {
    let mut out:HashMap<String,Activity> = HashMap::new();
    for p in files().iter().filter_map(|f|load(f).ok().flatten()).filter(|p|!p.cwd.is_empty()) {
        let a = out.entry(p.cwd.clone()).or_insert(Activity{src:Src::Comate,cwd:p.cwd,remote:None,sessions:0,last:0}); a.sessions+=1;a.last=a.last.max(p.end);
    } out.into_values().collect()
}
pub fn doctor() -> Health {
    let all=files();let parsed:Vec<_>=all.iter().filter_map(|f|load(f).ok().flatten()).collect();
    Health{source:"comate".into(),path:root().display().to_string(),readable:!parsed.is_empty(),format:"chat_session_<id> JSON; V2 blobs/manifest.json + SHA-256 BlobRef".into(),records:parsed.len()as u64,earliest:parsed.iter().map(|p|p.start).filter(|&t|t>0).min().map(date_cn),latest:parsed.iter().map(|p|p.end).filter(|&t|t>0).max().map(date_cn),note:creditia_core::tr!("官方 VS Code 4.14.1/Zulu engine 已只读核对。只读取 executed 且未拒绝的原生保存回执，校验外置正文哈希；导入其他 agent 的历史不归为 Comate。未持久化逐调用执行模型，保留未知；时间采用响应完成/请求时间。已通过构造记录验证，尚无真实会话验收。", "Official VS Code 4.14.1/Zulu engine inspected read-only. Native executed, non-rejected save receipts only; external text blobs are checksum-verified. Imported agent histories keep their own source. Per-call execution models are not persisted and remain unknown; response/request times are used. Fixture validated, not yet validated with real sessions."),..Default::default()}
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn native_receipts_exclude_approvals_failures_imports_and_current_models() {
        let tool=json!({"id":"t","type":"TOOL","toolName":"Edit","toolState":"executed","accepted":true,"result":{"metadata":{"absolutePath":"/repo/a.rs","originalContent":"keep","content":"keep\nwritten line"}}});
        let mut denied=tool.clone();denied["id"]=json!("denied");denied["result"]["metadata"]["state"]=json!("rejected");
        let mut failed=tool.clone();failed["id"]=json!("failed");failed["toolState"]=json!("failed");
        let mut rejected=tool.clone();rejected["id"]=json!("undo");rejected["accepted"]=json!(false);
        let mut v=json!({"sessionUuid":"s","source":"AgentConversation","workspaceDirectory":"/repo","ctime":1790000000000i64,"utime":1790000005000i64,"modelConfig":{"modelId":"current"},"messages":[{"role":"user","requestedAt":1790000001000i64,"payload":{"model":{"modelId":"requested-only"}}},{"role":"assistant","completedAt":1790000004000i64,"elements":[{"children":[tool.clone(),tool,denied,failed,rejected]}]}]});
        let p=parse(&v,Path::new("/unused")).unwrap();assert_eq!(p.edits,vec![("/repo/a.rs".into(),"written line\n".into(),1790000004)]);
        v["messages"][1]["forkedFromSessionUuid"]=json!("original-session");
        assert!(parse(&v,Path::new("/unused")).unwrap().edits.is_empty());
        v["source"]=json!("CodexConversation");assert!(parse(&v,Path::new("/unused")).is_err());
    }
    #[test]
    fn v2_blob_checksums_are_required_and_paths_cannot_escape() {
        let dir=std::env::temp_dir().join(format!("creditia-comate-blobs-{}",std::process::id()));std::fs::create_dir_all(&dir).unwrap();
        let bytes=b"actual saved text";let id=format!("{:x}",Sha256::digest(bytes));std::fs::write(dir.join(&id),bytes).unwrap();
        let mut v=json!({"$blob":format!("sha256:{id}"),"version":1,"encoding":"utf8","byteLength":bytes.len(),"compression":"none"});
        assert_eq!(decode(&v,&dir).unwrap(),json!("actual saved text"));v["byteLength"]=json!(1);assert!(decode(&v,&dir).is_err());v["$blob"]=json!("sha256:../secret");assert!(decode(&v,&dir).is_err());std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn v2_native_loader_requires_complete_manifest_and_reports_corrupt_receipts(){
        let root=std::env::temp_dir().join(format!("creditia-comate-load-{}",std::process::id()));std::fs::create_dir_all(&root).unwrap();let file=root.join("chat_session_s");let dir=blobs(&file);std::fs::create_dir_all(&dir).unwrap();
        let content=b"saved generated line";let id=format!("{:x}",Sha256::digest(content));std::fs::write(dir.join(&id),content).unwrap();
        let reference=json!({"$blob":format!("sha256:{id}"),"version":1,"encoding":"utf8","byteLength":content.len(),"compression":"none"});
        let v=json!({"sessionUuid":"s","source":"AgentConversation","workspaceDirectory":"/repo","messages":[{"role":"assistant","elements":[{"children":[{"type":"TOOL","id":"write","toolName":"Write","toolState":"executed","result":{"metadata":{"absolutePath":"/repo/a.rs","originalContent":null,"content":reference}}}]}]}]});
        std::fs::write(&file,v.to_string()).unwrap();assert!(load(&file).is_err());
        let manifest=dir.join("manifest.json");std::fs::write(&manifest,json!({"formatVersion":2,"state":"complete","sessionUuid":"s"}).to_string()).unwrap();
        assert_eq!(load(&file).unwrap().unwrap().edits[0].1,"saved generated line\n");
        std::fs::write(dir.join(id),b"tampered").unwrap();assert!(load(&file).is_err());std::fs::remove_dir_all(root).unwrap();
    }
}
