//! Capture public Git provenance locally for review. Do not publish this raw
//! output: identities, paths and original line fingerprints require sanitizing.
use creditia_adapters::{git, squash};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().collect();
    if args.len() != 5 {
        return Err("usage: squash_replay REPO MERGE HEAD OUTPUT".into());
    }
    let repo = std::path::Path::new(&args[1]);
    let (mut commits, _) =
        git::read_commits(repo, &args[2], Some(&format!("{}^..{}", args[2], args[2])));
    if commits.len() != 1 {
        return Err("expected one squash commit".into());
    }
    let commit = commits.remove(0);
    let history = squash::local_history(repo, &commit, &args[3])?;
    std::fs::write(
        &args[4],
        serde_json::to_vec(&serde_json::json!({"commit":commit,"history":history}))?,
    )?;
    Ok(())
}
