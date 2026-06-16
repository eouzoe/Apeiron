# Threat Model

## Evidence Levels

- ✅ **verified**: traced from source code or public client repo
- ⚠️ **confirmed**: supported by external evidence (CVE, issue, public disclosure)
- 🔶 **inferred**: derived from architectural analysis, not yet empirically verified on all clients

## Attack Vector: On-Chain Trigger

The attacker deploys a contract that emits event data passing EVM execution but failing when parsed by an ExEx or plugin:

```solidity
contract Landmine {
    event Trigger(bytes data);
    function fire() external {
        emit Trigger(hex"deadbeef");  // EVM-valid LOG, fails ABI decode
    }
}
```

The transaction is valid in the EVM, produces a LOG in the receipt, and survives consensus. Any plugin that processes this log and uses `?` on the data will panic — and with no isolation, the node terminates.

✅ **Permanent trigger**: The transaction lives on-chain indefinitely. Every reorg replay, restart, or re-sync reaches the same block and retriggers the crash.

✅ **Zero traceability**: The attack transaction is indistinguishable from normal contract interactions to the operator.

### Client Coverage (⚠️ confirmed via cross-client analysis)

| Client | Exploitable via | Confirmed paths | Verdict |
|--------|----------------|-----------------|---------|
| Reth | Any ExEx `?` on chain data | 3+ (ExEx, txpool, tx-batcher) | ✅ Kill chain verified in source |
| Geth | Nil deref in txpool/sync/P2P | 10+ CVE/issue | ⚠️ Multiple production incidents |
| Nethermind | Plugin crash in core hooks | 6+ (chain split, blob, OOM) | ⚠️ "By design" no isolation |
| Besu | RuntimeException in block creation | 6+ (2024 halt, CVE chain) | ⚠️ Production chain halt |
| Erigon | Stage panic from malformed block/tx | 5+ (Immunefi, bounds crash) | ⚠️ Single P2P tx can kill |

## Attack Economics

### Direct Attack Cost (✅ verified on Ethereum mainnet economics)

- **Gas cost**: ~$2-30 per deployment + emit (one-time)
- **Optimisation paths**: L2 deployment, contract factories, low-gas periods (🔶 inferred, economically sound but not verified client-by-client)
- **Batch deployment**: One contract can emit multiple event signatures. A single deployment can target multiple plugin types simultaneously

### Attacker Motivations

| Motivation | Plausibility | Evidence |
|------------|-------------|----------|
| MEV extraction by disabling competitor plugins | 🔶 Possible | MEV market is competitive; disabling opponent's ExEx gives exclusive MEV access |
| Market manipulation via short position | 🔶 Hypothetical | Coordinated node takedowns could affect ETH price. Not verified as a realistic attack strategy |
| Network disruption by state actors | 🔶 Hypothetical | Historical precedent in crypto (Lazarus group) but no direct evidence for this specific attack |
| Ransom / extortion of plugin operators | 🔶 Hypothetical | Possible but unverified |

**Important note on shorting**: This report does not quantify profit from short positions. The relationship between node outages and token prices is complex and depends on many factors outside the scope of this analysis. Shorting is listed only as one possible motivation among several.

## Scaling

✅ **Architecturally confirmed**: The attack is scalable because:
1. The trigger is on-chain and permanent. One transaction reaches every node that processes that block
2. The vulnerability is cross-client. Blocked on Reth? Target Geth nodes instead
3. The deployment is cheap (~$2-30) relative to potential impact
4. Multiple event signatures in one contract can target multiple plugin types

No coordinated multi-client attack has been observed in production. These scaling properties derive from the architecture, not from empirical observation.

## Detection Difficulty

- **Before detonation**: The trigger contract and transaction are indistinguishable from legitimate activity. No static analysis would flag them
- **After detonation**: The operator sees a node crash and restarts. Standard operating procedure. The crash is attributed to a "bug" not an "attack"
- **Attribution**: Low. The trigger transaction is identical in form to any other event-emitting transaction. No crash attribution data exists across EL clients

## Cross-Client Implications

Evidence: crash paths confirmed in all 5 EL clients (see §2 Technical Analysis). Production incidents in 5 of 5 clients. Example sequences:

- Scenario A: Attacker targets Geth's G-6..G-10. Geth patches all 5. Attacker switches to Reth ExEx.
- Scenario B: Attacker deploys 5 triggers simultaneously (one per client). Patching any single client reduces population affected but does not eliminate the threat.
- Scenario C: One trigger contract targets Geth (G-6, ~60% nodes). Network loses finality. Remaining clients (Nethermind ~20%, Besu ~10%, Erigon+Reth ~10%) cannot absorb Geth's load — cascading failure by redirected traffic.
