# Round 2 correction record — JSON Resume Rust V2

Date: 2026-10-27. Implementation and owned verification only. No staging, commit,
push, PR, release, install, profile/host/SDK/sibling edits, registration changes,
or clone deletion was performed.

Selected clone: `/Users/almat/Desktop/code/Tezbar APP/rust-extension-ports/json-resume`.
Branch: `feat/native-v2-rust`. HEAD: `6cb8e33954fdc5da52af9993d92c6fa7b37e7c79`.
Origin: `https://github.com/almatkai/json-resume.git`. Index remains empty.
Temporary project registration for eventual parent cleanup:
`mcp:cd28d0b2-64c8-49d1-b6b4-7c215cc52584` (unchanged; clone retained).

## Implemented and checked

- Genuine standalone Rust process executable; schema 3, protocol 1, SDK 0.1.1
  from immutable HTTPS Git revision `e955911f0bf5b558f9d3eb5183df0adc95c22af2`.
  Both manifest commands work: `open-resume` and `search-resumes`.
- Original Form/List/Detail behavior, exact valid Markdown, automatic save after
  validation, explicit save/clear, title-only fallback fetch, re-fetch, browser,
  copy and deletion. Detail deletion stays on its pushed detail as the original
  callback does. Native internal navigation supplies the SDK's absent
  launchCommand behavior; no unsupported AI tool is advertised.
- Atomic private bounded storage with fresh transaction reads, URL dedup/order/
  title/date semantics, corruption preservation, explicit record identity and durable per-URL observation fences,
  stale deletion protection and late-autosave conflict recovery.
- Owned cancellable native HTTP, exact frozen source URL/revision, client-owned
  form revisions, no stale A/B/A restoration/save, no effect replay, strict closed
  ACKs, manual frozen effect retry, and bounded SDK/task/session lifecycle.

## Round 2 correction and evidence

The parent regression remains intact. `.qa/parent-absence-aba-before.log`
records the old production failure: an initially absent URL was saved and deleted
while an older valid load waited, and the old load resurrected it. Format 2 now
persists the latest successful save/delete revision for each URL under a SHA-256
key. `Observation::Absent { generation }` and `Observation::Present(RecordIdentity)`
are explicit types. The same locked transaction compares the observation and
writes the record/history atomically. The global checked counter allocates stamps;
it does not veto unrelated URL changes. Full deleted URLs/titles are not retained.

History is bounded to 4,096 distinct hashes without eviction. A new distinct URL
at that limit reports a precise error and preserves the previous bytes; known URL
reuse/deletion remains available within the other limits. The reader validates
hash syntax, positive generation bounds and exact record/stamp history agreement.
Candidate format 1 and unknown formats report explicit unsupported-version errors;
valid legacy records/bytes are preserved without an invented migration. The loader
retains duplicate-struct-field rejection. A failed serialized-size/persistence
operation restores the transaction's previous in-memory snapshot. Lock failures
and checked-counter exhaustion preserve disk bytes. Tempfiles stay in the store
directory. No directory-fsync, crash-durability or exactly-once guarantee is made.
`ring` 0.17.14 supplies SHA-256 and was already locked through TLS; the only lockfile
change is the root's direct dependency edge, with no package/version/checksum churn.

The production pipe suite now exercises these cases using two genuine executable
processes sharing private fixture storage:

- Initially absent URL: A waits; B saves/deletes; A conflicts and leaves exact
  deleted-store bytes intact. A new explicit Save then succeeds.
- An existing deletion generation: the same recreate/delete race conflicts again,
  with no loss of previous tombstone evidence; explicit Save remains usable.
- Existing same-record update: a newer B update survives A's stale completion.
- Old deletion action after B deletes/reuses the URL: the new UUID/record survives.
- Title-only explicit Save pending across B's deletion: no resurrection; a new
  deliberate title-only Save succeeds.
- Previously present snapshot followed by B deletion: the stale completion
  conflicts, preserves absence, and offers a usable Save URL action.
- Different URL concurrent saves: both still persist; no blanket store-revision
  conflict was introduced.

The last case exposed a second recovery issue: a stale saved-list snapshot could
show only Unsave while the error directed the user to Save URL.
`.qa/round2-present-conflict-before.log` reproduces that production failure.
Save errors now explicitly expose Save URL, including when the snapshot still
contains an old record. Existing Unsave identity guards remain active. Starting a
new job/navigation clears this recovery state; successful saving restores the
normal actions. No confirmation or new command was added.

Five focused Rust store tests cover persisted/reopened ABA and repeated reuse,
unrelated writes/order/title preservation, saturated history with known URL reuse,
invalid/legacy/duplicate-field storage preservation, lock contention, counter
overflow, and a real 1 MiB commit failure with disk and memory preservation.
The parent oversized/nontext draft and oversized-submit body-read cancellation
regressions remain unchanged and green, alongside all previous production cases.

Final logs are `.qa/round2-{tests-185,tests-stable,fmt-185,fmt-stable,clippy-185,clippy-stable}.log`,
`.qa/round2-{oracle,host,markdown,pipes,extracted-pipes,extracted-tls}.log`,
`.qa/round2-{v1-standard,v1-install,v1-test,v1-typecheck}.log`,
`.qa/round2-fresh-build.log`, `.qa/round2-package-schema.log`,
`.qa/round2-artifact-audit.json`, `.qa/round2-msrv.json`,
`.qa/round2-notices.log`, and `.qa/round2-windows-gnu-{check,clippy}.log`.
Pipe assertion totals vary with deadline refreshes and cancellation scheduling;
HTTP request totals and required case assertions are deterministic.

The full original TSX commands, helpers, types, tools, tests, manifest, lock and
assets were inspected. Closed VM producers replace filesystem/network/browser/
clipboard/storage/Date before source evaluation. Both Markdown functions are
executed independently. Original-schema validation is executed with the actual
locked `@jsonresume/schema` 1.2.1 and `jsonschema` 1.5.0. The unchanged V1 lock also
pins `@raycast/utils` 2.3.0; its actual MIME/parser functions are executed.

## Verification results

| Check | Result |
|---|---|
| Original TSX/helper/library oracle | 92 assertions; 1,517 schema cases; 90 Markdown outputs from 45 inputs, plus 6 separately executed formatter-error cases |
| Rust original-oracle/schema/storage/ACK/URL/full-schema-3 tests | 11 integration tests pass (6 original + 5 focused store tests); complete fixture matrix compared, including exponent/large-number JavaScript formatting |
| Actual pinned renderer/producer functions | 21 assertions pass; both original defects reproduced; patched form/action callbacks exercised, including concurrent duplicate effect responses |
| Actual pinned Markdown renderer | 5 assertions pass with exact pinned Marked/DOMPurify/highlight libraries and inert DOM; raw Markdown forwarded and host sanitization exercised |
| Production SDK JSONL pipes, final extracted binary | 2,176 assertions, 92 owned synthetic HTTP requests; count includes deadline refresh responses |
| TLS | Owned real TLS succeeds with test-only explicit trust; default production trust rejects that fixture certificate; extracted production process checked |
| Rust 1.85.0 | Locked tests, formatting and strict all-target Clippy pass |
| Rust stable 1.98.0 | Locked tests and strict all-target Clippy pass |
| Production-target dependency declarations | 181 selected packages for ARM64 macOS, 189 for Windows MSVC; none declares MSRV above 1.85 |
| Windows GNU check and all-target Clippy | Attempted; blocked in ring's C build because `x86_64-w64-mingw32-gcc` is absent; no claim that final Windows code compiled |
| Native macOS GUI / Windows GUI / MSVC acceptance | Not run; both manifest verification records remain `not-run` |
| Disposable exact-lock V1 baseline | Standard npm ci hits existing React peer conflict; exact-lock `--legacy-peer-deps` install succeeds, smoke test 1/1 passes and typecheck passes; V1 unchanged |
| Immutable inputs | 24 V1 files audited; only the permitted root README discovery line differs; 27 pinned host/SDK Git objects verified |
| Cargo archives/notices | All 241 locked registry archives verified against actual Cargo.lock SHA256; 444 authentic notice files copied, plus SDK/npm/Raycast grants |
| Icon | Fresh pinned upstream download equals own original Git bytes and packaged bytes |
| Final XERJ update | Incomplete: frozen dataset schema rejects the new inventory JSON fields; no prefix/settings/schema bypass or restart |

The source oracle and actual production suites cover URL trimming/normalization,
schema positives/negative types/formats/null/optional/additional properties,
permissive locked dates, malformed JSON, MIME/string-body coercion, redirects
including exact five/six boundary, body/depth/value bounds, gzip/deflate, title
lookup, name association, ordering, autosave, clear, source and detail failures,
all action families, separate editable/last-submitted/last-valid URL targets,
repeated deliberate copy, strict ACK negatives, frozen
manual retry, duplicate queued action (exactly one effect and one view plus one
unchanged response; either scheduled task may win), A/B/A/invalid/oversized/nontext intermediate
drafts, delayed autosave conflict, absent/create/delete ABA, stale/reused URL deletion, two real processes
sharing storage, private Unix modes, disposal, EOF/shutdown during fetch,
actual socket EOF on response-body cancellation, idle pooling disabled,
140 canceled opens followed by 128 live opens, session overflow, 120 occupied
ordinary handlers with reserved disposal, malformed/oversized/truncated frames,
and graceful shutdown. No real clipboard/browser/profile/API operation is used.

## Development artifact

The final production build fetched the immutable HTTPS SDK in a new empty Cargo
home **distinct from** its new empty target directory. Existing Rust 1.85.0 was
used; no toolchain was installed. Paths are recorded in
`.qa/fresh-build-paths.txt`. The downloaded SDK checkout and runner bytes were
independently checked against the pin. Source paths are remapped at compilation. The final two roots are
`.qa/fresh-cargo-qby1oMVq/` and `.qa/fresh-target-RGgx7SlJ/`.
`verify-artifact.py` independently verifies the freshly fetched SDK snapshot bytes,
all 179 downloaded fresh-Cargo registry archives against the lockfile, deterministic
closed ZIP contents/hash/modes, extracted resources, protocol info, arm64 architecture,
system-only dylibs, and absence of fixture/clone/Node strings in the executable.

- ZIP: `.qa/json-resume-v2-aarch64-apple-darwin-dev.zip`
- ZIP SHA256: `42748ed4811273cf2a515d02ff395d6e96814d730d7ba8f831081d3c510f4977`
- Executable SHA256: `6e63a2012e72087eb67e44e2731a716affc3347664198de402c84ef8f872ace2`
- 456 closed members; executable mode 0755, remaining files 0644; fixed ZIP times.
  A second generated archive is byte-identical. Per-file SHA/mode inventory is
  `.qa/package-inventory.json`; the packaged manifest inventories every member
  except itself. The complete packaged schema-3 manifest also passes validation.
- Extracted binary: Mach-O arm64; protocol-info identifies the canonical ID,
  version 0.1.0, SDK 0.1.1 and exactly both commands. Dynamic dependencies are
  system CoreFoundation, libiconv and libSystem; no host/SDK checkout dependency.
- The final extraction is `.qa/extracted-ouq9kylv`.
The extracted production binary ran the full new pipe and TLS suites with owned
  loopback fixtures, private fake HOME/XDG/temp/data and package-root paths.
  It contains no clone path, fixture route/environment strings or node_modules
  string. No QA/helper/Node/cache/fixture code is packaged.

This ZIP is an ignored local development output, not a published artifact.
`artifacts` remains `{}`; release, native GUI acceptance and external publication
are parent-owned future work.

Icon SHA256:
`038c492799df020c53b8bd0ffb6f84e664e8caae1de571496ba5591087b71d45`.
Provenance: Raycast commit `6734d85992290066fd31a7f5bee3cca3d3545a1a`,
`extensions/json-resume/assets/extension-icon.png`; unmodified.
Embedded schema SHA256:
`8911e912ee487954b10cb59da39265c7e62ef7cba5973706d125448adc853969`.

## Explicit gaps and adaptations

The host pin is `ea100cee3f69711de12485ba061950c5fc65258f`. Its
[generic renderer patch](host-prerequisites.patch) is **unapplied** and required
for safe form editing and unchanged-action responses. The patch is limited to
client form revisions, latest **form** response ownership, and unchanged response
handling. Restricting the response fence to forms preserves the one fresh view
when concurrent duplicate browser/copy actions return view/unchanged in either
order. Actual original/patched renderer and producer functions are executed;
no full host build was performed.

Schema validation uses a real complete embedded-schema evaluator. Finite oracles
do not establish universal engine parity. Rust enforces bounded, finite JSON and
Unicode scalar decoding; overflow/lone surrogate parsing is stricter than
JavaScript. Native URL normalization is tested against Node WHATWG cases, not
claimed universally identical. Credential URLs/non-HTTP schemes are visibly
rejected. Invalid non-string names from the source's unvalidated title-only
lookup are ignored rather than persisted as malformed typed records. Corrupt
storage is surfaced and preserved instead of V1's silent empty fallback. No V1
migration is invented. Save errors retain a valid detail with manual Save recovery.

Missing-grant evidence is precise and unresolved: locked archives for
`alloc-stdlib` 0.3.0 (BSD-3-Clause), `uuid-simd` 0.8.0 and `vsimd` 0.8.0 (MIT),
and `winapi-i686-pc-windows-gnu` / `winapi-x86_64-pc-windows-gnu` 0.4.0
(MIT/Apache-2.0) contain no standalone grant file; an archive-wide scan also
found no standard MIT/BSD grant text. Their actual checksum/license metadata is
recorded. No missing grant was invented and no legal approval is asserted.
The full r-efi 5.3.0 and 6.0.0 AUTHORS files, including actual MIT grants and
copyright attribution, are retained byte for byte.

XERJ canonical prefix is `ax-afca8192d4ca`, state under `.qa/xerj`. Initial indexing
covered 10/10 source files (17/22 files, completed-with-junk). The final bounded
attempt was repeated in Round 2 and failed on frozen dataset/schema evolution for the candidate inventory.
Research and verification therefore use full source/Git/rg/hash fallback; there
was no infrastructure detour, outer indexing, new-prefix workaround or restart.

## Reproduction

All commands run from this exact clone. Existing toolchains only. Python 3.11+
is needed by notice/audit/package scripts; on this machine use
`/opt/homebrew/bin/python3`. Other fixture scripts run with system Python 3.9.

```sh
v2/scripts/owned.sh /opt/homebrew/bin/python3 v2/qa/prepare.py
# Standard exact-lock ci reproduces the baseline peer conflict:
v2/scripts/owned.sh npm ci --prefix v2/.qa/v1 --ignore-scripts --no-audit --no-fund
v2/scripts/owned.sh npm ci --prefix v2/.qa/v1 --ignore-scripts --legacy-peer-deps --no-audit --no-fund
v2/scripts/owned.sh npm test --prefix v2/.qa/v1
v2/scripts/owned.sh npm run typecheck --prefix v2/.qa/v1
v2/scripts/owned.sh npm ci --prefix v2/.qa/host-ui --ignore-scripts --no-audit --no-fund
v2/scripts/owned.sh node v2/qa/oracle.cjs
v2/scripts/owned.sh node v2/qa/host.cjs
v2/scripts/owned.sh node v2/qa/markdown.cjs
v2/scripts/owned.sh cargo +1.85.0 fmt --manifest-path v2/Cargo.toml -- --check
v2/scripts/owned.sh cargo +stable fmt --manifest-path v2/Cargo.toml -- --check
v2/scripts/owned.sh cargo +1.85.0 test --locked --manifest-path v2/Cargo.toml
v2/scripts/owned.sh cargo +stable test --locked --manifest-path v2/Cargo.toml
v2/scripts/owned.sh cargo +1.85.0 clippy --locked --manifest-path v2/Cargo.toml --all-targets -- -D warnings
v2/scripts/owned.sh cargo +stable clippy --locked --manifest-path v2/Cargo.toml --all-targets -- -D warnings
v2/scripts/owned.sh cargo +1.85.0 build --locked --manifest-path v2/Cargo.toml
v2/scripts/owned.sh python3 v2/qa/pipes.py
v2/scripts/owned.sh python3 v2/qa/tls.py
v2/scripts/owned.sh cargo +1.85.0 check --locked --manifest-path v2/Cargo.toml --target x86_64-pc-windows-gnu
v2/scripts/owned.sh cargo +1.85.0 clippy --locked --manifest-path v2/Cargo.toml --target x86_64-pc-windows-gnu --all-targets -- -D warnings
v2/scripts/owned.sh cargo +1.85.0 fetch --locked --manifest-path v2/Cargo.toml
v2/scripts/owned.sh /opt/homebrew/bin/python3 v2/scripts/notices.py
v2/scripts/fresh-build.sh
# Pass the binary printed by fresh-build.sh; never reuse one path for both roots.
v2/scripts/owned.sh /opt/homebrew/bin/python3 v2/scripts/package.py <fresh-binary>
v2/scripts/owned.sh python3 v2/qa/pipes.py <extracted-package/bin/json-resume-v2>
v2/scripts/owned.sh python3 v2/qa/tls.py <extracted-package/bin/json-resume-v2>
v2/scripts/owned.sh curl -q --fail --location --max-time 30 https://raw.githubusercontent.com/raycast/extensions/6734d85992290066fd31a7f5bee3cca3d3545a1a/extensions/json-resume/assets/extension-icon.png -o v2/.qa/upstream-icon-final.png
v2/scripts/owned.sh env JSON_RESUME_TEST_PACKAGE_MANIFEST=<extracted-package/extension.json> cargo +1.85.0 test --locked --manifest-path v2/Cargo.toml --test parity complete_schema3_validation
v2/scripts/owned.sh /opt/homebrew/bin/python3 v2/scripts/verify-artifact.py
v2/scripts/owned.sh /opt/homebrew/bin/python3 v2/scripts/audit.py
```

The icon must first be freshly downloaded to `.qa/upstream-icon-final.png` from
the exact pinned raw GitHub URL before the audit. `prepare.py` reads only pinned
Git objects in the adjacent SDK/host repositories and applies the patch only to
owned fixtures. `CANDIDATE-SHA256.json` inventories all Git-visible README/v2
candidate files except itself. The parent must rehash the eventual **staged**
blobs; this report deliberately leaves the index empty. All targets, node_modules,
cache roots, fake homes, fixtures' mutable outputs and ZIPs are ignored under
`.qa/` and must not be staged.

Parent acceptance review repeated all original oracles, actual host/Markdown
functions, 12 Rust tests on stable/1.85, strict Clippy, V1 baseline and TLS.
A new empty-Cargo-home HTTPS build was packaged and its extracted executable
passed 2,199 assertions / 92 owned HTTP requests (including refresh polling).
The original absent-create-delete reproduction now remains absent across two
processes; unrelated URL writes and deliberate Save recovery both pass.
Parent independently checked the 456-member ZIP and exact original artwork.
The packaged manifest also passed the complete pinned schema validator.
