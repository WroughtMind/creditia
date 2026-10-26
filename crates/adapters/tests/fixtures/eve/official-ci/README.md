# Original Eve CI file receipts

These two unmodified OTLP segments come from an actual model-driven Eve 0.83.0
`file-tools` evaluation, not from Creditia's controlled fixture generator.

- [Official run 40056903264](https://github.com/vercel/eve/actions/runs/40056903264)
- Source commit: `db0ffaf1d8997ee7f396e67ff974945c08c9e6b0`
- [Artifact 12545831396](https://api.github.com/repos/vercel/eve/actions/artifacts/12545831396), `eval-artifacts-agent-tools-sandbox-openai-sol`
- Archive SHA256: `3ccee524a117052e57e4fece90fc52ef1189e07c84777d387347b330ff461306`, verified against GitHub's artifact digest.
- Original directory: `e2e/fixtures/agent-tools-sandbox/.eve/traces/v1/1ba42b28aab49f6688bd396c5a917b43/segments/`

| Stored name | Original basename | SHA256 |
|---|---|---|
| write.otlp.json | aa636480f301c229.otlp.json | 6c849c051ccd19712042c2f8e4edd22c86b4c5df57ec388bf2ff7fb00e53fb12 |
| read.otlp.json | 26dcdd6739ef79db.otlp.json | 7dd2d88a63dff02fb522456ff9f41a07c19d6964b679b0898098673b7f182a7f |

Both spans are completed framework tool calls in the same native session. The
first creates `/workspace/notes/handoff.txt`; the second reads back its exact
single-line sentinel. Only these public test receipts are retained, with their
bytes unchanged. The full artifact contains unrelated test contexts and is not
vendored. The workflow failed in another evaluation; this file-tools case passed.

This proves positive receipt decoding and native field names, including
`agent.action.outcome`. It does **not** prove project attribution: the sandbox
run has no GitHub-channel checkpoint. The test also asserts that the full reader
emits no repository evidence without that binding. We do not attach a constructed
checkpoint or repurpose the CI source repository as the edited repository.
