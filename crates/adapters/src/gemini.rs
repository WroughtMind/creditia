//! Gemini CLI 的项目登记表与会话文件；工具执行成功才构成行级证据。
use crate::{any_ts,source_dir,Activity,Health,Stats};
use creditia_core::model::{fingerprint,FileGroup,PathMap,Session,Src,Store};
use serde_json::{json,Value};
use std::{collections::{HashMap,HashSet},path::PathBuf};

fn projects()->Vec<(PathBuf,String)> {
    let base=source_dir(Src::Gemini);let mut roots=HashMap::new();
    if let Ok(t)=std::fs::read_to_string(base.join("projects.json")) {
        if let Ok(v)=serde_json::from_str::<Value>(&t){if let Some(map)=v["projects"].as_object(){for (cwd,slug) in map{if let Some(slug)=slug.as_str(){roots.insert(base.join("tmp").join(slug),cwd.clone());}}}}
    }
    for e in std::fs::read_dir(base.join("tmp")).into_iter().flatten().flatten(){
        if let Ok(cwd)=std::fs::read_to_string(e.path().join(".project_root")){roots.insert(e.path(),cwd.trim().to_string());}
    }
    roots.into_iter().collect()
}
fn files(dir:&std::path::Path)->Vec<PathBuf>{
    let mut files:Vec<_>=std::fs::read_dir(dir.join("chats")).into_iter().flatten().flatten().map(|e|e.path()).filter(|p|p.extension().map(|e|e=="json"||e=="jsonl").unwrap_or(false)).collect();files.sort();files
}
fn merge(dst:&mut Value,src:&Value){
    if let (Some(d),Some(s))=(dst.as_object_mut(),src.as_object()){for(k,v)in s{if v.is_object(){merge(d.entry(k.clone()).or_insert(json!({})),v)}else{d.insert(k.clone(),v.clone());}}}else{*dst=src.clone();}
}
fn read(p:&std::path::Path)->Result<Value,String>{
    let text=std::fs::read_to_string(p).map_err(|e|e.to_string())?;
    if p.extension().map(|e|e=="json").unwrap_or(false){return serde_json::from_str(&text).map_err(|e|e.to_string())}
    let mut meta=json!({});let mut messages:Vec<Value>=vec![];
    for line in text.lines().filter(|l|!l.trim().is_empty()){
        let v:Value=serde_json::from_str(line).map_err(|e|e.to_string())?;
        if v["$set"].is_object(){merge(&mut meta,&v["$set"]);if let Some(m)=v["$set"]["messages"].as_array(){messages=m.clone();}}
        else if let Some(id)=v["$rewindTo"].as_str(){if let Some(i)=messages.iter().position(|m|m["id"]==id){messages.truncate(i+1);}}
        else if v["$patch"].is_object(){
            let patch=&v["$patch"];let mut updates=patch["updates"].as_array().cloned().unwrap_or_default();if patch["id"].is_string(){updates.push(patch.clone());}
            for u in updates{if let Some(m)=messages.iter_mut().find(|m|m["id"]==u["id"]){
                if !u["content"].is_null(){m["content"]=u["content"].clone();}
                for update in u["toolCalls"].as_array().into_iter().flatten(){if let Some(calls)=m["toolCalls"].as_array_mut(){if let Some(call)=calls.iter_mut().find(|c|c["id"]==update["id"]){merge(call,update);}}}
            }}
            if let Some(ids)=patch["removeIds"].as_array(){messages.retain(|m|!ids.contains(&m["id"]));}
            if let Some(ids)=patch["orderIds"].as_array(){messages.sort_by_key(|m|ids.iter().position(|id|id==&m["id"]).unwrap_or(ids.len()));}
        }else if v["type"].is_string(){messages.push(v)}else{merge(&mut meta,&v)}
    }
    meta["messages"]=json!(messages);Ok(meta)
}
pub fn scan(store:&mut Store,pm:&PathMap,st:&mut Stats){
    let mut count=0;
    for (dir,cwd) in projects().into_iter().filter(|(_,cwd)|pm.in_project(cwd)){
        for path in files(&dir){
            let key=format!("gemini:{}",path.display());let fp=fingerprint(&path);
            if store.groups.get(&key).map(|g|g.fp==fp).unwrap_or(false){count+=1;continue}
            let v=match read(&path){Ok(v)=>v,Err(e)=>{st.warnings.push(format!("Gemini 会话文件无法读取 {}：{e}",path.display()));continue}};
            let id=v["sessionId"].as_str().unwrap_or("");let start=any_ts(&v["startTime"]).unwrap_or(0);let mut end=any_ts(&v["lastUpdated"]).unwrap_or(start);
            let mut g=FileGroup{fp,..Default::default()};let mut models=HashSet::new();let mut tokens=0;let mut prompts=0;
            for m in v["messages"].as_array().into_iter().flatten(){
                let ts=any_ts(&m["timestamp"]).unwrap_or(0);end=end.max(ts);
                if m["type"]=="user"{prompts+=1;}if m["type"]!="gemini"{continue}
                let model=m["model"].as_str().unwrap_or("");if !model.is_empty(){models.insert(model.to_string());}
                tokens+=m["tokens"]["total"].as_i64().unwrap_or(0);
                for call in m["toolCalls"].as_array().into_iter().flatten(){
                    if call["status"]!="success"{continue}
                    let input=&call["args"];let path=input["file_path"].as_str().or(input["absolute_path"].as_str()).unwrap_or("");
                    let path=if std::path::Path::new(path).is_absolute()||path.as_bytes().get(1)==Some(&b':'){path.into()}else{format!("{cwd}/{path}")};
                    let content=match call["name"].as_str(){Some("write_file")=>input["content"].as_str(),Some("replace")=>input["new_string"].as_str(),_=>None};
                    if let Some(text)=content{crate::push_text(&mut g.evs,pm,&path,text,Src::Gemini,model,id,any_ts(&call["timestamp"]).unwrap_or(ts));}
                }
            }
            let model=if models.len()==1{models.into_iter().next().unwrap()}else{String::new()};
            g.sessions.push(Session{src:Src::Gemini,id:id.into(),model,provider:None,start,end,tokens,branch:None,prompts,reviewer:false,has_content:true});
            store.groups.insert(key,g);count+=1;
        }
    }
    st.notes.push(format!("gemini: {count} 个项目会话；通过 projects.json 或 .project_root 关联目录"));
}
pub fn discover()->Vec<Activity>{projects().into_iter().filter_map(|(dir,cwd)|{let fs=files(&dir);if fs.is_empty(){return None}let last=fs.iter().map(|p|fingerprint(p).0 as i64).max().unwrap_or(0);Some(Activity{src:Src::Gemini,cwd,remote:None,sessions:fs.len()as u64,last})}).collect()}
pub fn doctor()->Health{let base=source_dir(Src::Gemini);let ps=projects();Health{source:"gemini".into(),path:base.display().to_string(),readable:base.join("tmp").is_dir(),format:"projects.json / .project_root + chats/*.json[l]".into(),records:ps.iter().map(|(d,_)|files(d).len()as u64).sum(),note:"支持登记项目中的成功 write_file/replace；无法对应项目的旧哈希目录不猜测".into(),..Default::default()}}
