//! Original devalue/OTLP serializers and Eve's actual patch executor generated
//! these controlled fixtures. They are not a real user's account history.
use super::*;
use base64::{engine::general_purpose::STANDARD, Engine};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

fn fixture(name: &str) -> Value {
    serde_json::from_slice(
        &fs::read(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../adapters/tests/fixtures/eve")
                .join(name),
        )
        .unwrap(),
    )
    .unwrap()
}
fn envelope(bytes: &[u8]) -> Value {
    json!({"__type":"Uint8Array","data":STANDARD.encode(bytes)})
}
fn checkpoint(env: &Env) -> Value {
    let mut v = fixture("checkpoint.json");
    for key in ["input", "output"] {
        let bytes = STANDARD.decode(v[key]["data"].as_str().unwrap()).unwrap();
        let mut graph: Value = serde_json::from_slice(&bytes[4..]).unwrap();
        for v in graph.as_array_mut().unwrap() {
            if let Some(s) = v.as_str() {
                *v = json!(s.replace(
                    "__EVE_BUNDLE_ROOT__",
                    env.root.join("eve-app").to_str().unwrap()
                ));
            }
        }
        let mut bytes = b"devl".to_vec();
        bytes.extend(graph.to_string().as_bytes());
        v[key] = envelope(&bytes);
    }
    v
}
fn store_json(path: &Path, v: &Value) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, v.to_string()).unwrap();
}
fn span(v: &mut Value) -> &mut Value {
    &mut v["resourceSpans"][0]["scopeSpans"][0]["spans"][0]
}
fn attr(v: &mut Value, key: &str, value: Value) {
    let attrs = span(v)["attributes"].as_array_mut().unwrap();
    let kind = if value.is_boolean() {
        "boolValue"
    } else {
        "stringValue"
    };
    attrs.retain(|a| a["key"] != key);
    attrs.push(json!({"key":key,"value":{kind:value}}));
}
fn count(env: &Env, expected: i64) {
    let db = rusqlite::Connection::open(env.root.join("lib/library.sqlite")).unwrap();
    let n: i64 = db
        .query_row(
            "select count(*) from evidence where src='eve' and model=''",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(n, expected, "durable Eve evidence");
    let v: Value =
        serde_json::from_slice(&fs::read(env.repo.join(".creditia/credits.json")).unwrap())
            .unwrap();
    let lines: i64 = v["breakdown"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|b| b["contributor"] == "a:eve")
        .map(|b| {
            b["byEvidence"]["verified"]["linesAdded"]
                .as_i64()
                .unwrap_or(0)
        })
        .sum();
    assert_eq!(lines, expected, "final Eve credits");
}
fn native_store(env: &Env) -> (PathBuf, PathBuf, PathBuf) {
    git(
        &env.repo,
        &[
            "remote",
            "add",
            "origin",
            "https://github.com/creditia-fixtures/native-eve.git",
        ],
    );
    fs::write(env.repo.join("README.md"), "human baseline\n").unwrap();
    commit(&env.repo, "base", "me@example.com", "2026-09-17T00:00:00Z");
    let expected = fixture("expected.json");
    fs::write(
        env.repo.join("created.ts"),
        expected["created"].as_str().unwrap(),
    )
    .unwrap();
    fs::write(
        env.repo.join("patched.ts"),
        expected["patched"].as_str().unwrap(),
    )
    .unwrap();
    commit(&env.repo, "save", "me@example.com", "2026-09-17T00:01:00Z");
    let root = env.root.join(".eve");
    let step = root.join(".workflow-data/steps/wrun_fixture/wrun_fixture-step_fixture.json");
    store_json(&step, &checkpoint(env));
    let segments = root.join("traces/v1/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa/segments");
    let write = segments.join("0000000000000001.otlp.json");
    let patch = segments.join("0000000000000002.otlp.json");
    store_json(&write, &fixture("write.otlp.json"));
    store_json(&patch, &fixture("patch.otlp.json"));
    let source = env.root.join("eve-app/node_modules/eve-code/dist");
    store_json(
        &env.root
            .join("eve-app/.eve/compile/compiled-agent-manifest.json"),
        &json!({
            "kind":"eve-agent-compiled-manifest","version":54,
            "tools":[{"name":"code__apply_patch","sourceId":"patch-source","sourceKind":"module","hasExecute":true}],
            "bindings":{"patch-source":{"backing":{"kind":"filesystem","sourcePath":source.join("tools/apply_patch.js"),"mountId":"code-mount"}}},
            "extensionMounts":[{"packageName":"eve-code","mountId":"code-mount","sourceRoot":source}]
        }),
    );
    let directory = env.root.join("eve-app/.eve/compile");
    let digest = format!(
        "{:x}",
        Sha256::digest(fs::read(directory.join("compiled-agent-manifest.json")).unwrap())
    );
    store_json(
        &directory.join("compile-metadata.json"),
        &json!({
            "kind":"eve-compile-metadata","version":6,"status":"ready",
            "compile":{"manifest":{"sha256":digest}},
            "discovery":{"sourceGraphHash":"b".repeat(64)}
        }),
    );
    (step, write, patch)
}

#[test]
fn eve_native_cli_export_and_packaged_checkpoint() {
    let env = setup();
    let rs = env.repo.display().to_string();
    let (step, write, patch) = native_store(&env);
    let mut checkpoint = checkpoint(&env);
    checkpoint["stepName"] = json!("step//eve@0.83.0//turnStep");
    store_json(&step, &checkpoint);
    let mut spans = vec![];
    // Same fields as Eve's serializeTraceForJson, with native OTLP receipts.
    for path in [&write, &patch] {
        let v: Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
        let s = &v["resourceSpans"][0]["scopeSpans"][0]["spans"][0];
        let attrs: serde_json::Map<String, Value> = s["attributes"]
            .as_array()
            .unwrap()
            .iter()
            .map(|a| {
                (
                    a["key"].as_str().unwrap().to_string(),
                    a["value"]
                        .as_object()
                        .unwrap()
                        .values()
                        .next()
                        .unwrap()
                        .clone(),
                )
            })
            .collect();
        spans.push(json!({"attributes":attrs,"spanId":s["spanId"],"name":s["name"],"statusCode":s["status"]["code"],"startTimeNs":s["startTimeUnixNano"],"endTimeNs":s["endTimeUnixNano"]}));
    }
    let mut unbound = spans[0].clone();
    unbound["attributes"]["agent.run.id"] = json!("unbound-other-session");
    spans.push(unbound);
    let export = env.root.join(".eve/traces/export.json");
    let mut value = json!([{"traceId":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","spans":spans}]);
    store_json(&export, &value);
    assert!(run(&env, &["init", "--repo", &rs]).0);
    for round in 0..3 {
        if round == 1 {
            fs::remove_file(&write).unwrap();
            fs::remove_file(&patch).unwrap();
        }
        if round == 2 {
            value[0]["spans"][0]["statusCode"] = json!(2);
            store_json(&export, &value);
        }
        let original = fs::read(&export).unwrap();
        let (ok, result) = run(&env, &["analyze", "--repo", &rs]);
        assert!(ok, "{result}");
        count(&env, if round == 2 { 8 } else { 16 });
        assert_eq!(fs::read(&export).unwrap(), original);
    }
    fs::remove_dir_all(env.root).unwrap();
}

#[test]
fn eve_native_receipts_compression_dedup_and_recovery() {
    use std::io::Write;
    let env = setup();
    let rs = env.repo.display().to_string();
    let (step, write, patch) = native_store(&env);
    let original = checkpoint(&env);
    let write_bytes = fs::read(&write).unwrap();
    let patch_bytes = fs::read(&patch).unwrap();
    assert!(run(&env, &["init", "--repo", &rs]).0);
    for round in 0..7 {
        let mut v = original.clone();
        match round {
            1 | 2 => {
                for key in ["input", "output"] {
                    let raw = STANDARD.decode(v[key]["data"].as_str().unwrap()).unwrap();
                    let bytes = if round == 1 {
                        let mut encoder = flate2::write::GzEncoder::new(
                            Vec::new(),
                            flate2::Compression::default(),
                        );
                        encoder.write_all(&raw).unwrap();
                        [b"gzip".as_slice(), encoder.finish().unwrap().as_slice()].concat()
                    } else {
                        [
                            b"zstd".as_slice(),
                            zstd::encode_all(raw.as_slice(), 3).unwrap().as_slice(),
                        ]
                        .concat()
                    };
                    v[key] = envelope(&bytes);
                }
            }
            3 => {
                fs::copy(&write, write.parent().unwrap().join("copy.otlp.json")).unwrap();
            }
            4 => {
                v["output"] = envelope(b"encrunreadable-without-key");
            }
            5 => {
                v["input"] = envelope(b"devl[{\"broken\":999}]");
            }
            6 => {
                fs::remove_file(write.parent().unwrap().join("copy.otlp.json")).unwrap();
            }
            _ => {}
        }
        store_json(&step, &v);
        let before = fs::read(&step).unwrap();
        let (ok, result) = run(&env, &["analyze", "--repo", &rs]);
        assert!(ok, "{result}");
        count(&env, 16);
        assert_eq!(fs::read(&step).unwrap(), before);
        assert_eq!(fs::read(&write).unwrap(), write_bytes);
        assert_eq!(fs::read(&patch).unwrap(), patch_bytes);
    }
    let (ok, found) = run(&env, &["discover", "--all"]);
    assert!(ok, "{found}");
    assert!(found.to_string().contains("native-eve"), "{found}");
    let (ok, health) = run(&env, &["doctor"]);
    assert!(ok, "{health}");
    assert!(health
        .to_string()
        .contains("OTLP / traces JSON + Workflow checkpoints"));
    // Explicit failure of the same call retracts only that call, not its peer.
    let mut failed = fixture("write.otlp.json");
    span(&mut failed)["status"]["code"] = json!(2);
    store_json(&write, &failed);
    assert!(run(&env, &["analyze", "--repo", &rs]).0);
    count(&env, 8);
    fs::remove_file(&step).unwrap();
    fs::remove_file(&patch).unwrap();
    assert!(run(&env, &["analyze", "--repo", &rs]).0);
    count(&env, 8);
    fs::remove_dir_all(env.root).unwrap();
}

#[test]
fn eve_rejects_unbound_failed_proposed_and_custom_file_calls() {
    for case in [
        "foreign-repo",
        "ambiguous",
        "failed",
        "action-failed",
        "action-denied",
        "direct-denied",
        "proposal",
        "outside",
        "overwrite",
        "custom",
        "wrong-manifest",
        "rebuilt-manifest",
        "missing-manifest",
        "missing-receipt-file",
    ] {
        let env = setup();
        let rs = env.repo.display().to_string();
        let (step, write, patch) = native_store(&env);
        let mut w = fixture("write.otlp.json");
        let mut p = fixture("patch.otlp.json");
        let expected = fixture("expected.json");
        let expected_count = match case {
            "foreign-repo" => {
                git(
                    &env.repo,
                    &[
                        "remote",
                        "set-url",
                        "origin",
                        "https://github.com/other/native-eve.git",
                    ],
                );
                0
            }
            "ambiguous" => {
                let mut v = checkpoint(&env);
                let mut bytes = STANDARD
                    .decode(v["output"]["data"].as_str().unwrap())
                    .unwrap();
                bytes = String::from_utf8(bytes)
                    .unwrap()
                    .replace("/app/repository", "/other/repository")
                    .into_bytes();
                v["output"] = envelope(&bytes);
                store_json(&step.with_file_name("second.json"), &v);
                0
            }
            "failed" => {
                span(&mut w)["status"]["code"] = json!(2);
                span(&mut p)["status"]["code"] = json!(2);
                0
            }
            "action-failed" | "action-denied" => {
                for value in [&mut w, &mut p] {
                    attr(
                        value,
                        "agent.action.outcome",
                        json!(case.trim_start_matches("action-")),
                    );
                }
                0
            }
            "direct-denied" => {
                for value in [&mut w, &mut p] {
                    attr(value, "eve.tool.outcome", json!("denied"));
                }
                0
            }
            "proposal" => {
                for v in [&mut w, &mut p] {
                    span(v)["attributes"]
                        .as_array_mut()
                        .unwrap()
                        .retain(|a| a["key"] != "gen_ai.tool.call.result");
                }
                0
            }
            "outside" => {
                attr(
                    &mut w,
                    "gen_ai.tool.call.result",
                    json!(
                        json!({"existed":false,"path":"/app/repository/../other/created.ts"})
                            .to_string()
                    ),
                );
                attr(&mut p, "gen_ai.tool.call.arguments", json!(json!({"root":"/app/repository/../other","patchText":"*** Begin Patch\n*** Add File: patched.ts\n+not real\n*** End Patch"}).to_string()));
                0
            }
            "overwrite" => {
                attr(
                    &mut w,
                    "gen_ai.tool.call.result",
                    json!(json!({"existed":true,"path":"/app/repository/created.ts"}).to_string()),
                );
                8
            }
            "custom" => {
                attr(&mut w, "agent.tool.is_framework", json!(false));
                8
            }
            "wrong-manifest" => {
                let f = env
                    .root
                    .join("eve-app/.eve/compile/compiled-agent-manifest.json");
                let mut v: Value = serde_json::from_slice(&fs::read(&f).unwrap()).unwrap();
                v["extensionMounts"][0]["packageName"] = json!("unrelated-plugin");
                store_json(&f, &v);
                let metadata = f.with_file_name("compile-metadata.json");
                let mut m: Value = serde_json::from_slice(&fs::read(&metadata).unwrap()).unwrap();
                m["compile"]["manifest"]["sha256"] =
                    json!(format!("{:x}", Sha256::digest(fs::read(&f).unwrap())));
                store_json(&metadata, &m);
                8
            }
            "rebuilt-manifest" => {
                let f = env.root.join("eve-app/.eve/compile/compile-metadata.json");
                let mut v: Value = serde_json::from_slice(&fs::read(&f).unwrap()).unwrap();
                v["discovery"]["sourceGraphHash"] = json!("c".repeat(64));
                store_json(&f, &v);
                8
            }
            "missing-manifest" => {
                fs::remove_file(env.root.join("eve-app/.eve/compile/compile-metadata.json"))
                    .unwrap();
                8
            }
            "missing-receipt-file" => {
                attr(
                    &mut p,
                    "gen_ai.tool.call.result",
                    json!(json!({"diagnostics":[],"files":[]}).to_string()),
                );
                8
            }
            _ => unreachable!(),
        };
        // Same native content really exists in Git; rejection cannot be caused by
        // absent code, an unknown path, or a coincidentally nonmatching line.
        assert_eq!(
            fs::read_to_string(env.repo.join("created.ts")).unwrap(),
            expected["created"].as_str().unwrap()
        );
        store_json(&write, &w);
        store_json(&patch, &p);
        assert!(run(&env, &["init", "--repo", &rs]).0);
        let (ok, result) = run(&env, &["analyze", "--repo", &rs]);
        assert!(ok, "{case}: {result}");
        count(&env, expected_count);
        fs::remove_dir_all(env.root).unwrap();
    }
}
