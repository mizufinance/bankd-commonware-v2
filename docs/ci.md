# Bankd CI

CI builds and tests each pull request using GitHub-hosted Ubuntu runners. The
inherited Tempo setup requested Depot runners and authenticated to Tempo's
StepSecurity service. In this repository, Rust jobs remained queued and jobs
that reached setup failed with `Step Security STS exchange failed (HTTP 403)`.
Those failures happened before the checks could evaluate Bankd's code.

The active workflows use pinned public actions, initialize submodules (including
the `shieldd` path dependencies), and keep checkout credentials out of the worktree.
Public StepSecurity runner hardening remains enabled with egress auditing. Native
build dependencies and Cargo caching are shared through a local setup action.
Rust builds use two compiler workers and omit debug information to fit standard
runners, with a longer timeout for cold builds.

## Checks retained

| Workflow | Checks |
| --- | --- |
| `lint.yml` | Clippy with warnings denied, workspace formatting, Rust documentation, spelling, `cargo deny` advisories/licenses/sources/bans, feature propagation, and `no_std` compilation. Feature combinations also run on pushes to `main`. |
| `test.yml` | Unit/integration tests in two partitions, node end-to-end tests in four partitions, the existing advisory flaky-test job, CLI smoke tests, minimum Rust version, and generated genesis consistency. |
| `specs.yml` | Solidity build, Solidity formatting, and Rust/Solidity ABI alignment when relevant paths change. |
| `scan-github-actions.yml` | Actionlint, zizmor, and CI helper tests. |
| `build.yml` | Manually requested binary builds uploaded as GitHub artifacts. |

Summary jobs fail on failed, cancelled, or unexpectedly skipped prerequisites.
Only jobs explicitly excluded by their event or path conditions may be skipped.
The existing advisory status of the flaky-test job is unchanged.

Formatting covers the Bankd workspace, rather than recursively enforcing Bankd's
style on `shieldd`'s separate workspace. Spelling excludes submodules and vendored
code, and recognizes the project name Shieldd and the cryptographer's name Groth.

The dependency policy explicitly allows the three existing Mizu crypto forks
(`decaf377`, `decaf377-rdsa`, `poseidon377`). Two unmaintained transitive procedural
macros have narrow advisory exceptions: `derivative` (RUSTSEC-2024-0388, Arkworks
0.5) and `proc-macro-error` (RUSTSEC-2024-0370, genawaiter in the shield wallet).
Neither has a compatible maintained release in this dependency graph.

RUSTSEC-2025-0055 is excepted only because Arkworks' `tracing-subscriber` 0.2.25
has no features enabled, so its vulnerable log formatting module is absent.
A separate required metadata check fails if any subscriber older than 0.3.20
enables `fmt`. The application's subscriber is patched at 0.3.23. Other
vulnerability, license, source, and banned-crate checks remain enforced.

## Disabled workflows

All 33 files below are preserved unchanged in
[`.github/disabled-workflows`](../.github/disabled-workflows). Moving them out of the active workflow directory
disables automatic events, schedules, and manual dispatches after this change is
merged. No application feature, Rust test, benchmark implementation, or release
script is deleted by these moves.

| Workflows | Why disabled / what is lost |
| --- | --- |
| `bench.yml`, `bench-e2e.yml`, `bench-e2e-scheduled.yml`, `bench-e2e-multi-region.yml`, `bench-e2e-multi-region-scheduled.yml`, `bench-replay.yml`, `bench-replay-scheduled.yml` | Tempo's comment-triggered and scheduled benchmarking relies on its bare-metal/Depot runners, cloud deployments, telemetry, Slack credentials, and benchmark repositories. Automated performance reports stop; Rust benchmark targets remain. |
| `codspeed-microbench.yml` | The inherited CodSpeed reporting setup and Depot runner have not been configured for Bankd. Automated regression measurements are disabled; benchmark sources remain. |
| `docker.yml`, `docker-profiling.yml`, `docker-pr.yml`, `docker-pr-status.yml`, `build-devnet.yml`, `build-reth-bump-image.yml`, `promote-canary.yml` | These build, publish, promote, or report on Tempo images/devnet using Depot, Tempo registry destinations, or internal deployment hooks. They do not constitute a configured Bankd image release pipeline. |
| `prepare-release.yml`, `release-pr.yml`, `release.yml`, `publish.yml`, `publish-check.yml`, `semver-check.yml` | These implement Tempo's crate/release process, including trusted publishing, its existing crate versions, release branches, and binary destinations. Automatic publishing and compatibility checks against upstream published crates are disabled. |
| `reproducible-build.yml` | The inherited artifact/hash pipeline needs its Depot runner and an adapted Docker recipe for Bankd's submodules. Reproducibility hashes are no longer generated automatically; the recipe and script remain available for adaptation. |
| `deploy-docs.yml` | Calls Tempo's Vercel deployment hook. Rust documentation still builds in lint CI; deployment must be configured for Bankd separately. |
| `pr-audit.yml` | Requires Tempo's Cyclops/Derek audit service, audit labels, and event credentials. That service cannot approve Bankd PRs in the inherited configuration. |
| `dependency-scan.yml` | Uses Tempo's authenticated dependency scanner, which fails during StepSecurity setup. Its independent scanner is disabled; the portable `cargo deny` checks remain required. |
| `rpc-tests.yml` | Runs against Tempo's Moderato/devnet endpoints, not the Bankd node built by the PR. Local node and end-to-end tests remain required. |
| `coverage.yml` | Combines artifacts from Tempo's patched-Foundry pipeline and Rust tests. That Foundry pipeline was already restricted to `tempoxyz/tempo`; its aggregate coverage report cannot be completed here. |
| `add-hardfork.yml`, `update-reth.yml`, `sync-from-upstream.yml` | Tempo-specific cross-repository hardfork edits, AI-assisted dependency updates, and release-branch synchronization require its repositories, tokens, or event services. Bankd dependency and protocol changes remain manual PRs. |
| `changelog.yml` | Uses an Amp API key to generate upstream crate changelogs. No configured Bankd AI changelog service is assumed. |
| `label-pr.yml`, `stale.yml` | Upstream issue-label copying and automatic stale-PR closure are repository housekeeping, not code validation. Bankd PR labels and closure remain manual. |

## Removed steps inside retained workflows

- Tempo's `secure-runner` authentication and vendored action wrappers are replaced
  with public pinned actions. GitHub caching replaces the inherited sccache setup;
  Tempo's OIDC permissions and obsolete self-hosted runner labels are removed.
- GitHub Pages deployment is removed from `lint.yml`. Documentation compilation
  remains a required check and now runs in this repository.
- `tempoup` installer checks are removed from `test.yml`; that installer targets
  Tempo's release distribution. The Bankd node's CLI smoke tests remain required.
- Coverage instrumentation/uploads and their change-detection job are removed
  from `test.yml`. The same unit and integration test filters still run.
- Patched-Foundry compilation/execution, the Foundry dependency-resolver smoke
  check, and cross-workflow coverage aggregation are removed from `specs.yml`.
  Patched-Foundry execution was already disabled in this fork by its upstream
  repository guard. Solidity build/format/ABI checks and native Rust precompile
  tests remain; this CI no longer checks compatibility of the patched Foundry
  dependency graph or produces its coverage report.
- Live Tempo RPC secret variables are removed from the normal test workflow.
  Its existing filter already excludes the live testnet/devnet matrix tests.
- The Python flaky-test reporter runs with the runner's Python, so installing a
  separate uv runtime solely for those standard-library scripts is unnecessary.

## Restoring automation

Before restoring a file, configure Bankd-owned runners, credentials, destinations,
and repository permissions for that workflow. Update all Tempo-specific values,
restore any referenced reusable workflows, and verify a manual run. Do not restore
the whole directory just to enable one job.

The `pull_request_target`, comment, and scheduled workflows use the default
branch. A PR cannot remove those default-branch triggers until it is merged;
disabling a workflow through GitHub Actions can stop it immediately when needed.

For local validation, initialize `shieldd` and run `cargo +nightly fmt --check`,
`actionlint`, `zizmor .github/workflows .github/actions`, and
`python3 -m unittest discover -s .github/scripts -p 'test_*.py'`. Rust compilation
and integration checks use the commands in the retained workflows.
