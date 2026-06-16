# Recommendations

## Audience

These recommendations are addressed to the Ethereum Foundation Protocol Security team, with secondary audiences being individual client teams.

## Two Layers

| Layer | Scope | Who | Timeline | Evidence |
|-------|-------|-----|----------|----------|
| **Code fixes** | Specific crash paths in each client | Client teams | Days | Verified locally: `reth-exex` compiles clean |
| **Policy & architecture** | Cross-client isolation standard, severity classification, lifecycle API | EF Protocol Security | Months | Directional — EF to design and coordinate |

Code fixes are verified and ready to merge. Policy changes require EF coordination and are provided as direction only.

## Immediate (0-30 days)

### 1. Cross-Client Disclosure Coordination

The EF should initiate a coordinated disclosure across all five EL client teams. The Reth ExEx fix can serve as a concrete case study. The EF's role is to ensure every client team understands that the same crash-containment gap exists in their codebase.

Per EF's public disclosure process (github.com/ethereum/public-disclosures), the cross-check phase exists for cross-client vulnerabilities. Evidence: crash paths confirmed in all 5 clients (see §2 Technical Analysis).

### 2. Reth ExEx Fix

Three components use `spawn_critical_task` and all must be fixed to guarantee node survival after ExEx crash:

| Component | Original | Fixed | Reason |
|-----------|----------|-------|--------|
| ExEx task | `spawn_critical_task` + `panic!` | `spawn_task` + `error!` | ExEx crash → ExEx dies, node lives |
| ExEx manager | `spawn_critical_task` + `.expect()` | `spawn_task` + `if let Err(err) = ...await` | Manager channel error → logged, not fatal |
| Notif forwarder | `spawn_critical_task` + `.expect()` | `spawn_task` + `if let Err(err) = ...await; break` | Forwarder send failure → logged, break |

Plus a fix in the manager itself: when an ExEx channel closes (from crash), the manager must skip the dead handle rather than return `Err` — which would propagate to kill the node.

**Verified**: `reth-exex` crate compiles clean with all manager changes. Diff: 2 files, 69 insertions, 27 deletions.

#### Option Analysis

| Option | Description | Effort | Isolation level | Risk |
|--------|-------------|--------|-----------------|------|
| **A (implemented)** | `spawn_task` for ExEx + manager + forwarder; manager skips dead handles | 69 lines across 2 files | Task-level: ExEx crash → removed from manager, node survives | Low: no behavioural change for healthy ExExes |
| **B** | Keep critical tasks, add supervisor that restarts crashed ExEx | 200-500 lines | Supervisor-level: crash → restart with backoff | Medium: supervisor adds new failure mode |
| **C** | Fork ExEx execution into a separate OS process | 2000-5000 lines | Process-level: crash → zero effect on node | High: IPC overhead, state sync complexity |

Option A is the verified fix. Options B and C are architectural upgrades for the EF-wide standard.

### 3. Fix Verification

For each client, verify that:
- Plugin/extension crash does not terminate the node
- Plugin crash produces a diagnostic log/metric
- Plugin can be restarted without node restart (or at minimum, the node survives the crash)
- Fix does not reduce the severity of genuine critical task failures

## Short-term (30-90 days)

### 4. Audit Remaining Critical Plugin Surfaces

| Client | Priority surfaces to audit |
|--------|--------------------------|
| Reth | Txpool maintenance (`pool.rs:281`), tx-batcher (`core.rs:338`) |
| Geth | All goroutines outside RPC — sync, txpool, P2P handlers |
| Nethermind | Plugin event handler hooks, BlockchainProcessor recovery |
| Besu | Runtime block creation hooks, lifecycle halt-on-error |
| Erigon | All stagedsync stages, all-in-one mode risks |

### 5. Establish EF-Wide Isolation Standard

Define a minimum isolation standard for EL client plugins/extensions:

| Tier | Requirement | Implementation example | Status |
|------|------------|----------------------|--------|
| **T1 (minimum)** | Plugin crash → node survives. Diagnostic log + metric emitted | Reth ExEx (verified fix, 2 files) | Only Reth ExEx has this |
| **T2 (recommended)** | Supervisor-managed lifecycle: crash detection → log → restart with exponential backoff | No client implements this | Greenfield |
| **T3 (ideal)** | OS process isolation: plugin in separate process, zero crash propagation | Erigon sentry/RPC daemon (partial) | None for plugins |

### 6. Add Regression Tests to EF Test Suite

The EF test vector suite should include:
- Malformed event data that passes EVM but fails ABI/RLP parsing
- Empty/nil edge cases for state access patterns
- Boundary condition blocks for hardfork transitions

## Medium-term (90 days - 1 year)

### 7. EF Policy: Establish Cross-Client Architecture Gap Classification

EF's severity classification (Low/Medium/High per single-client bug) has no category for cross-client architectural gaps. Data: ~30+ crash paths confirmed across all 5 clients, each patched per-instance, with no category for the cross-client pattern.

**Recommendations:**
- Add "Architectural Gap" as a classification alongside Fork, DoS, Consensus, Specification
- Any finding affecting ≥3 EL clients at the architecture level should enter a separate review channel
- Severity assessment must include a cross-client impact dimension

### 8. EF Policy: Mandate Plugin Isolation in Client Specification

The EF should define a minimum plugin isolation requirement as part of the EL client specification. This is not a consensus change — it is a quality-of-security requirement for participation in the Ethereum network.

**Minimum requirements:**
- Plugin crash cannot terminate the node (T1 isolation)
- Plugin crash produces structured log output (plugin ID, crash type, trigger source)
- Plugin can be detected as crashed via a health API

### 9. EF Architecture: Plugin Lifecycle API Design

The EF should define a set of lifecycle hook signatures for plugin/extension systems. These are not mandatory — they serve as a coordination point so all client teams build compatible systems.

**Direction:**
- Init: plugin returns error → node logs and skips plugin, does not halt
- Runtime: plugin panic → crash attribution log, manager marks dead
- Restart: optional supervisor re-creates plugin from WAL/backfill
- Health: plugin exposes health-check endpoint

### 10. EF Architecture: Crash Attribution Standard

Every client should output crash diagnostics in a consistent format:

```
plugin_id=<id>
crash_type=<panic|oom|assert>
trigger_source=<tx_hash|contract_address|block_number>
affected_subsystems=<consensus,storage,rpc,...>
```

This allows operators to write monitoring rules that distinguish attack crashes from accidental crashes.

### 11. Explore Formal Verification of Core Isolation Boundaries

The sheaf-theoretic model used in §2.3 (H¹ ≠ 0 at the plugin-node boundary) identifies a repeating failure pattern across all 5 clients. Formal verification tools such as Halley Young's Deppy (arXiv:2603.27015, Čech cohomology analysis, 100% recall at 69% precision across 375 benchmarks) may be applicable to specifying isolation boundaries. This is a longer-term directional suggestion.

## Disclosure Strategy

Individual crash paths have been discovered independently in each client (documented CVEs, Immunefi bounties, production incidents). The cross-client pattern — that all 5 clients have the same containment gap — has not been previously documented. Disclosure should:

1. Present the cross-client data first (technical analysis §2), not individual CVEs
2. Give each client team 30 days to audit their plugin/extension isolation
3. Publish coordinated advisories on a single date
4. The EF's public-disclosures repository is the appropriate publication channel (90-day timeline per existing policy)

### Classification Gap

EF's existing severity classification (Low/Medium/High per single-client bug) has no category for cross-client patterns. Data: 30+ crash paths across 5 clients, each patched per-instance. Without a cross-client classification, the pattern risks being addressed as 5 individual bugs rather than one coordinated fix.


