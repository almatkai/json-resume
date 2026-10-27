# JSON Resume — standalone Rust V2

Two native commands: **Open Resume** and **View Saved Resumes**. The executable
fetches the supplied HTTP(S) URL, validates the embedded locked JSON Resume
schema, renders the original Markdown, and automatically saves a valid URL,
name and fetch date. Save URL, Unsave URL, Re-Fetch, source-browser actions,
saved-list viewing, copying and deletion remain available. Deletes have no extra
confirmation. The original icon is used byte for byte.

This is a development candidate for parent review. It is not published or
installed. `extension.json` is schema 3 / process / protocol 1, replaces both
`com.tezbar.json-resume` and `raycast.json-resume`, and has `artifacts: {}`.
Native GUI verification remains `not-run` on macOS and Windows.

The pinned host needs the generic, **unapplied**
[renderer prerequisite patch](qa/host-prerequisites.patch): client-owned form
revisions, latest-action-response fencing, and support for `unchanged` action
responses. Editing the form on the unpatched pinned host reports a concrete
upgrade message. Actual original and patched functions are exercised by QA.
No host or SDK working files are modified here.

## Build and verify

Use an existing Rust stable and 1.85.0 toolchain, Node/npm, Python 3.11+ and
OpenSSL. No installer is run. All mutable QA, npm, Cargo, HOME, XDG, temp,
fixture, output and build state is under `v2/.qa/`. `scripts/owned.sh` is a child
wrapper; it does not alter the caller's shell. On this machine use
`/opt/homebrew/bin/python3` for scripts requiring `tomllib`.

From the repository root:

```sh
v2/scripts/owned.sh node v2/qa/oracle.cjs
v2/scripts/owned.sh node v2/qa/host.cjs
v2/scripts/owned.sh cargo fmt --manifest-path v2/Cargo.toml -- --check
v2/scripts/owned.sh cargo +1.85.0 test --locked --manifest-path v2/Cargo.toml
v2/scripts/owned.sh cargo +stable test --locked --manifest-path v2/Cargo.toml
v2/scripts/owned.sh cargo +1.85.0 clippy --locked --manifest-path v2/Cargo.toml --all-targets -- -D warnings
v2/scripts/owned.sh cargo +stable clippy --locked --manifest-path v2/Cargo.toml --all-targets -- -D warnings
v2/scripts/owned.sh python3 v2/qa/pipes.py
v2/scripts/owned.sh python3 v2/qa/tls.py
v2/scripts/fresh-build.sh
# After package.py has produced the owned ZIP and extraction:
v2/scripts/owned.sh /opt/homebrew/bin/python3 v2/scripts/verify-artifact.py
```

[QA reproduction and results](qa/REPORT.md) describes initial fixture setup,
exact-lock V1 baseline, full package/byte/notice audits, platform attempts,
fresh Cargo roots, and extracted production executable tests.

## Behavior and ownership

The two original TSX Markdown functions are executed independently as oracles.
No HTML parsing or new content sections are added. Raw Markdown is sent to the
host's existing display/sanitizer. The original formatter reads extra `company`,
`website`, and `gpa` fields; schema `work.name` is not substituted for `company`.
Profiles, empty fallbacks, blank lines, contact separators, dates and first-eight
skill keywords follow the source.

The embedded npm schema is exactly `@jsonresume/schema` 1.2.1; its original
validator is `jsonschema` 1.5.0. Rust uses the genuine `jsonschema` 0.29.1 evaluator
with local refs and compatible URI/email formats. It applies the complete schema,
including optional fields, null/type restrictions and additional properties.
The schema uses a permissive date pattern, so e.g. `2020-19-39` is accepted just
as in the locked library. No defaults are inserted and document `$schema` values
never select or fetch a new schema. Finite fixtures establish tested agreement,
not universal equivalence of JSON/JavaScript/format engines. Rust rejects numeric
overflow and lone-surrogate JSON decoding and enforces work bounds beyond the unbounded source.

HTTP content-type handling follows the executed locked useFetch parser: JSON MIME
responses are decoded as JSON; other bodies are text with the source's subsequent
JSON/string coercion. HTTP errors and malformed JSON are visible. Redirects,
UTF-8 replacement/BOM handling, JSON string bodies, gzip/deflate/brotli, and URL
normalization are native Rust. URL parsing follows Rust `url`'s WHATWG behavior;
finite normalization fixtures do not assert complete Node URL equivalence.
Credential-bearing URLs and non-HTTP(S) schemes are rejected visibly.

Valid open-command loads automatically save the **original trimmed source URL**,
not a redirected or normalized URL. Existing URL updates retain their position
and keep the old name when the new name is empty; new URLs prepend. An explicit
Save with no name still makes the source's extra title-only JSON fetch and ignores
its fetch failure. Viewing an existing saved resume does not silently re-save it.
Title fallback, URL subtitle, locale date and globe accessories follow the source;
the native host formats the ISO date accessory with its locale.

The SDK has no launchCommand. The empty-list Open Resume action navigates to the
same fully functional form inside the session. Back/Edit/View Saved Resumes actions
make that navigation reachable. Saved-list searching uses native client filtering.
`src/tools/text-to-resume.tsx` is an unregistered non-command AI helper: package.json
has only these two commands and no tools registration. It is **not implemented**
or advertised in V2; no AI backend or credential provisioning is added.

Save/Clear prefer the last valid URL, then the last submitted input URL; an
unsubmitted editable draft never changes that source target. Each session has one owned fetch/save job, cancellable by newer client form
revisions, navigation, dispose, EOF or shutdown. A/B/A and invalid intermediate
drafts cannot revive an old response. Fetch, validation and autosave share the
same frozen source URL and revision. Just before commit the task checks ownership;
the small synchronous atomic replacement has a bounded non-preemptible commit
window. There is no rollback after a committed write or exactly-once/crash-durability
claim. Storage conflicts and failed saves stay visible with explicit recovery.

Copy/browser IDs are consumed outside SDK staged state before awaiting the host.
A duplicate, stale or foreign ID is inert. Only closed `{ok:true,message?:string}`
ACKs confirm success; uncertain results expose a frozen manual Retry action.
A new deliberate action after confirmed success gets a fresh ID. Refresh cannot
repeat an effect. No paste, hide-window or unrelated OS actions are added.

## Bounds and private persistence

- URL: 8 KiB; body: 1 MiB **after decompression**; JSON depth: 64; values: 20,000.
- HTTP idle connection pooling is disabled. HTTP connect: 5 seconds; entire fetch/body: 15 seconds; redirects: 5; no proxy,
  cookies, arbitrary remote schema retrieval, automatic retries or body cache.
- Saved records: 256; title: 16 KiB; private store: 1 MiB. Reads and writes are
  serialized with an advisory file lock and atomic tempfile replacement.
- Store: host-provided `dataDir/saved-resumes/storage.json`; directory/file modes
  0700/0600 on Unix. Windows relies on the host's private directory ACL; native
  ACL acceptance has not run. Corrupt/unknown/oversized storage is rejected and
  its bytes retained. This intentionally replaces V1's silent empty fallback.
- Storage format 2 explicitly separates present record identity (UUID/stamp) from
  absent URL generation. Every successful save/delete retains that URL's latest
  checked revision under a SHA-256 key, including after deletion. An asynchronous
  load or title-only Save must match its original per-URL observation under the
  transaction lock. This prevents absent/create/delete ABA resurrection, stale
  deletion, reused-URL deletion and late updates across real processes; unrelated
  URL writes remain independent. A new deliberate Save takes a fresh observation. Save failures expose that
  action even when the session still has a stale saved-list snapshot; the existing
  guarded Unsave action cannot delete a replacement record.
- History: at most 4,096 distinct URL hashes, retained without eviction. Reaching
  this limit visibly rejects new distinct URLs and preserves the previous bytes;
  known URLs can still be saved/deleted within the other limits. Deleted full URLs
  and titles are not retained. Unsalted hashes are fingerprints, not encryption.
  Invalid keys/generations, missing history and record/history mismatches are
  rejected. Prior candidate format 1 and unknown versions report an explicit
  unsupported-format error and retain their bytes; no migration/reset is implied.
  Lock contention is a visible manual-retry error. Failed persistence restores
  the transaction's in-memory state as well as preserving prior disk bytes before
  atomic replacement; no directory-fsync/crash-durability guarantee is claimed.
- No V1 migration, real profile reads, persistent resume-body cache, URL/body/
  credential logging, or unrelated data persistence.
- Host effect wait: 8 seconds; SDK protocol limits: 16 MiB/frame, 128 sessions,
  120 ordinary + 8 cleanup handlers, 128 pending reverse requests, 256 output
  frames and 64 tasks per scope. The port owns one cancellable job per session;
  completed scope tasks are reclaimed by the SDK. A scope lease handles canceled
  opens even when explicit dispose never arrives.

The standalone crate pins SDK 0.1.1 over immutable HTTPS Git revision
`e955911f0bf5b558f9d3eb5183df0adc95c22af2`. No local SDK dependency, JS engine,
JavaScript subprocess, host build or sibling runtime dependency is present.
Production targets are Apple Silicon macOS and Windows x64 MSVC, minimum Rust
1.85. Platform declarations describe intended support; native GUI/MSVC acceptance
and production release artifacts are still pending.

Authentic notices and precise missing-grant evidence are in [NOTICE](NOTICE) and
[dependency-notices.json](resources/dependency-notices.json). No legal approval is
claimed. The original V1 source, manifest, lock, assets and license remain intact.
