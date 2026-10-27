//! Native Antigravity conversation SQLite, confirmed against the installed
//! application's embedded gemini_coder/exa protobuf descriptors (2026-10-17).
//! Generic tool results wrap an executed Step; arguments and rendered Markdown
//! are never evidence. Only completed ActionResultEdit unified-diff insertions.
use crate::{file_uri_path, open_ro, Activity, Health, Stats};
use creditia_core::model::{FileGroup, PathMap, Session, Src, Store};
use rusqlite::Connection;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

#[derive(Clone, Copy)]
enum Field<'a> {
    Int(u64),
    Bytes(&'a [u8]),
    Fixed,
}
struct Pb<'a>(Vec<(u32, Field<'a>)>);
fn varint(b: &[u8], at: &mut usize) -> Result<u64, String> {
    let mut value = 0;
    for shift in (0..70).step_by(7) {
        let byte = *b.get(*at).ok_or("truncated protobuf varint")?;
        *at += 1;
        if shift == 63 && byte > 1 {
            return Err("overflowed protobuf varint".into());
        }
        value |= u64::from(byte & 127) << shift;
        if byte < 128 {
            return Ok(value);
        }
    }
    Err("invalid protobuf varint".into())
}
impl<'a> Pb<'a> {
    fn parse(b: &'a [u8]) -> Result<Self, String> {
        let mut at = 0;
        let mut out = vec![];
        while at < b.len() {
            let tag = varint(b, &mut at)?;
            let number = u32::try_from(tag >> 3).map_err(|_| "invalid protobuf field")?;
            if number == 0 || number >= 1 << 29 {
                return Err("invalid protobuf field".into());
            }
            let field = match tag & 7 {
                0 => Field::Int(varint(b, &mut at)?),
                2 => {
                    let len = usize::try_from(varint(b, &mut at)?)
                        .map_err(|_| "invalid protobuf size")?;
                    let end = at.checked_add(len).ok_or("invalid protobuf size")?;
                    let data = b.get(at..end).ok_or("truncated protobuf message")?;
                    at = end;
                    Field::Bytes(data)
                }
                wire @ (1 | 5) => {
                    at = at
                        .checked_add(if wire == 1 { 8 } else { 4 })
                        .filter(|&end| end <= b.len())
                        .ok_or("truncated protobuf fixed field")?;
                    Field::Fixed
                }
                _ => return Err("unsupported protobuf wire type".into()),
            };
            out.push((number, field));
        }
        Ok(Self(out))
    }
    fn bytes(&self, n: u32) -> Option<&'a [u8]> {
        self.0.iter().rev().find_map(|(k, v)| match v {
            Field::Bytes(b) if *k == n => Some(*b),
            _ => None,
        })
    }
    fn int(&self, n: u32) -> u64 {
        self.0
            .iter()
            .rev()
            .find_map(|(k, v)| match v {
                Field::Int(v) if *k == n => Some(*v),
                _ => None,
            })
            .unwrap_or(0)
    }
    fn text(&self, n: u32) -> &str {
        self.bytes(n)
            .and_then(|b| std::str::from_utf8(b).ok())
            .unwrap_or("")
    }
    fn message(&self, n: u32) -> Result<Pb<'a>, String> {
        Self::parse(self.bytes(n).unwrap_or_default())
    }
    fn repeated(&self, n: u32) -> impl Iterator<Item = &'a [u8]> + '_ {
        self.0.iter().filter_map(move |(k, v)| match v {
            Field::Bytes(b) if *k == n => Some(*b),
            _ => None,
        })
    }
    fn indices(&self, n: u32) -> Result<Vec<i64>, String> {
        let mut out = vec![];
        for (k, v) in &self.0 {
            if *k != n {
                continue;
            }
            match v {
                Field::Int(v) => out.push(i64::try_from(*v).map_err(|_| "invalid step index")?),
                Field::Bytes(b) => {
                    let mut at = 0;
                    while at < b.len() {
                        out.push(
                            i64::try_from(varint(b, &mut at)?).map_err(|_| "invalid step index")?,
                        );
                    }
                }
                Field::Fixed => return Err("invalid step index encoding".into()),
            }
        }
        Ok(out)
    }
}
fn timestamp(pb: &Pb<'_>, field: u32) -> Result<i64, String> {
    Ok(i64::try_from(pb.message(field)?.int(1)).unwrap_or(0))
}
fn files() -> Vec<PathBuf> {
    let root = crate::editor_history::antigravity_dir();
    let root = if root.is_file() {
        return vec![root];
    } else if root.file_name().is_some_and(|s| s == "conversations") {
        root
    } else {
        root.join("conversations")
    };
    let mut out: Vec<_> = walkdir::WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_file() && e.path().extension().is_some_and(|s| s == "db"))
        .map(|e| e.into_path())
        .collect();
    out.sort();
    out
}
struct Info {
    id: String,
    roots: Vec<String>,
    start: i64,
}
fn info(c: &Connection) -> Result<Info, String> {
    let (trajectory, cascade): (String, Option<String>) = c
        .query_row(
            "SELECT trajectory_id,cascade_id FROM trajectory_meta LIMIT 1",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .map_err(|e| e.to_string())?;
    let bytes: Vec<u8> = c
        .query_row(
            "SELECT data FROM trajectory_metadata_blob WHERE id='main'",
            [],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;
    let meta = Pb::parse(&bytes)?;
    let roots = meta
        .repeated(7)
        .filter_map(|b| std::str::from_utf8(b).ok())
        .map(file_uri_path)
        .collect();
    Ok(Info {
        id: cascade.filter(|s| !s.is_empty()).unwrap_or(trajectory),
        roots,
        start: timestamp(&meta, 2)?,
    })
}
fn fingerprint(path: &Path) -> (u64, u64) {
    let mut fp = (0, 0);
    for p in [
        path.to_path_buf(),
        PathBuf::from(format!("{}-wal", path.display())),
    ] {
        if let Ok(m) = std::fs::metadata(p) {
            fp.0 = fp.0.max(
                m.modified()
                    .ok()
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|d| d.as_nanos() as u64)
                    .unwrap_or(0),
            );
            fp.1 += m.len();
        }
    }
    fp
}

fn edit(
    step: &Pb<'_>,
    g: &mut FileGroup,
    pm: &PathMap,
    model: &str,
    sid: &str,
    ts: i64,
) -> Result<(), String> {
    // An outer "done" generic call may still contain a failed/canceled action.
    if step.int(4) != 3 || step.bytes(31).is_some_and(|b| !b.is_empty()) {
        return Ok(());
    }
    if step.int(1) == 132 {
        let generic = step.message(140)?;
        let result = generic.message(2)?;
        let payload = result.message(6)?;
        if payload.text(1) == "type.googleapis.com/gemini_coder.Step" {
            let child = payload.message(2)?;
            // The observed protocol wraps one concrete action, never recurses
            // through another generic result. Reject that malformed shape.
            if child.int(1) != 132 {
                edit(&child, g, pm, model, sid, ts)?;
            }
        }
    } else if step.int(1) == 5 {
        let action = step.message(10)?;
        if action.int(5) == 2 {
            return Ok(());
        } // explicit rejected code action
        let result = action.message(2)?;
        let actual = result.message(1)?;
        let path = if actual.text(8).is_empty() {
            actual.text(1).to_string()
        } else {
            file_uri_path(actual.text(8))
        };
        let diff = actual.message(2)?;
        let unified = diff.message(3)?;
        let (mut removed, mut additions) = (String::new(), String::new());
        for b in unified.repeated(3) {
            let line = Pb::parse(b)?;
            if line.int(2) == 1 {
                additions.push_str(line.text(1));
                additions.push('\n');
            } else if line.int(2) == 2 {
                removed.push_str(line.text(1));
                removed.push('\n');
            }
        }
        if !path.is_empty() && !additions.is_empty() {
            crate::push_text(
                &mut g.evs,
                pm,
                &path,
                &crate::added_lines(&removed, &additions),
                Src::AntigravityIde,
                model,
                sid,
                ts,
            );
        }
    }
    Ok(())
}
fn parse(c: &Connection, i: &Info, pm: &PathMap) -> Result<FileGroup, String> {
    let mut g = FileGroup::default();
    let mut by_step: HashMap<i64, Option<String>> = HashMap::new();
    let (mut models, mut tokens) = (HashMap::<String, usize>::new(), 0_i64);
    let mut q = c
        .prepare("SELECT data FROM gen_metadata ORDER BY idx")
        .map_err(|e| e.to_string())?;
    for row in q
        .query_map([], |r| r.get::<_, Vec<u8>>(0))
        .map_err(|e| e.to_string())?
    {
        let data = row.map_err(|e| e.to_string())?;
        let gen = Pb::parse(&data)?;
        let chat = gen.message(1)?;
        let model = if chat.text(22).is_empty() {
            chat.text(19)
        } else {
            chat.text(22)
        };
        if !model.is_empty() {
            *models.entry(model.to_string()).or_default() += 1;
        }
        let usage = chat.message(4)?;
        tokens = tokens
            .saturating_add(i64::try_from(usage.int(2).saturating_add(usage.int(3))).unwrap_or(0));
        for idx in gen.indices(2)? {
            let entry = by_step
                .entry(idx)
                .or_insert_with(|| Some(model.to_string()));
            if entry.as_deref() != Some(model) {
                *entry = None;
            }
        }
    }
    let (mut prompts, mut end) = (0, i.start);
    let mut q = c.prepare("SELECT idx,step_type,status,metadata,step_payload,step_format,error_details FROM steps ORDER BY idx").map_err(|e| e.to_string())?;
    let rows = q
        .query_map([], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, i64>(1)?,
                r.get::<_, i64>(2)?,
                r.get::<_, Option<Vec<u8>>>(3)?.unwrap_or_default(),
                r.get::<_, Vec<u8>>(4)?,
                r.get::<_, i64>(5)?,
                r.get::<_, Option<Vec<u8>>>(6)?.unwrap_or_default(),
            ))
        })
        .map_err(|e| e.to_string())?;
    for row in rows {
        let (idx, kind, status, metadata, payload, format, error) =
            row.map_err(|e| e.to_string())?;
        if format != 0 {
            return Err(format!("unrecognized native step format {format}"));
        }
        let meta = Pb::parse(&metadata)?;
        let created = timestamp(&meta, 1)?;
        let completed = timestamp(&meta, 8)?;
        end = end.max(created).max(completed);
        if kind == 14 {
            prompts += 1;
        }
        if status != 3 || !error.is_empty() {
            continue;
        }
        let model_info = meta.message(24)?;
        let direct_model = if model_info.text(12).is_empty() {
            model_info.text(8)
        } else {
            model_info.text(12)
        };
        let model = if direct_model.is_empty() {
            by_step.get(&idx).and_then(Option::as_deref).unwrap_or("")
        } else {
            direct_model
        };
        let step = Pb::parse(&payload)?;
        if step.int(1) != kind as u64 {
            return Err("native step type disagrees with payload".into());
        }
        edit(
            &step,
            &mut g,
            pm,
            model,
            &i.id,
            if completed > 0 { completed } else { created },
        )?;
    }
    g.sessions.push(Session {
        src: Src::AntigravityIde,
        id: i.id.clone(),
        model: models
            .into_iter()
            .max_by_key(|(m, n)| (*n, std::cmp::Reverse(m.clone())))
            .map(|(m, _)| m)
            .unwrap_or_default(),
        start: i.start,
        end,
        tokens,
        branch: None,
        prompts,
        reviewer: false,
        has_content: true,
        provider: None,
    });
    Ok(g)
}
pub fn scan(store: &mut Store, pm: &PathMap, st: &mut Stats) {
    let paths = files();
    let keys: HashSet<_> = paths
        .iter()
        .map(|p| format!("antigravity-native:{}", p.display()))
        .collect();
    store
        .groups
        .retain(|k, _| !k.starts_with("antigravity-native:") || keys.contains(k));
    for p in paths {
        let key = format!("antigravity-native:{}", p.display());
        let fp = fingerprint(&p);
        if store.groups.get(&key).is_some_and(|g| g.fp == fp) {
            continue;
        }
        let result = (|| {
            let c = open_ro(&p).map_err(|e| e.to_string())?;
            let i = info(&c)?;
            if !i.roots.iter().any(|r| pm.in_project(r)) {
                return Ok(FileGroup::default());
            }
            parse(&c, &i, pm)
        })();
        match result {
            Ok(mut g) => {
                g.fp = fp;
                store.groups.insert(key, g);
            }
            Err(e) => {
                store.groups.remove(&key);
                st.warnings
                    .push(format!("antigravity-ide: {}: {e}", p.display()));
            }
        }
    }
}
pub fn discover() -> (Vec<Activity>, HashSet<String>) {
    let mut activities = vec![];
    let mut ids = HashSet::new();
    for p in files() {
        if let Ok(i) = open_ro(&p)
            .map_err(|e| e.to_string())
            .and_then(|c| info(&c))
        {
            ids.insert(i.id);
            for root in i.roots {
                activities.push(Activity {
                    src: Src::AntigravityIde,
                    cwd: root,
                    remote: None,
                    sessions: 1,
                    last: i.start,
                });
            }
        }
    }
    (activities, ids)
}
pub fn doctor() -> Option<Health> {
    let paths = files();
    if paths.is_empty() {
        return None;
    }
    let mut count = 0;
    let mut errors = vec![];
    for p in &paths {
        match open_ro(p).map_err(|e| e.to_string()).and_then(|c| info(&c)) {
            Ok(_) => count += 1,
            Err(e) => errors.push(e),
        }
    }
    Some(Health { source: Src::AntigravityIde.harness().into(), path: crate::editor_history::antigravity_dir().join("conversations").display().to_string(), readable: errors.is_empty(), format: "native trajectory SQLite + protobuf".into(), records: count, earliest: None, latest: None, note: format!("Reads native executed Step/ActionResultEdit insertions and generation response models; excludes failed, canceled, rejected, context and unknown step formats. Summary records only fill missing native sessions. {}", errors.join("; ")), unsupported: false })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn vi(mut n: u64) -> Vec<u8> {
        let mut v = vec![];
        while n >= 128 {
            v.push((n as u8 & 127) | 128);
            n >>= 7;
        }
        v.push(n as u8);
        v
    }
    fn int(n: u32, v: u64) -> Vec<u8> {
        [vi(u64::from(n) << 3), vi(v)].concat()
    }
    fn bytes(n: u32, v: &[u8]) -> Vec<u8> {
        [vi(u64::from(n) << 3 | 2), vi(v.len() as u64), v.to_vec()].concat()
    }
    fn line(t: u64, text: &str) -> Vec<u8> {
        bytes(3, &[bytes(1, text.as_bytes()), int(2, t)].concat())
    }
    fn action(status: u64, ack: u64) -> Vec<u8> {
        let lines = [
            line(3, "unchanged context line"),
            line(2, "removed previous line"),
            line(1, "new successful addition"),
        ]
        .concat();
        let actual = [bytes(8, b"file:///demo/a.rs"), bytes(2, &bytes(3, &lines))].concat();
        [
            int(1, 5),
            int(4, status),
            bytes(10, &[int(5, ack), bytes(2, &bytes(1, &actual))].concat()),
        ]
        .concat()
    }
    fn generic(child: &[u8]) -> Vec<u8> {
        let payload = [
            bytes(1, b"type.googleapis.com/gemini_coder.Step"),
            bytes(2, child),
        ]
        .concat();
        [
            int(1, 132),
            int(4, 3),
            bytes(140, &bytes(2, &bytes(6, &payload))),
        ]
        .concat()
    }
    #[test]
    fn native_result_requires_success_and_keeps_only_actual_insertions() {
        let pm = PathMap {
            known: ["a.rs".into()].into_iter().collect(),
            markers: vec!["/demo/".into()],
            excludes: vec![],
        };
        let mut g = FileGroup::default();
        for payload in [
            action(7, 0),
            action(6, 0),
            action(3, 2),
            generic(&action(7, 0)),
        ] {
            edit(&Pb::parse(&payload).unwrap(), &mut g, &pm, "m", "s", 100).unwrap();
        }
        assert!(g.evs.is_empty());
        edit(
            &Pb::parse(&generic(&action(3, 0))).unwrap(),
            &mut g,
            &pm,
            "m",
            "s",
            100,
        )
        .unwrap();
        assert_eq!(g.evs.len(), 1);
        assert_eq!(g.evs[0].model, "m");
        assert!(g.evs[0].single_use);
    }
    #[test]
    fn native_database_maps_each_generation_and_rejects_unknown_format() {
        let c = Connection::open_in_memory().unwrap();
        c.execute_batch("CREATE TABLE gen_metadata(idx INTEGER,data BLOB); CREATE TABLE steps(idx INTEGER,step_type INTEGER,status INTEGER,metadata BLOB,step_payload BLOB,step_format INTEGER,error_details BLOB);").unwrap();
        for (idx, model) in [(0_i64, "first"), (1, "second")] {
            let generation = [
                bytes(1, &bytes(19, model.as_bytes())),
                bytes(2, &vi(idx as u64)),
            ]
            .concat();
            c.execute(
                "INSERT INTO gen_metadata VALUES(?1,?2)",
                rusqlite::params![idx, generation],
            )
            .unwrap();
            let meta = bytes(1, &int(1, 100 + idx as u64));
            c.execute(
                "INSERT INTO steps VALUES(?1,132,3,?2,?3,0,NULL)",
                rusqlite::params![idx, meta, generic(&action(3, 0))],
            )
            .unwrap();
        }
        let pm = PathMap {
            known: ["a.rs".into()].into_iter().collect(),
            markers: vec!["/demo/".into()],
            excludes: vec![],
        };
        let info = Info {
            id: "s".into(),
            roots: vec!["/demo".into()],
            start: 99,
        };
        let g = parse(&c, &info, &pm).unwrap();
        assert_eq!(
            g.evs.iter().map(|e| e.model.as_str()).collect::<Vec<_>>(),
            vec!["first", "second"]
        );
        assert_eq!(g.evs[1].ts, 101);
        c.execute("UPDATE steps SET step_format=1 WHERE idx=1", [])
            .unwrap();
        assert!(parse(&c, &info, &pm).err().unwrap().contains("step format"));
    }
    #[test]
    fn malformed_protobuf_is_not_silently_accepted() {
        for b in [
            &[0_u8][..],
            &[10, 3, 1],
            &[8, 128],
            &[10, 255, 255, 255, 255, 255, 255, 255, 255, 255, 2],
        ] {
            assert!(Pb::parse(b).is_err());
        }
    }
}
