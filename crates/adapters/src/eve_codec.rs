//! Read the JSON parts of Workflow's devalue graph without reviving streams,
//! classes, functions or encryption keys. Pinned upstream contracts: SUPPORT_AGENT4.md.
use base64::{engine::general_purpose::STANDARD, Engine};
use serde_json::{Map, Value};
use std::{collections::HashSet, io::Read};

// A compact reference graph can expand exponentially. This is a decoding safety
// limit, not a cap on the number of histories or contributions scanned.
const MAX_BYTES: usize = 64 * 1024 * 1024;

pub(super) struct Graph(Vec<Value>);
impl Graph {
    pub(super) fn read(v: &Value) -> Result<Self, String> {
        let graph = if v["__type"] == "Uint8Array" {
            let mut bytes = STANDARD
                .decode(v["data"].as_str().ok_or("missing Workflow bytes")?)
                .map_err(|e| e.to_string())?;
            let prefix = bytes.get(..4).ok_or("missing Workflow format prefix")?;
            if matches!(prefix, b"gzip" | b"zstd") {
                let reader: Box<dyn Read> = if prefix == b"gzip" {
                    Box::new(flate2::read::GzDecoder::new(&bytes[4..]))
                } else {
                    Box::new(
                        zstd::stream::read::Decoder::new(&bytes[4..]).map_err(|e| e.to_string())?,
                    )
                };
                let mut decoded = vec![];
                reader
                    .take(MAX_BYTES as u64 + 1)
                    .read_to_end(&mut decoded)
                    .map_err(|e| e.to_string())?;
                bytes = decoded;
            }
            if bytes.len() > MAX_BYTES {
                return Err("Workflow payload exceeds decoding budget".into());
            }
            match bytes.get(..4) {
                Some(b"devl") => serde_json::from_slice(&bytes[4..]).map_err(|e| e.to_string())?,
                Some(b"encr" | b"encp") => {
                    return Err("encrypted Workflow payload; prior evidence retained".into())
                }
                _ => return Err("unsupported Workflow format".into()),
            }
        } else {
            // The SDK also accepts legacy, already flattened devalue arrays.
            v.clone()
        };
        let values = graph
            .as_array()
            .filter(|a| !a.is_empty())
            .ok_or("expected devalue graph")?;
        Ok(Self(values.clone()))
    }

    /// Select only a needed branch before decoding it. Unrelated opaque runtime
    /// capabilities (for example sessionWritable) must never be deserialized.
    pub(super) fn get(&self, path: &[&str]) -> Result<Option<Value>, String> {
        let mut index = 0i64;
        for key in path {
            let Some(node) = usize::try_from(index).ok().and_then(|i| self.0.get(i)) else {
                return if index == -1 {
                    Ok(None)
                } else {
                    Err("invalid devalue reference".into())
                };
            };
            let child = match node {
                Value::Object(m) => m.get(*key),
                Value::Array(a) if a.first().is_none_or(|v| !v.is_string()) => {
                    key.parse::<usize>().ok().and_then(|i| a.get(i))
                }
                _ => return Err("unexpected value on Workflow field path".into()),
            };
            let Some(child) = child else { return Ok(None) };
            index = child.as_i64().ok_or("invalid devalue reference")?;
        }
        let mut budget = MAX_BYTES;
        self.expand(index, &mut HashSet::new(), &mut budget)
            .map(Some)
    }

    fn expand(
        &self,
        index: i64,
        active: &mut HashSet<i64>,
        budget: &mut usize,
    ) -> Result<Value, String> {
        if index == -1 || index == -2 {
            return Ok(Value::Null);
        }
        if *budget == 0 || active.len() >= 128 {
            return Err("devalue expansion budget exceeded".into());
        }
        *budget -= 1;
        let node = usize::try_from(index)
            .ok()
            .and_then(|i| self.0.get(i))
            .ok_or("invalid devalue reference")?;
        if !active.insert(index) {
            return Err("cyclic Workflow data".into());
        }
        let out = match node {
            Value::Object(m) => {
                let mut out = Map::new();
                for (key, value) in m {
                    *budget = budget
                        .checked_sub(key.len())
                        .ok_or("devalue expansion budget exceeded")?;
                    out.insert(
                        key.clone(),
                        self.expand(
                            value.as_i64().ok_or("invalid devalue reference")?,
                            active,
                            budget,
                        )?,
                    );
                }
                Value::Object(out)
            }
            Value::Array(a) => {
                if a.first().is_some_and(Value::is_string) {
                    return Err("unsupported runtime value in history field".into());
                }
                let mut out = vec![];
                for v in a {
                    out.push(self.expand(
                        v.as_i64().ok_or("invalid devalue reference")?,
                        active,
                        budget,
                    )?);
                }
                Value::Array(out)
            }
            Value::String(s) => {
                *budget = budget
                    .checked_sub(s.len())
                    .ok_or("devalue expansion budget exceeded")?;
                node.clone()
            }
            _ => node.clone(),
        };
        active.remove(&index);
        Ok(out)
    }
}

/// Eve's structural ValueDelta, applied only to the selected native channel.
pub(super) fn delta(base: &Value, change: &Value) -> Result<Value, String> {
    fn apply(base: &Value, d: &Value, depth: usize) -> Result<Value, String> {
        if depth > 128 {
            return Err("nested Eve delta exceeds decoding budget".into());
        }
        for key in ["set", "patch"] {
            if d.get(key).is_some_and(|v| !v.is_object()) {
                return Err("invalid Eve delta entries".into());
            }
        }
        match d["kind"].as_str() {
            Some("value") => d
                .get("value")
                .cloned()
                .ok_or_else(|| "missing Eve delta value".into()),
            Some("object") => {
                let base = base.as_object().ok_or("Eve object delta has no base")?;
                let keys: Vec<String> = if let Some(keys) = d.get("keys") {
                    keys.as_array()
                        .ok_or("invalid Eve delta keys")?
                        .iter()
                        .map(|k| {
                            k.as_str()
                                .map(str::to_owned)
                                .ok_or_else(|| "invalid Eve delta key".to_string())
                        })
                        .collect::<Result<_, _>>()?
                } else {
                    let mut keys: Vec<_> = base.keys().cloned().collect();
                    if let Some(set) = d["set"].as_object() {
                        keys.extend(set.keys().filter(|k| !base.contains_key(*k)).cloned());
                    }
                    keys
                };
                let mut out = Map::new();
                for k in keys {
                    let v = if let Some(v) = d["set"].get(&k) {
                        v.clone()
                    } else if let Some(patch) = d["patch"].get(&k) {
                        apply(base.get(&k).unwrap_or(&Value::Null), patch, depth + 1)?
                    } else {
                        base.get(&k).cloned().ok_or("missing Eve delta base key")?
                    };
                    out.insert(k, v);
                }
                Ok(Value::Object(out))
            }
            Some("array") => {
                let base = base.as_array().ok_or("Eve array delta has no base")?;
                let len = d["length"]
                    .as_u64()
                    .and_then(|n| usize::try_from(n).ok())
                    .ok_or("invalid Eve array length")?;
                let set = d["set"].as_object();
                // Eve sends arrays with holes as whole values, never sparse
                // deltas. Bound allocation by the actual supplied entries.
                if len > base.len().saturating_add(set.map_or(0, Map::len)) {
                    return Err("incomplete Eve array delta".into());
                }
                for entries in [set, d["patch"].as_object()].into_iter().flatten() {
                    for k in entries.keys() {
                        if k.parse::<usize>()
                            .ok()
                            .is_none_or(|i| i >= len || i.to_string() != *k)
                        {
                            return Err("invalid Eve array index".into());
                        }
                    }
                }
                let mut out = Vec::with_capacity(len);
                for i in 0..len {
                    let key = i.to_string();
                    // Native array deltas apply patch after set, unlike object
                    // deltas. diffValue never emits both for one entry.
                    let value = if let Some(p) = d["patch"].get(&key) {
                        apply(base.get(i).ok_or("missing Eve array base")?, p, depth + 1)?
                    } else if let Some(v) = d["set"].get(&key) {
                        v.clone()
                    } else {
                        base.get(i).cloned().ok_or("missing Eve array entry")?
                    };
                    out.push(value);
                }
                Ok(Value::Array(out))
            }
            _ => Err("unsupported Eve channel delta".into()),
        }
    }
    apply(base, change, 0)
}

pub(super) fn context_after(
    base: &Value,
    output: &Graph,
    key: &str,
) -> Result<Option<Value>, String> {
    let prefix = ["stateDelta", "serializedContext"];
    let mut path = prefix.to_vec();
    path.push("kind");
    let kind = output.get(&path)?;
    match kind.as_ref().and_then(Value::as_str) {
        None => Ok(Some(base.clone())),
        Some("value") => output.get(&["stateDelta", "serializedContext", "value", key]),
        Some("object") => {
            if let Some(keys) = output.get(&["stateDelta", "serializedContext", "keys"])? {
                if !keys
                    .as_array()
                    .ok_or("invalid Eve context keys")?
                    .iter()
                    .any(|k| k == key)
                {
                    return Ok(None);
                }
            }
            if let Some(v) = output.get(&["stateDelta", "serializedContext", "set", key])? {
                return Ok(Some(v));
            }
            if let Some(d) = output.get(&["stateDelta", "serializedContext", "patch", key])? {
                return delta(base, &d).map(Some);
            }
            Ok(Some(base.clone()))
        }
        _ => Err("unsupported Eve context delta".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn native_reference_graph_and_structural_delta_restore_github_checkout() {
        let checkpoint: Value =
            serde_json::from_str(include_str!("../tests/fixtures/eve/checkpoint.json")).unwrap();
        let input = Graph::read(&checkpoint["input"]).unwrap();
        let output = Graph::read(&checkpoint["output"]).unwrap();
        let before = input
            .get(&["args", "0", "serializedContext", "eve.channel"])
            .unwrap()
            .unwrap();
        assert!(before["state"]["checkoutPath"].is_null());
        let after = context_after(&before, &output, "eve.channel")
            .unwrap()
            .unwrap();
        assert_eq!(after["state"]["checkoutPath"], "/app/repository");
        assert_eq!(after["state"]["owner"], "creditia-fixtures");
    }

    #[test]
    fn opaque_fields_are_not_revived_and_bad_graphs_or_deltas_fail_closed() {
        let graph =
            Graph::read(&json!([{"needed":1,"stream":2},"known",["WritableStream",2]])).unwrap();
        assert_eq!(graph.get(&["needed"]).unwrap(), Some(json!("known")));
        assert!(graph.get(&["stream"]).is_err());
        assert!(Graph::read(&json!([{"self":0}])).unwrap().get(&[]).is_err());
        assert!(Graph::read(&json!([{"bad":90}]))
            .unwrap()
            .get(&["bad"])
            .is_err());
        assert!(delta(&json!([]), &json!({"kind":"array","length":4294967295u64})).is_err());
        assert!(delta(
            &json!({"path":"old"}),
            &json!({"kind":"object","set":"broken"})
        )
        .is_err());
        assert_eq!(delta(&json!([{"x":1},2]), &json!({"kind":"array","length":3,"set":{"2":3},"patch":{"0":{"kind":"object","set":{"x":9}}}})).unwrap(), json!([{"x":9},2,3]));
        assert_eq!(
            delta(
                &json!({"keep":1,"remove":2}),
                &json!({"kind":"object","keys":["keep"]})
            )
            .unwrap(),
            json!({"keep":1})
        );
    }
}
