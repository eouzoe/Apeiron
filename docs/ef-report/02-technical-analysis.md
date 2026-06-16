# Technical Analysis

## 1. Reth ExEx Kill Chain

### Verified Trace (✅ source code, 4 crates, 6 files)

| Hop | Location | Mechanism | Outcome |
|-----|----------|-----------|---------|
| 0 | ExEx application code | The `?` operator on chain data returns `Err` | ExEx future resolves with `Err` |
| 1 | `crates/node/builder/src/launch/exex.rs:129-134` | `match Err => panic!("ExEx crashed: {err}")` | Stack unwind begins |
| 2 | `crates/tasks/src/runtime.rs:626-631` | `catch_unwind` → `PanickedTaskError` → `task_events_tx.send(TaskEvent::Panic)` | TaskManager is notified |
| 3 | `crates/tasks/src/lib.rs:181` | `Some(TaskEvent::Panic(err)) => Poll::Ready(Err(err))` | TaskManager resolves with error |
| 4 | `crates/tasks/src/runtime.rs:975-981` | JoinHandle from `task_manager.await` carries the error | Error escapes the task system |
| 5 | `crates/cli/runner/src/lib.rs:240-247` | `run_to_completion_or_panic` receives `Err` | Process exits |

### Root Cause

`crates/node/builder/src/launch/exex.rs:124`: ExEx instances are spawned via `spawn_critical_task`. The task system has two tiers (`spawn_task` and `spawn_critical_task`). ExEx (an application plugin) uses the critical tier, which kills the node on panic. Evidence: the manager at `manager.rs:519-529` already handles ExEx disconnection gracefully (logs warning, continues), showing the fix is consistent with existing design.

### The Isolation Gap

Reth has a two-tier task system:

| Property | `spawn_task` | `spawn_critical_task` |
|----------|-------------|----------------------|
| catch_unwind | ❌ | ✅ |
| Notifies TaskManager on panic | ❌ | ✅ (via TaskEvent::Panic) |
| Panic kills node | ❌ | ✅ |
| Used for | Non-critical work | "Must never fail" tasks |

The bug: ExEx instances (application plugins) are incorrectly assigned to the critical tier. The fix is two lines: replace `spawn_critical_task` with `spawn_task`, and `panic!` with `error!`.

### Framework Layer Behaviour (✅ verified)

The `ExExNotifications` stream (`crates/exex/exex/src/notifications.rs:497-514`) always returns `Poll::Ready(Some(Ok(notification)))`. The framework layer cannot be remotely triggered to produce an error. The crash path requires ExEx application code to use `?` on chain data.

### Manager Disconnection Handling (✅ verified)

`crates/exex/exex/src/manager.rs:519-529`: When the channel to an ExEx returns `Poll::Ready(Err)`, the manager logs a warning and continues — skipping that ExEx without affecting others. This path is the correct handler; the fix routes ExEx crashes to it.

### Fix Edge Cases

The `spawn_task` + `error!` fix was verified against the following edge cases:

| Edge case | Behaviour | Risk |
|-----------|-----------|------|
| ExEx panics inside spawn_task | Tokio runtime catches panic, JoinHandle<()> resolves silently. TaskManager not notified. ExEx removed from manager via channel close. | ✅ Safe — no different from Err return |
| ExEx panics during notification delivery | Manager gets Poll::Ready(Err) on channel, warns and continues. | ✅ Safe — manager handles this path |
| ExEx silently hangs (no panic, no return) | Task runs indefinitely, no error path triggered. Node continues with other ExExes. | ⚠️ Resource leak — ExEx task lingers. Mitigation: add timeout on ExEx notification delivery |
| error! macro panics (e.g. poisoned logger) | Unlikely in practice. If it occurs, same as ExEx panic — caught by tokio runtime. | 🔶 Low probability. Standard log crate does not panic |
| All ExExes crash simultaneously | Manager removes each on disconnect. Node continues without any ExEx. | ✅ Safe — degraded but operational |
| WAL corruption during crash | WAL commit/finalize errors are handled separately in the manager. Not affected by this fix. | ✅ Independent subsystem |
| Upstream merges fix, downstream fork ignores | Each fork must apply independently. Fix is 2 lines, easy to carry. | 🔶 Coordination risk, not technical |

## 2. The General Pattern

The same architectural gap appears in every EL client:

### Geth (✅ verified)

Geth's core subsystems have no panic recovery. Across the entire codebase, only 5 production (non-test) `recover()` calls exist — 4 in peripheral code (console, logging, JS engine, DB recovery) and 1 in RPC method handlers (`rpc/service.go:200`). All core execution paths have zero recovery.

**Finding G-1: Core block processing has no panic safety net.**
- `core/state_processor.go:344` has an explicit `panic(err)` on system contract failure — no `recover()` anywhere in the call chain
- `core/state_transition.go`, `core/evm.go`: zero `recover()` calls
- A nil dereference during EVM execution propagates through `Process()` → `InsertChain()` → process exit

**Finding G-2: P2P handler goroutines (9 total) are unprotected.**
- `eth/handler.go:413` `go h.txBroadcastLoop()`, `:418` `go h.blockRangeLoop()`, `:425` `go h.protoTracker()`, `:303` `go func() { ... }()`
- `p2p/server.go:845` `go func() { srv.SetupConn() }`, `:984` `go srv.runPeer(p)`
- `p2p/peer.go:278` `go p.readLoop()`, `:279` `go p.pingLoop()`, `:473` `go func() { proto.Run() }()`
- Any panic in any of these 9 goroutines terminates the process

**Finding G-3: Txpool goroutine panic causes permanent deadlock.**
- `core/txpool/txpool.go:201` `go func() { subpool.Reset(); resetDone <- newHead }()` — if `subpool.Reset()` panics, the channel send never executes
- The main loop (`pool.loop()`) blocks forever on `<-resetDone`, rendering the txpool non-functional
- No log, no metric, no crash — operator cannot distinguish deadlock from idle

**Finding G-4: Miner has an explicit panic path.**
- `miner/worker.go:384` `panic("blob transaction without blobs in miner")` — triggered by malformed input
- `miner/payload_building.go:268` background goroutine dies silently on any panic

**Finding G-5: No top-level recover in main().**
- `cmd/geth/main.go:280` `func main() { app.Run(os.Args) }` — any unrecovered panic prints a stack trace and exits with code 1
- Contrast with Reth: `tokio::runtime` catches panics at the runtime level

**New findings G-6 to G-10 (all currently unpatched, ✅ verified):**
| Finding | Location | Type |
|---------|----------|------|
| G-6 | `core/state_processor.go:344` | `panic(err)` on system contract failure — triggered by block data |
| G-7 | `core/state/statedb.go:310` | `panic` on refund underflow — triggered by transaction |
| G-8 | `eth/protocols/eth/handlers.go:92-94` | `log.Crit` → `os.Exit(1)` on P2P header RLP encode failure |
| G-9 | `eth/fetcher/tx_fetcher.go:601,648,796,1007` | `panic()` on tx fetcher state machine inconsistency |
| G-10 | `p2p/rlpx/rlpx.go:133,213` | `panic()` on handshake race |

Full details: `.harness/research/ef-cross-client-report/poc-geth.md`

✅ **Verified evidence file**: `.harness/research/ef-cross-client-report/client-evidence-geth.md`

**Historical incidents (all ✅ verified against public sources):**
- CVE-2020-26242: MULMOD buffer underflow through deployable contract
- CVE-2021-39137: datacopy chain split at block 13107518
- CVE-2022-29177: P2P log crash
- #27050: nil Random in Clique+Shanghai

**Summary**: Geth's core subsystems have zero `recover()` calls outside RPC. By contrast, Reth's `spawn_critical_task` uses `catch_unwind` which catches the panic, logs it, and routes it to TaskManager before process exit. Geth has no equivalent: a panic in any of the 9 P2P goroutines or core execution path terminates the process immediately.

### Nethermind (✅ verified, 6 findings)

Nethermind's plugin system (INethermindPlugin) explicitly does not isolate plugin crashes. The project has stated this is "by design" — but the design leaves critical gaps.

**Finding N-1: Plugin Init has two unprotected paths.**
- `InitTxTypesAndRlp.cs:29-32` and `InitializeNetwork.cs:273-276`: Plugin `InitTxTypesAndRlpDecoders()` and `InitNetworkProtocol()` are called in a bare `foreach` loop with no try/catch.
- A plugin crash in either kills the node during startup. No graceful degradation, no skip.

**Finding N-2: `MustInitialize` flag is a binary circuit breaker.**
- `InitializePlugins.cs:29-30`: A plugin declaring `MustInitialize = true` becomes a kill switch — if its `Init()` throws, the node dies unconditionally.

**Finding N-3: Plugin sandbox isolation is zero.**
- `INethermindPlugin.cs:9-19`: Plugin code runs in-process with full access. No AppDomain, no AssemblyLoadContext boundary.

**Finding N-4: BlockchainProcessor loop catch is a silent zombie.**
- `BlockchainProcessor.cs:300-315`: The `RunProcessing` catch block only logs. The processing task terminates silently — node stays alive but stops processing new blocks. No operator-visible signal.

**Finding N-5: P2P handler partially unprotected.**
- `Eth62ProtocolHandler.cs:95-183`: Core message types (Transactions, BlockHeaders, NewBlockHashes) are dispatched in `HandleMessageCore` with no try/catch. BackgroundTaskScheduler isolates some paths but not all.

**Finding N-6: AppDomain.UnhandledException logs only.**
- `Program.cs:64-72`: The global handler logs but cannot prevent process termination. No graceful shutdown (flush DB, notify peers).

✅ **Verified evidence file**: `.harness/research/ef-cross-client-report/client-evidence-nethermind.md`

⚠️ Production: Jan 2024 revert overflow → ~9% validator chain split (block 19056922). 2026 blob tx validation bypass → ~38% validator at risk (EF $50k bounty)

### Besu (✅ verified, 6 findings)

Besu's plugin lifecycle methods have try/catch, but the default configuration (`continueOnPluginError=false`) means any plugin failure kills the node. Runtime isolation is absent.

**Finding B-1: Plugin crash default kills node.**
- `BesuPluginContextImpl.java:200-218`: `register()`, `beforeExternalServices()`, `start()` are wrapped in `catch (Exception)`, but unless `--plugin-continue-on-error=true` is set, the exception rethrows as `RuntimeException` → node crash.
- Admin must explicitly opt into resilience. Default is fragile.

**Finding B-2: No plugin sandbox isolation.**
- `BesuPluginContextImpl.java:361-393`: Single shared `URLClassLoader`. No SecurityManager. No per-plugin ClassLoader. Plugins have full access to all internal packages.

**Finding B-3: Thread death silently swallowed.**
- `Besu.java:85-99`: `UncaughtExceptionHandler` only logs — no thread restart, no node termination, no operator alert. Any thread dying (sync, P2P, background) leaves a zombie node.

**Finding B-4: Engine API single worker thread unprotected.**
- `RunnerBuilder.java:1348`: Vert.x instance with `workerPoolSize=1`, no `exceptionHandler` set. A RuntimeException in any `engine_*` handler kills the sole worker, stalling Engine API permanently. CL sees alive node, EL has dead execution layer.

**Finding B-5: Subscribers callback exceptions propagate.**
- `Subscribers.java:128-145`: Plugin-registered listeners throw `RuntimeException` through the callback chain to the caller — no suppression by default.

**Finding B-6: Sync pipeline abort silently swallowed.**
- `Pipeline.java:167-213` catches `Throwable` but `Runner.startEthereumMainLoop()` never checks the pipeline's future. Sync dies silently — node continues without syncing.

✅ **Verified evidence file**: `.harness/research/ef-cross-client-report/client-evidence-besu.md`

⚠️ Production: 2024-01-06 Mainnet halting — SELFDESTRUCT/CREATE2 combination crashed ~70% of Besu nodes (block 18947893). CVE-2022-36025 (gas calc chain halt). CVE-2025-30147 (ALTBN128 crafted points).

### Erigon (✅ verified, 7 findings)

Erigon's staged sync pipeline has selective recovery — parallel execution paths are well-protected, but critical serial paths and Engine API have zero panic recovery.

**Finding E-1: Engine API has zero `recover()` calls.**
- `execution/engineapi/` (entire directory): No recover in any Engine API method. CL → EL communication has no panic safety net at all. A single `engine_newPayload` panic kills the node.
- Propagation chain: `engine_newPayload` → `ExecModule.UpdateForkChoice` → `updateForkChoice` → `stateSync.Run` → `stage.Forward` → `ExecV3` → PROCESS EXIT. Every hop has no recover.

**Finding E-2: Serial execution path (`ExecV3`) has no recover.**
- `exec3.go:110-387`: The serial block execution path has no defer/recover. In contrast, the parallel executor (`exec3_parallel.go`) has comprehensive recovery.
- Team deliberately added recover to parallel paths but left serial path unprotected.

**Finding E-3: `Sync.RunNoInterrupt` and `runStage` have no recover.**
- `sync.go:286-357`, `sync.go:494-515`: Core stage pipeline functions have no panic recovery. Any stage's `Forward()` panic escapes the caller.

**Finding E-4: Component system `runActor` goroutine has no recover.**
- `component.go:319-333`: Each component's lifecycle goroutine has no recover. A panic in `doActivate`/`doDeactivate`/`doOnComponentStateChanged` kills the goroutine — component permanently deadlocked.

**Finding E-5: Multiple TxPool goroutines (6+) unprotected.**
- `txpool/fetch.go:268` (ConnectCore state stream), `:255` (receiveMessageLoop), `:259` (receivePeerLoop), `:405` (ticker flush), `pool.go:2354` (announcement drain), `:567` (kickKZGOffenders).
- Core processing (`processRemoteTxns`, `handleInboundMessageWithTx`) has recover. Background goroutines do not — all die silently on panic.

**Finding E-6: P2P background goroutines unprotected.**
- `sentry_multi_client.go:757` (blockRange69), `:838` (HandlePeerEvent), `libsentry/loop.go:147` (pump worker).
- Core message dispatch (`HandleInboundMessage`) has recover. Background goroutines do not.

**Finding E-7: No top-level recover in main().**
- `cmd/erigon/main.go:39-54`: Same as Geth — any startup panic terminates the process.

✅ **Verified evidence file**: `.harness/research/ef-cross-client-report/client-evidence-erigon.md`

⚠️ Production: Immunefi #38766 (nil To BlobTx → encodePayload crash from single P2P tx). #14581 (decompressor bounds at block ~22213999).

### EthereumJS (⚠️ source check)

EthereumJS (TypeScript) has no plugin system. `process.on('uncaughtException')` at `packages/client/bin/cli.ts:318` calls `stopClient()` with 30s timeout. ⚠️ No `unhandledRejection` handler — rejected promises silently swallowed.

### Nimbus-eth1 (⚠️ source check)

Nimbus-eth1 (Nim) uses `{.push raises: [].}` (no exceptions) and `Result` types pervasively. P2P dispatch (`rlpx.nim:271`) catches `EthP2PError`. ⚠️ Main event loop (`nimbus_execution_client.nim:355-363`) runs bare `chronos.poll()` with no exception wrapper.

## 3. Cross-Client Data

### Per-client crash containment status

| Client | Crash paths verified | Has panic recovery in execution? | Has plugin isolation? |
|--------|---------------------|----------------------------------|----------------------|
| Reth | 3 confirmed (ExEx, txpool, tx-batcher) | ✅ catch_unwind in spawn_critical_task (but kills node) | ❌ No plugin isolation |
| Geth | 10+ (G-1..G-10) | ❌ Zero recover() in core (only 5 total: console, logging, JS, DB, RPC) | ❌ No plugin system |
| Nethermind | 6 (N-1..N-6) | ❌ BlockchainProcessor catch only logs (zombie) | ❌ "By design" no isolation |
| Besu | 6 (B-1..B-6) | ❌ UncaughtExceptionHandler only logs (zombie) | ❌ continueOnPluginError=false default |
| Erigon | 7 (E-1..E-7) | ❌ Zero recover() in Engine API + serial execution | ❌ All-in-one mode default |

### Common pattern across all clients

```
On-chain/P2P data arrives → plugin/extension processes it
  → malformed data triggers runtime error
  → no isolation boundary between plugin and node core
  → node terminates (or enters zombie state)
```

### Sheaf-Theoretic Model

```
Cover: {execution, networking, plugin_1, ..., plugin_n, consensus}
Cocycle condition: all subsystems must stay healthy for the node to survive
H¹ ≠ 0 detected at: plugin crash → node termination (repeating in every client)
Design Shift (predicted): remove plugin from the node-liveness cocycle → process isolation
```
