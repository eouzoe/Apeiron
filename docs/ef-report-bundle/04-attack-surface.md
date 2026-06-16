# Attack Surface

## Methodology

The attack surface is classified into three tiers:

1. **Discovered and resolved**: CVE/issue with confirmed patch
2. **Discovered but unresolved**: Known crash path with no containment fix
3. **Undiscovered (inferred)**: Projected from architecture and trends

## Reth

| Tier | Surface | Location | Evidence | Status |
|------|---------|----------|----------|--------|
| Fix identified | ExEx crash → node death | `exex.rs:124` | ✅ source trace | Fix: spawn_task + error! (local; upstream paradigmxyz/reth HEAD still vulnerable) |
| Unresolved | Txpool maintenance | `pool.rs:281` (spawn_critical) | ✅ source (`crates/node/builder/src/components/pool.rs:281`) | Still critical — chain events trigger it |
| Unresolved | Tx-batcher | `core.rs:338` (spawn_critical) | ✅ source (`crates/rpc/rpc/src/eth/core.rs:338`) | Still critical — RPC triggered |
| Inferred | Future ExEx `?` paths | Any `impl ExecutionProvider` | 🔶 General Rust pattern | New ExExes add new surfaces |
| Inferred | Manager WAL error | WAL commit/finalize | 🔶 Disk I/O not isolated | Low probability, catastrophic |

## Geth

| Tier | Surface | Evidence | Status |
|------|---------|----------|--------|
| Resolved | CVE-2020-26242 (MULMOD crash), CVE-2021-39137 (chain split), + 8 others | ⚠️ Public CVEs | Patched per-CVE |
| Unresolved | No recover() outside RPC | ✅ source: sync/txpool/P2P handlers have no recover | Core execution paths: 0 recover |
| Unresolved | State trie nil deref | ⚠️ #19286, #18977 | Recurring pattern |
| Inferred | New EIP opcodes | 🔶 Every hardfork adds 5-15 new opcodes | Next: Pectra, Osaka |
| Inferred | Verkle tree conversion | 🔶 Major state structure change | 2027+ |

## Nethermind

| Tier | Surface | Evidence | Status |
|------|---------|----------|--------|
| Resolved | 2024 revert overflow (chain split), 2026 blob tx (liveness) + 5 others | ⚠️ Public incidents | Patched per-instance |
| Unresolved | Plugin throws in core hooks | ⚠️ Official "by design" stance | No isolation |
| Unresolved | BlockchainProcessor retry loop | ⚠️ Reported stuck nodes | Incomplete fix |
| Inferred | Plugin event handler crash | 🔶 Same pattern as ExEx | Grows with L2 plugin ecosystem |
| Inferred | .NET runtime AccessViolation | 🔶 dotnet/runtime#107503 unpatched | Platform-level risk |

## Besu

| Tier | Surface | Evidence | Status |
|------|---------|----------|--------|
| Resolved | 2024 mainnet halt, CVE-2022-36025, CVE-2021-41272, CVE-2025-30147 + 2 others | ⚠️ Public incidents | Patched per-instance |
| Unresolved | Plugin RuntimeException in block creation | ⚠️ PR #9208 (log + rollback, fragile) | Partial fix |
| Unresolved | Lifecycle halt on plugin error (post-24.10.0) | ⚠️ PR #7662 | Regression: halt on error |
| Inferred | BFT (IBFT/QBFT) implementation | 🔶 Complex state machine | Ongoing risk |
| Inferred | Bonsai trie edge cases | 🔶 2024 halt showed fundamental issues | Next Bonsai bug |

## Erigon

| Tier | Surface | Evidence | Status |
|------|---------|----------|--------|
| Resolved | Immunefi #38766 (nil To BlobTx), #14581 (decompressor), #18234 (eth_getLogs) + 2 others | ⚠️ Public issues | Patched per-instance |
| Unresolved | No recover() in any stage | ✅ source: stagedsync pipeline unprotected | Pipeline: zero recover |
| Unresolved | All-in-one mode (default) | ⚠️ Erigon docs confirm "Embedded" default: RPC server hosted within erigon process | No isolation by default |
| Inferred | Stage state inconsistency | 🔶 16+ sequential stages | Edge cases in ordering |
| Inferred | Custom EVM (erigon-lib/evm) | 🔶 Non-geth EVM, less audited | Independent bug surface |

## Per-Client Incident Breakdown

The "30+ confirmed" figure breaks down as follows:

| Client | Known crash paths | Evidence |
|--------|------------------|----------|
| Geth | 10+ (CVEs 2020-2025, issues #19286, #18977, #27050) | ⚠️ Multiple public security advisories |
| Nethermind | 6+ (Jan 2024 chain split, 2026 blob bypass, OOM, plugin crashes x3) | ⚠️ Incidents, Immunefi bounty |
| Besu | 6+ (2024 mainnet halt, CVE-2022-36025, CVE-2021-41272, CVE-2025-30147, +2) | ⚠️ CVE tracked |
| Erigon | 5+ (Immunefi #38766, #14581, #18234, +2) | ⚠️ Public issues, Immunefi |
| Reth | 3+ (ExEx, txpool, tx-batcher — all spawn_critical) | ✅ Source verified |

**Total: ~30+ confirmed, 0 with runtime containment.** Each row: individual bug patched per-instance.

## Attack Surface Growth Factors

### Factor 1: New crash paths from each hardfork

Each Ethereum hardfork adds 5-15 new opcodes/precompiles. Each is implemented independently in 5 EL clients. Data: Prague EIPs add system contract interactions (new paths in `state_processor.go:344`-type code). Historical pattern: CVE-2021-39137 (EIP-2929 datacopy, chain split at block 13107518), CVE-2022-29177 (P2P crash), Besu 2024 halt (SELFDESTRUCT removal).

### Factor 2: Plugin/extension ecosystem growth

Reth ExEx: 1 implementation → 10+ in ~12 months. Each new ExEx adds `?` operators on chain data. Evidence: Rollup ExEx uses `abi_decode(tx.input())?` at line 95, Rust pattern generates Err from malformed ABI data.

### Factor 3: No crash attribution across any client

| Client | Crash diagnostics available |
|--------|---------------------------|
| Geth | stack trace → process exit. No structured crash metadata |
| Reth | TaskManager log (task ID, panic message) |
| Nethermind | BlockchainProcessor: catch only logs. Zombie: no signal at all |
| Besu | UncaughtExceptionHandler: logs only. Thread death: no alert |
| Erigon | Goroutine death: silent (no recover, no log) |

Without crash attribution data, operators cannot distinguish attack crashes from bug crashes. History: Besu 2024 halt took hours to diagnose as not-a-bug.
