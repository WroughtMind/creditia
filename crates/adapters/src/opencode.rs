//! OpenCode 的本地会话数据库。只查询 session/message/part，不读取账号与密钥表。
use crate::{any_ts, open_ro, source_dir, Activity, Health, Stats};
use creditia_core::model::{FileGroup, PathMap, Session, Src, Store};
use creditia_core::util::date_cn;
use serde_json::Value;
use std::{collections::HashMap, path::PathBuf};

fn database() -> PathBuf {
    let base = source_dir(Src::Opencode);
    std::env::var("OPENCODE_DB").ok().filter(|s| !s.is_empty()).map(|p| base.join(p)).unwrap_or_else(|| base.join("opencode.db"))
}
fn sessions() -> Result<Vec<(String,String,i64,i64)>,String> {
    let db=database(); if !db.exists(){return Ok(vec![])}
    let conn=open_ro(&db).map_err(|e| e.to_string())?;
    let mut q=conn.prepare("select id,directory,time_created,time_updated from session order by time_created,id").map_err(|e|format!("OpenCode 会话结构无法识别：{e}"))?;
    let rows=q.query_map([],|r|Ok((r.get(0)?,r.get(1)?,r.get::<_,i64>(2)?/1000,r.get::<_,i64>(3)?/1000))).map_err(|e|e.to_string())?.collect::<rusqlite::Result<Vec<_>>>().map_err(|e|e.to_string())?;
    Ok(rows)
}
pub fn scan(store:&mut Store, pm:&PathMap, st:&mut Stats){
    let rows=match sessions(){Ok(x)=>x,Err(e)=>{st.warnings.push(e);return}};
    if rows.is_empty(){return}
    let conn=match open_ro(&database()){Ok(c)=>c,Err(e)=>{st.warnings.push(format!("OpenCode：{e}"));return}};
    let mut count=0;
    for (id,cwd,start,end) in rows.into_iter().filter(|(_,cwd,_,_)|pm.in_project(cwd)) {
        count+=1;
        // 每条写入的模型和服务商取其所属 assistant 消息，不按会话多数票覆盖。
        let result=(|| -> Result<FileGroup,String>{
            let mut g=FileGroup::default();
            let mut q=conn.prepare("select id,data from message where session_id=?1 order by time_created,id").map_err(|e|e.to_string())?;
            let msgs=q.query_map([&id],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?))).map_err(|e|e.to_string())?.collect::<rusqlite::Result<Vec<_>>>().map_err(|e|e.to_string())?;
            let mut models=HashMap::new();let mut tokens=0;let mut prompts=0;
            for (mid,text) in msgs {
                let m:Value=serde_json::from_str(&text).map_err(|e|e.to_string())?;
                if m["role"]=="user" {prompts+=1;}
                if m["role"]!="assistant" {continue}
                let model=m["modelID"].as_str().unwrap_or("").to_string();
                let provider=m["providerID"].as_str().filter(|p|!p.is_empty()).map(String::from);
                let t=&m["tokens"];
                tokens+=["input","output","reasoning"].iter().map(|k|t[*k].as_i64().unwrap_or(0)).sum::<i64>()+t["cache"]["read"].as_i64().unwrap_or(0)+t["cache"]["write"].as_i64().unwrap_or(0);
                models.insert(mid,(model,provider));
            }
            let mut q=conn.prepare("select message_id,data from part where session_id=?1 order by time_created,id").map_err(|e|e.to_string())?;
            let parts=q.query_map([&id],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?))).map_err(|e|e.to_string())?.collect::<rusqlite::Result<Vec<_>>>().map_err(|e|e.to_string())?;
            for (mid,text) in parts {
                let p:Value=serde_json::from_str(&text).map_err(|e|e.to_string())?;
                if p["type"]!="tool" || p["state"]["status"]!="completed" {continue}
                let Some((model,provider))=models.get(&mid) else {continue};
                let state=&p["state"];let input=&state["input"];
                let ts=any_ts(&state["time"]["end"]).unwrap_or(0);
                let path=input["filePath"].as_str().unwrap_or("");
                let path=if std::path::Path::new(path).is_absolute(){path.to_string()}else{format!("{cwd}/{path}")};
                let before=g.evs.len();
                match p["tool"].as_str(){
                    Some("write")=>{crate::push_text(&mut g.evs,pm,&path,input["content"].as_str().unwrap_or(""),Src::Opencode,model,&id,ts);},
                    Some("edit")=>{crate::push_text(&mut g.evs,pm,&path,input["newString"].as_str().unwrap_or(""),Src::Opencode,model,&id,ts);},
                    Some("apply_patch")=>crate::push_patch(&mut g.evs,pm,&cwd,input["patchText"].as_str().unwrap_or(""),Src::Opencode,model,&id,ts),
                    _=>{}
                }
                for e in &mut g.evs[before..]{e.provider=provider.clone();}
            }
            // 会话可以包含多种模型；没有可唯一代表它的模型就留空。
            let identities:std::collections::HashSet<_>=models.into_values().collect();
            let (model,provider)=if identities.len()==1{identities.into_iter().next().unwrap()}else{(String::new(),None)};
            g.sessions.push(Session{src:Src::Opencode,id:id.clone(),model,provider,start,end,tokens,branch:None,prompts,reviewer:false,has_content:true});
            Ok(g)
        })();
        match result{Ok(g)=>{store.groups.insert(format!("opencode:{id}"),g);},Err(e)=>st.warnings.push(format!("OpenCode 会话 {id} 无法读取：{e}"))}
    }
    st.notes.push(format!("opencode: {count} 个项目会话；仅采集成功写入，保留模型和服务商原始标识"));
}
pub fn discover()->Vec<Activity>{sessions().unwrap_or_default().into_iter().map(|(_,cwd,_,last)|Activity{src:Src::Opencode,cwd,remote:None,sessions:1,last}).collect()}
pub fn doctor()->Health{
    let db=database();let mut h=Health{source:"opencode".into(),path:db.display().to_string(),format:"session + message + part".into(),..Default::default()};
    match sessions(){Ok(v)=>{h.readable=db.exists();h.records=v.len() as u64;h.earliest=v.iter().map(|x|x.2).min().map(date_cn);h.latest=v.iter().map(|x|x.3).max().map(date_cn);h.note="记录存在时支持模型、服务商和已完成的 edit/write/apply_patch".into();},Err(e)=>h.note=e}h
}
