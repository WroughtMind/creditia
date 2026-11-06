# GPUI candidate releases

`gpui-release.yml` creates a **draft prerelease** from already tested installers.
It never rebuilds them, updates the main branch, replaces an existing tag, changes
the Tauri release workflow, or promotes a stable version. No signing certificate
is required; the candidate's recorded signing policy remains visible in the
installation guide and release notes.

Both Mac packages use the original five-petal Icon Composer source. Compile it
once on macOS 26 with `DEVELOPER_DIR=/Applications/Xcode_26.6.app/Contents/Developer
node packaging/gpui/mac-icons.mjs compile --output mac-icon-assets` and pass
`--mac-icons mac-icon-assets` to each architecture's staging command. The builder
verifies the light, dark and tinted stacks before the macOS 15 runners install the
same asset catalog and original `.icns` fallback, sign the bundle, and hash it.

The workflow can be started manually after it is available in the default branch.
Before then, an explicit repository push that changes
`packaging/gpui/release-request.json` can request the same operation. The committed
template has `enabled:false`, so shipping this workflow does not create a release.

Provide the full **tested** candidate commit recorded in the evidence manifests.
For a pull request this is normally GitHub's checkout merge commit, not the PR
branch head. Supply successful run IDs for `gpui`, `gpui-artworks`,
`gpui-performance`, and `ci`, all bound to that candidate, a new
`gpui-vX.Y.Z-rc.N` tag, and a full commit/path containing the completed review JSON:

```json
{
  "schema_version": 1,
  "enabled": true,
  "candidate_sha": "FULL_TESTED_40_CHARACTER_COMMIT",
  "native_run": "NATIVE_WORKFLOW_RUN_ID",
  "artwork_run": "ARTWORK_WORKFLOW_RUN_ID",
  "performance_run": "PERFORMANCE_WORKFLOW_RUN_ID",
  "core_run": "CORE_AND_BROWSER_WORKFLOW_RUN_ID",
  "review_commit": "FULL_40_CHARACTER_REVIEW_COMMIT",
  "review_path": "docs/gpui-migration/reviews/review.json",
  "tag": "gpui-v0.2.0-rc.1"
}
```

The review uses `packaging/gpui-acceptance/report.mjs`'s existing schema: candidate
SHA, reviewer identity, review timestamp, artwork visual/motion reviews, and every
named feature check with evidence references. The review can be committed after
the test candidate; it must still identify that exact tested candidate. Evidence
references are relative to a download root whose immediate directories use the
GitHub artifact names, for example `gpui-evidence-aarch64-apple-darwin/…/manifest.json#CASE`.
Keep the exact paths below those artifact roots as uploaded. A review with missing
or nonmatching references fails; the release workflow never rewrites reviews or
marks unreviewed work accepted.

Preparation has read-only repository permissions. It verifies successful workflow
conclusions, workflow identity, repository and tested commit relationships. It
then downloads each uniquely identified v4 artifact by ID and verifies GitHub's
archive SHA-256 before safely extracting it. Missing, expired, duplicate or
unverifiable artifacts fail the job. It recomputes the complete acceptance report
from raw evidence with `--require-complete`, including all four native platforms,
the full artwork/browser/native catalogs, all preference/export/motion coverage,
recorded review and twelve comparable performance scenarios.

Each installer must match both its candidate's checksum manifest and the exact
bytes recorded by the installed-package smoke test. The next job has narrowly
scoped write permission, downloads the prepared artifact by its immutable ID,
rechecks the sealed bundle, and uses `gh release create --draft --prerelease`.
An existing tag is never replaced. If uploading fails, the draft remains a draft;
inspect and resolve that incomplete draft before retrying with the same tag.

Release assets include the eight installers/archives, installation instructions,
combined `report.json`, `REPORT.md` and `ACCEPTANCE.zh.md`, per-row `ARTWORK-STATUS.tsv` and `FEATURE-STATUS.tsv`, SHA256SUMS, and all raw evidence plus
its review and immutable run/artifact provenance. Evidence archives larger than
1 GiB are split into ordered parts. `evidence-archive.json` records the part hashes
and the reconstructed archive hash. Concatenate parts in listed order, verify
the reconstructed SHA-256, then extract the tar.gz. This keeps individual assets
below GitHub's upload limit without dropping full-resolution native screenshots.

The draft remains available for final distribution review. Publishing it or
changing the stable desktop/main branch is a separate action; this workflow does
neither automatically.
