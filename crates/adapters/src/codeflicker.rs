//! Official CodeFlicker 1.0.29 (20260507, extension 9.6.2601080), downloaded
//! from www.codeflicker.ai/api/proxy/update/api/update/darwin-arm64/stable/external/1.
//! Read-only ZIP/pkg VFS extraction; no installed/downloaded agent execution.
//! Extension SqliteCore and pkg src/agent2/session/ApiConversationHistory.ts:
//! KwaipilotKV apiConversationHistory:<sid>, composerData:<sid> JSON values.
//! Tool responses persist toolId/toolName/params/category/isError. Write/replace
//! success sentences are emitted only after the IDE returns type=success;
//! cross-repo rejection may have isError=false but never that receipt.
//! Current chatModel is rewritten by ensureMessagesChatMode; it is NOT a
//! historical execution model. Native histories therefore keep models unknown.

use crate::{added_lines, any_ts, Activity, Health, Stats};
use creditia_core::model::{fingerprint, FileGroup, PathMap, Session, Src, Store};
use creditia_core::util::date_cn;
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

fn root() -> PathBuf {
    std::env::var("CREDITIA_CODEFLICKER_DIR").ok().filter(|s|!s.is_empty()).map(PathBuf::from)
        .unwrap_or_else(||crate::home().join(".codeflicker"))
}
fn dirs() -> Vec<PathBuf> {
    let r=root();let mut out=vec![r.join("data/codeflicker"),r.join("data/vscode")];
    if r.join("api_history.sqlite").is_file()||r.join("session_data.sqlite").is_file(){out.push(r);}
    out.sort();out.dedup();out
}
fn kv(path:&Path,prefix:&str)->Result<HashMap<String,Value>,String>{
    if !path.is_file(){return Ok(HashMap::new());}
    let c=crate::open_ro(path).map_err(|e|e.to_string())?;
    let mut q=c.prepare("SELECT key,value FROM KwaipilotKV WHERE key LIKE ?1").map_err(|e|e.to_string())?;
    let rows=q.query_map([format!("{prefix}%")],|r|{
        let key:String=r.get(0)?;
        let data=match r.get_ref(1)?{rusqlite::types::ValueRef::Text(t)|rusqlite::types::ValueRef::Blob(t)=>t.to_vec(),_=>vec![]};Ok((key,data))
    }).map_err(|e|e.to_string())?;
    let mut out=HashMap::new();
    for row in rows{let(k,b)=row.map_err(|e|e.to_string())?;let v=serde_json::from_slice(&b).map_err(|e|e.to_string())?;out.insert(k.trim_start_matches(prefix).into(),v);}
    Ok(out)
}
fn workspace(data:&Value)->String{
    if data["worktreeConfig"]["status"]!="deleted"{
        if let Some(path)=data["worktreeConfig"]["worktreePath"].as_str().filter(|s|!s.is_empty()){return crate::file_uri_path(path);}
    }
    // storageUri points at the extension's directory under workspaceStorage,
    // not at the project. Only the editor's explicit workspace.json maps it.
    let storage=crate::file_uri_path(data["workspaceUri"].as_str().unwrap_or(""));
    let Some(parent)=Path::new(&storage).parent()else{return String::new()};
    let value:Value=std::fs::read(parent.join("workspace.json")).ok().and_then(|b|serde_json::from_slice(&b).ok()).unwrap_or_default();
    value["folder"].as_str().map(crate::file_uri_path).unwrap_or_default()
}
#[derive(Default)]
struct Parsed{id:String,cwd:String,start:i64,end:i64,prompts:i64,tokens:i64,edits:Vec<(String,String,i64,String)>}
fn success_text(name:&str,path:&str)->String{
    match name{"write_to_file"=>format!("The content has been successfully saved to {path}."),_=>format!("The content has been successfully replaced in {path}.")}
}
fn parse(id:&str,history:&Value,data:&Value)->Parsed{
    let mut p=Parsed{id:id.into(),cwd:workspace(data),..Default::default()};
    let mut times=HashMap::new();let mut locals=HashMap::new();
    for m in data["localMessages"].as_array().into_iter().flatten(){
        let ts=any_ts(&m["ts"]).unwrap_or(0);
        if ts>0{p.start=if p.start==0{ts}else{p.start.min(ts)};p.end=p.end.max(ts);}
        if m["role"]=="user"{p.prompts+=1;}
        if let Some(id)=m["toolCallId"].as_str().filter(|s|!s.is_empty()){times.insert(id,ts);locals.insert(id,m);}
    }
    let mut seen=HashSet::new();
    for m in history.as_array().into_iter().flatten(){
        p.tokens+=m["usage"]["totalTokens"].as_i64().unwrap_or(0);
        if m["role"]!="user"{continue;}
        for b in m["content"].as_array().into_iter().flatten(){
            if b["category"]!="tool-response"||b["isError"]!=false||b["type"]!="text"{continue;}
            let Some(id)=b["toolId"].as_str().filter(|s|!s.is_empty())else{continue};
            let name=b["toolName"].as_str().unwrap_or("");let args=&b["params"];
            if !matches!(name,"write_to_file"|"replace_in_file"|"multi_replace_in_file"){continue;}
            let Some(path)=args["path"].as_str().filter(|s|!s.is_empty())else{continue};
            if b["text"].as_str().unwrap_or("").trim()!=success_text(name,path)||!seen.insert(id){continue;}
            let effect=locals.get(id).map(|m|&m["workingSetEffect"]).unwrap_or(&Value::Null);
            // Copies reset workingSetEffect to init; explicit rejection means
            // this previously successful edit has since been reverted.
            if matches!(effect["status"].as_str(),Some("init"|"rejected")){continue;}
            let path=if Path::new(path).is_absolute()||path.get(1..2)==Some(":"){path.into()}else if !p.cwd.is_empty(){format!("{}/{path}",p.cwd.trim_end_matches('/'))}else{continue};
            let saved=effect["diffContent"]["originalCode"].as_str().zip(effect["diffContent"]["modifiedCode"].as_str());
            let text=if let Some((before,after))=saved{added_lines(before,after)}else{match name{
                // Success proves the overwrite ran, not that retained file
                // context was generated. Whole writes require the saved diff.
                "write_to_file"=>String::new(),
                "replace_in_file"=>added_lines(args["oldString"].as_str().unwrap_or(""),args["newString"].as_str().unwrap_or("")),
                // The IDE may return success for a partially applied multi-
                // replacement. Only its persisted actual before/after receipt
                // can distinguish applied operations from failed candidates.
                _=>String::new(),
            }};
            if !text.is_empty(){p.edits.push((path,text,times.get(id).copied().unwrap_or(p.start),id.into()));}
        }
    }p
}
fn sessions(dir:&Path)->Result<Vec<Parsed>,String>{
    let legacy=dir.join("session_data.sqlite");
    let mut histories=kv(&legacy,"apiConversationHistory:")?;
    histories.extend(kv(&dir.join("api_history.sqlite"),"apiConversationHistory:")?);
    let mut local=kv(&legacy,"composerData:")?;
    local.extend(kv(&dir.join("composer_data.sqlite"),"composerData:")?);
    let mut out:Vec<_>=histories.into_iter().filter(|(_,v)|v.is_array()).map(|(id,v)|parse(&id,&v,local.get(&id).unwrap_or(&Value::Null))).collect();
    out.sort_by(|a,b|(a.start,&a.id).cmp(&(b.start,&b.id)));Ok(out)
}
fn fp(dir:&Path)->(u64,u64){
    let mut fp=(0,0);
    for file in ["session_data.sqlite","composer_data.sqlite","api_history.sqlite"]{
        for suffix in ["","-wal"]{let a=fingerprint(&dir.join(format!("{file}{suffix}")));fp.0=fp.0.max(a.0);fp.1+=a.1;}
    }fp
}
pub fn scan(store:&mut Store,pm:&PathMap,st:&mut Stats){
    // Native session cloning copies old tool IDs. A copied receipt is not a new
    // execution, even when the cloned history has a different session UUID.
    let mut seen=HashSet::new();
    for dir in dirs(){
        let fp=fp(&dir);
        let parsed=match sessions(&dir){Ok(p)=>p,Err(e)=>{st.warnings.push(format!("codeflicker: {}: {e}",dir.display()));continue;}};
        for mut p in parsed{
            p.edits.retain(|(_,_,_,id)|seen.insert(id.clone()));
            let key=format!("codeflicker-native:{}:{}",dir.display(),p.id);
            if store.groups.get(&key).is_some_and(|g|g.fp==fp){continue;}
            let mut g=FileGroup{fp,..Default::default()};
            for(path,text,ts,_)in &p.edits{crate::push_text(&mut g.evs,pm,path,text,Src::CodeFlicker,"",&p.id,*ts);}
            if g.evs.is_empty()&&!pm.in_project(&p.cwd){continue;}
            g.sessions.push(Session{src:Src::CodeFlicker,id:p.id,model:String::new(),start:p.start,end:p.end,tokens:p.tokens,prompts:p.prompts,branch:None,reviewer:false,has_content:true,provider:None});
            store.groups.insert(key,g);
        }
    }
}
fn repo(path:&str)->Option<String>{Path::new(path).ancestors().find(|p|p.join(".git").exists()).map(|p|p.display().to_string())}
pub fn discover()->Vec<Activity>{
    let mut out:HashMap<String,Activity>=HashMap::new();
    for p in dirs().iter().flat_map(|dir|sessions(dir).unwrap_or_default()){
        let mut roots:HashSet<String>=p.edits.iter().filter_map(|e|repo(&e.0)).collect();
        if !p.cwd.is_empty(){roots.insert(p.cwd);}
        for cwd in roots{let a=out.entry(cwd.clone()).or_insert(Activity{src:Src::CodeFlicker,cwd,remote:None,sessions:0,last:0});a.sessions+=1;a.last=a.last.max(p.end);}
    }out.into_values().collect()
}
pub fn doctor()->Health{
    let parsed:Vec<_>=dirs().iter().flat_map(|d|sessions(d).unwrap_or_default()).collect();
    Health{source:"codeflicker".into(),path:dirs().iter().map(|p|p.display().to_string()).collect::<Vec<_>>().join("; "),readable:!parsed.is_empty(),format:"{session_data,composer_data,api_history}.sqlite / KwaipilotKV native tool-response receipts".into(),records:parsed.len()as u64,earliest:parsed.iter().map(|p|p.start).filter(|&t|t>0).min().map(date_cn),latest:parsed.iter().map(|p|p.end).filter(|&t|t>0).max().map(date_cn),note:creditia_core::tr!("已只读核对官方 1.0.29 ZIP 和内嵌 agent JS。只接受 isError=false 且匹配写入后固定成功回执；拒绝、失败、仅工具请求不计。项目根取编辑器 workspace.json 或真实 worktree；模型字段会被重写，保留未知。时间取同 toolCallId 的原生消息，没有则取会话开始。已通过构造记录验证，尚无真实会话验收。", "Official 1.0.29 ZIP and embedded agent JS inspected read-only. Requires isError=false plus the exact post-save success receipt; rejected, failed and request-only calls do not count. Projects use editor workspace.json or recorded worktree paths. Mutable model fields remain unknown. Native toolCallId message times are used, otherwise session start. Fixture validated, not yet validated with real sessions."),..Default::default()}
}

#[cfg(test)]
mod tests{
    use super::*;use serde_json::json;
    #[test]
    fn exact_native_success_excludes_rejection_that_has_no_error_flag(){
        let ok=json!({"type":"text","category":"tool-response","toolName":"replace_in_file","toolId":"t","isError":false,"params":{"path":"/repo/a.rs","oldString":"keep","newString":"keep\nactual added line"},"text":"The content has been successfully replaced in /repo/a.rs.\n"});
        let mut denied=ok.clone();denied["toolId"]=json!("denied");denied["text"]=json!("User rejected editing another repository.");
        let mut failed=ok.clone();failed["toolId"]=json!("failed");failed["isError"]=json!(true);
        let data=json!({"chatModel":"current-must-not-backfill","localMessages":[{"ts":1790000000000i64,"role":"user"},{"toolCallId":"t","ts":1790000002000i64,"chatModel":"also-mutable"}]});
        let p=parse("s",&json!([{"role":"user","content":[ok.clone(),ok,denied,failed]}]),&data);
        assert_eq!(p.edits,vec![("/repo/a.rs".into(),"actual added line\n".into(),1790000002,"t".into())]);
    }
    #[test]
    fn sqlite_native_blob_codec_and_relative_paths_need_workspace_binding(){
        let dir=std::env::temp_dir().join(format!("creditia-codeflicker-{}",std::process::id()));std::fs::create_dir_all(&dir).unwrap();let file=dir.join("api_history.sqlite");
        let c=rusqlite::Connection::open(&file).unwrap();c.execute_batch("CREATE TABLE KwaipilotKV(key TEXT PRIMARY KEY,value BLOB);").unwrap();
        let v=json!([{"role":"user","content":[{"category":"tool-response","type":"text","isError":false,"toolId":"a","toolName":"write_to_file","params":{"path":"a.rs","content":"written line"},"text":"The content has been successfully saved to a.rs."}]}]);
        c.execute("INSERT INTO KwaipilotKV VALUES(?1,?2)",rusqlite::params!["apiConversationHistory:s",v.to_string().as_bytes()]).unwrap();drop(c);
        let loaded=kv(&file,"apiConversationHistory:").unwrap();assert_eq!(loaded["s"],v);
        assert!(parse("s",&v,&Value::Null).edits.is_empty());
        let p=parse("s",&v,&json!({"worktreeConfig":{"worktreePath":"/repo","status":"active"},"localMessages":[{"toolCallId":"a","workingSetEffect":{"status":"accepted","diffContent":{"originalCode":"human context","modifiedCode":"human context\nwritten line"}}}]}));assert_eq!(p.edits[0].0,"/repo/a.rs");assert_eq!(p.edits[0].1,"written line\n");
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn partially_successful_multi_replace_uses_saved_text_not_all_candidates(){
        let history=json!([{"role":"user","content":[{"category":"tool-response","type":"text","isError":false,"toolId":"multi","toolName":"multi_replace_in_file","params":{"path":"/repo/a.rs","operations":[{"oldString":"old","newString":"actual saved line"},{"oldString":"missing","newString":"failed candidate line"}]},"text":"The content has been successfully replaced in /repo/a.rs."}]}]);
        assert!(parse("s",&history,&Value::Null).edits.is_empty());
        let mut local=json!({"localMessages":[{"toolCallId":"multi","ts":1790000000000i64,"workingSetEffect":{"status":"accepted","diffContent":{"originalCode":"old","modifiedCode":"actual saved line"}}}]});
        let p=parse("s",&history,&local);assert_eq!(p.edits[0].1,"actual saved line\n");
        local["localMessages"][0]["workingSetEffect"]["status"]=json!("rejected");assert!(parse("s",&history,&local).edits.is_empty());
        local["localMessages"][0]["workingSetEffect"]["status"]=json!("init");assert!(parse("s",&history,&local).edits.is_empty());
    }
}
