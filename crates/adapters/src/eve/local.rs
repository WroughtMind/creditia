//! Bind the official eve dev /source mount to its authored agent directory.
//! A subagent name, /source path, or mutable manifest alone proves nothing.
use super::{n, BTreeSet, Binding, Digest, Path, Sha256, Value};

pub(super) fn binding(
    root: &Path,
    session: &str,
    bundle: Option<&Value>,
    revision: Option<&Value>,
) -> Option<Binding> {
    let bundle = bundle?;
    if bundle["source"]["kind"] != "development"
        || !session
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_')
    {
        return None;
    }
    let root = root.canonicalize().ok()?;
    if root.file_name()? != ".eve" {
        return None;
    }
    let app = root.parent()?;
    let run = n::json(&root.join(format!(".workflow-data/runs/{session}.json"))).ok()?;
    let generation = run["deploymentId"].as_str()?;
    if run["runId"] != session
        || run["workflowName"] != "workflow//eve//workflowEntry"
        || run["attributes"]["$eve.version"] != "0.83.0"
        || generation.is_empty()
        || !generation
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-')
    {
        return None;
    }
    let generation = root.join("dev-runtime/snapshots").join(generation);
    let source = generation.join("source");
    let meta = n::json(&generation.join("generation.json")).ok()?;
    if meta["runtimeAppRoot"].as_str()? != source.to_str()? {
        return None;
    }
    let compile = source.join(".eve/compile");
    let bytes = std::fs::read(compile.join("compiled-agent-manifest.json")).ok()?;
    // eve dev rebases the manifest into the immutable snapshot, while copying
    // the original compile-metadata unchanged. Verify its original bytes.
    let original = std::str::from_utf8(&bytes)
        .ok()?
        .replace(source.to_str()?, app.to_str()?);
    let metadata = n::json(&compile.join("compile-metadata.json")).ok()?;
    let hash = metadata["discovery"]["sourceGraphHash"].as_str()?;
    let revision = revision?.as_str()?;
    let suffix = revision.strip_prefix(&format!("disk:{}:", source.display()))?;
    let (loader, end) = suffix.rsplit_once(&format!(":{hash}"))?;
    if hash.len() != 64
        || !hash.bytes().all(|b| b.is_ascii_hexdigit())
        || !loader.starts_with("authored-source:/")
        || !(end.is_empty()
            || end
                .strip_prefix(":mtime-")
                .is_some_and(|s| !s.is_empty() && s.bytes().all(|b| b.is_ascii_alphanumeric())))
        || metadata["kind"] != "eve-compile-metadata"
        || metadata["version"] != 6
        || metadata["status"] != "ready"
        || metadata["compile"]["manifest"]["sha256"]
            != format!("{:x}", Sha256::digest(original.as_bytes()))
    {
        return None;
    }
    let manifest: Value = serde_json::from_slice(&bytes).ok()?;
    if manifest["kind"] != "eve-agent-compiled-manifest" || manifest["version"] != 54 {
        return None;
    }
    let node = match bundle["nodeId"].as_str() {
        None | Some("__root__") => &manifest,
        Some(id) => &manifest["subagents"]
            .as_array()?
            .iter()
            .find(|n| n["nodeId"] == id)?["agent"],
    };
    let sandbox = &node["sandbox"];
    if sandbox["providerName"] != "just-bash" || sandbox["inheritsParent"] == true {
        return None;
    }
    let backing = &node["bindings"][sandbox["sourceId"].as_str()?]["backing"];
    let path = Path::new(backing["sourcePath"].as_str()?);
    let agent = app.join("agent");
    if backing["kind"] != "filesystem"
        || !path.starts_with(&agent)
        || path.canonicalize().ok()?.as_path() != path
    {
        return None;
    }
    // The compiler hashes the exact authored bytes, including whitespace.
    // Reading today's file is safe only if it still matches that old revision.
    let source = std::fs::read_to_string(path).ok()?;
    let mut digest = Sha256::new();
    digest.update(b"sandbox-source\0");
    digest.update(source.as_bytes());
    if sandbox["revisionHash"] != format!("{:x}", digest.finalize()) {
        return None;
    }
    let re = regex::Regex::new(
        r#"^\s*export\s*\{\s*default\s*\}\s*from\s*["']eve/self-modification/sandbox["']\s*;?\s*$"#,
    )
    .ok()?;
    if !re.is_match(&source) {
        return None;
    }
    Some(Binding {
        remote: agent.to_str()?.into(),
        checkout: "/source".into(),
        local: true,
        patch_tools: BTreeSet::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn write(path: &Path, value: &Value) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, value.to_string()).unwrap();
    }
    #[test]
    fn local_mount_requires_recorded_generation_and_exact_sandbox_source() {
        let temp = std::env::temp_dir().join(format!(
            "creditia-eve-binding-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(temp.join("agent")).unwrap();
        let temp = temp.canonicalize().unwrap();
        let root = temp.join(".eve");
        let source = root.join("dev-runtime/snapshots/generation-1/source");
        let sandbox_path = temp.join("agent/sandbox.ts");
        let sandbox = "export { default } from 'eve/self-modification/sandbox';\n";
        std::fs::write(&sandbox_path, sandbox).unwrap();
        let mut hash = Sha256::new();
        hash.update(b"sandbox-source\0");
        hash.update(sandbox);
        let manifest = json!({
            "kind":"eve-agent-compiled-manifest","version":54,"appRoot":temp,
            "sandbox":{"providerName":"just-bash","sourceId":"sandbox.ts","revisionHash":format!("{:x}",hash.finalize())},
            "bindings":{"sandbox.ts":{"backing":{"kind":"filesystem","sourcePath":sandbox_path}}}
        });
        let bytes = manifest.to_string();
        let mut snapshot = manifest.clone();
        snapshot["appRoot"] = json!(source);
        write(
            &source.join(".eve/compile/compiled-agent-manifest.json"),
            &snapshot,
        );
        write(
            &source.join(".eve/compile/compile-metadata.json"),
            &json!({
                "kind":"eve-compile-metadata","version":6,"status":"ready",
                "discovery":{"sourceGraphHash":"a".repeat(64)},
                "compile":{"manifest":{"sha256":format!("{:x}",Sha256::digest(bytes.as_bytes()))}}
            }),
        );
        write(
            &source.parent().unwrap().join("generation.json"),
            &json!({"runtimeAppRoot":source}),
        );
        let run = root.join(".workflow-data/runs/wrun_live.json");
        let native = json!({"runId":"wrun_live","workflowName":"workflow//eve//workflowEntry","deploymentId":"generation-1","attributes":{"$eve.version":"0.83.0"}});
        write(&run, &native);
        let bundle = json!({"source":{"kind":"development"}});
        let revision = json!(format!(
            "disk:{}:authored-source:/eve/loader.js:{}:mtime-abc",
            source.display(),
            "a".repeat(64)
        ));
        let read = || binding(&root, "wrun_live", Some(&bundle), Some(&revision));
        let bound = read().unwrap();
        assert_eq!(bound.remote, temp.join("agent").to_str().unwrap());
        assert_eq!(bound.checkout, "/source");
        assert!(bound.local);
        std::fs::write(&sandbox_path, "export { default } from 'eve/sandbox';\n").unwrap();
        assert!(
            read().is_none(),
            "a different filesystem must not reuse old receipts"
        );
        std::fs::write(&sandbox_path, sandbox).unwrap();
        let mut wrong = native.clone();
        wrong["deploymentId"] = json!("another-generation");
        write(&run, &wrong);
        assert!(
            read().is_none(),
            "the current manifest cannot stand in for a missing old generation"
        );
        write(&run, &native);
        snapshot["sandbox"]["providerName"] = json!("default");
        write(
            &source.join(".eve/compile/compiled-agent-manifest.json"),
            &snapshot,
        );
        assert!(
            read().is_none(),
            "a modified or virtual sandbox manifest must fail verification"
        );
        std::fs::remove_dir_all(temp).unwrap();
    }
}
