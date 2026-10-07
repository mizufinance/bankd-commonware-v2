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
Package downloads use Ubuntu's official HTTPS archive instead of the Azure mirror,
which repeatedly stalled in setup for thirty minutes. Downloads have network
timeouts and retries, plus overall limits for index refresh and installation.
Rust builds use two compiler workers and omit debug information to fit standard
runners, with a longer timeout for cold builds. Clippy and documentation use the
same pinned nightly compiler; formatting still uses `cargo +nightly fmt`.
End-to-end runners get 4 GiB of additional swap: the ten-validator network
fixtures use about 8.3 GiB by themselves in a local measurement, and repeatedly
coincided with hosted-runner shutdowns. Those tests remain enabled.

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
Snapshot-restart tests run only in the existing advisory job. Its failures remain
visible without blocking the PR. The inherited `on-timeout = "pass"` override is
removed: a timeout now fails that test instead of claiming success.
The advisory profile sets its nine-minute limit explicitly because the pinned
nextest version does not inherit per-test overrides from intermediate profiles
([fixed upstream in 0.9.145](https://nexte.st/changelog/#09145---2026-09-16)).

The checks also exposed runtime bugs that are fixed, not excluded: Shieldd
shutdown now releases background database references before reopening, and
consensus finalization schedules blocking work on the execution node's runtime
instead of assuming the caller runs inside Tokio. Restart and end-to-end tests
remain enabled. The stale-DKG snapshot fixture now clears archive metadata along
with its data partitions before installing a replacement snapshot. End-to-end
shutdown also waits for background tasks to release their Shieldd handles before
reopening the same database. Failure output is retained in CI logs instead of
suppressed.
The sparse-trie test checks Reth's synchronous fallback on runners with fewer
than five CPU threads, and checks the shared-trie path on larger hosts.
Transaction prewarming keeps its EVM cache separate from Reth's shared worker
state and resets it between payload builds. The worker-state regression waits
for cleanup before asserting that other users' state remains intact.

The generated test genesis is refreshed to include the 100 native BRL account
allocations and five Bankd module predeploys already emitted by the generator.
The generator and existing allocations are unchanged. The gas-fee integration
tests now check native BRL charges, refunds, and escrow credits on successful and
failed transactions, with TIP-20 balances unchanged. Token creation and payment
lane tests likewise expect the funded native balances.

Thirteen inherited node integration cases are explicitly ignored because they require
the removed TIP-20 gas settlement path: `test_set_user_token`,
`test_fee_token_tx`, `test_transact_different_fee_tokens`, both
`test_transact_two_hop_fee_route` variants, and
`test_cant_burn_required_liquidity`. Their source and ignore reasons remain next
to the tests. Direct AMM contract tests and liquidity mint/burn tests still run.
Fee sponsorship and payload fee scoring are adapted to native BRL and remain
required, as do successful and failed transaction fee tests. The other retired
cases are:

| Cases | Obsolete expectation |
| --- | --- |
| `pool::test_evict_txs_on_transfer_policy_change`, `tempo_transaction::local::test_aa_keychain_spending_limit_toctou_dos` | Pool eviction based on a TIP-20 gas token's policy or spending limit. Bankd validates native gas payment; TIP-20 transfer restrictions still execute in the token contract, and native Compliance checks still run. |
| `storage_credits::test_tip1060_keychain_fee_refund_does_not_retain_storage_credit`, `storage_credits::test_tip1060_successful_fee_token_spend_fee_refund_cancels_restored_balance_credit` | Gas refunds restore TIP-20 balance/spending-limit storage slots. Native gas refunds do not touch those slots. |
| `storage_credits::test_tip1060_rebalance_swap_does_not_mint_stale_fee_manager_custody_credit`, `storage_credits::test_tip1060_fee_manager_credit_from_distribute_fees_is_not_redeemable`, `storage_credits::test_tip1060_distribute_fees_receive_policy_guard_creations_are_accounted` | Gas accumulates TIP-20 FeeManager custody and validator distribution balances. Bankd credits native escrow instead. |

Fresh helper wallets are funded with native BRL. Gas snapshots are updated for
Bankd's fee path, which no longer warms TIP-20 storage during gas collection;
the gas matrices and their behavioral assertions remain enabled. The paused-token
test now verifies that pausing blocks token transfers while native gas remains usable.
Access-key matrix fixtures authorize native gas separately from TIP-20 transfers:
spending the exact token limit now succeeds, while over-limit token transfers and
keys without a gas allowance remain rejected or reverted as appropriate.
The four 85-case transfer matrices get ten minutes to complete on standard
runners, including their additional native-funding transactions.

The experimental `--builder.parallel` mode is disabled at the payload builder's
constructor, including requests through the Rust API. Its legacy storage-action
replay cannot account for native BRL balance changes or Compliance reads. Such
requests log a warning and use ordinary EVM execution; transaction prewarming
remains enabled. One related test, `test_tip20_full_evm_storage_actions`, is
explicitly ignored because its complete-state replay assumption no longer holds.
The implementation and test are preserved for a future Bankd-aware replay design.
A native-fee integration case also runs with a parallel request to check the safe
fallback. Together with the thirteen obsolete node cases above, this PR adds
fourteen explicit ignored cases; existing ignored tests are unchanged.

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
- End-to-end partitions compile the `tempo-e2e` package directly, avoiding
  redundant builds of unrelated workspace test binaries. Known snapshot-restart
  cases run in the separate advisory job instead of also running in required
  partitions. Hardfork helper tests read the preserved benchmark workflow from
  its new location.

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
