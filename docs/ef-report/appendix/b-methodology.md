# Appendix B: Analysis Methodology

## B.1 Analytical Framework

This report was produced using the AID (Assumption → Intervention → Design Shift) analytical framework — a three-phase method for identifying implicit assumptions, constructing falsification tests, and rearchitecting system boundaries. Full theoretical foundation is documented in `docs/aid-theory-foundation.md`.

### Phase 1: Assumption Surfacing (A)

**Seed question**: "Can an on-chain transaction crash an EL node?"

**Implicit assumptions identified (via REVERSE ×3):**

1. **Surface assumption**: crashes are bugs that need patching
   - Deeper: crash paths are implementation errors, not architectural failures
   - Deepest: the plugin-core boundary is a matter of configuration, not protocol

2. **Surface assumption**: a single client patching its bugs is sufficient
   - Deeper: the fix must be client-specific because each client has different code
   - Deepest: cross-client vulnerability patterns don't exist because each client is independently developed

3. **Surface assumption**: plugin/extension ecosystems are low-risk because they're optional
   - Deeper: optional code paths can't affect core node liveness
   - Deepest: the runtime boundary between "optional" and "core" is enforced by development discipline

All three deep assumptions were falsified by examining the source code.

### Phase 2: Falsification Testing (I)

**Prediction**: Plugin crash → node termination can be reproduced from on-chain data.

**Verification status by client**:

| Client | Predicted | Verified | Verification method |
|--------|-----------|----------|-------------------|
| Reth | Yes | ✅ | 5-hop kill chain from source code. Lines identified and traced |
| Geth | Yes | ⚠️ 10+ confirmed CVEs/incidents. Architecture: no recover() outside RPC |
| Nethermind | Yes | ⚠️ 6+ confirmed incidents. Official statement confirms no isolation |
| Besu | Yes | ⚠️ 6+ confirmed incidents. Architecture: no crash containment in block creation |
| Erigon | Yes | ⚠️ 5+ confirmed incidents. Architecture: no recover() in stagedsync |

**Cross-client prediction**: The same architectural gap exists in all five clients — confirmed. The vulnerability is not a single-client bug but an industry-wide structural gap.

### Phase 3: Design Shift (D)

**Architectural insight**: Plugin/core cocycle condition (both must stay healthy) is unenforceable without a formal boundary.

**Design shift required**: Plugin isolation must be promoted from "nice to have" to a protocol-level requirement, enforced by an EF-wide standard.

**Minimum viable shift**: Task-level isolation (spawn_task in Reth) + supervisor layer that removes crashed plugins without affecting the node.

## B.2 Thinking Tools Applied

The following Dennett intuition pumps were used at various analysis stages (full framework: `docs/thinking-tools.md`):

- **Two Black Boxes** (layers 1-2): Node = black box A (core), plugin = black box B (plugin). Layer 2 insight: A and B share liveness — B's crash kills A. Layer 3 analysis: Why? Because A sets `B.kill() => A.kill()` via spawn_critical
- **Sorting Hat** (layers 1-2): spawn_critical sits on the "must not fail" side. ExEx data sits on the "may fail" side. Category mismatch
- **Flooding** (layer 2): If the strongest client fix is applied (Reth spawn_task), does the vulnerability disappear? No — the other 4 clients remain exposed

## B.3 Evidence Classification

Every claim in this report is tagged with an evidence level:

| Tag | Meaning | Example |
|-----|---------|---------|
| ✅ verified | Traced from source code, line numbers available | Reth 5-hop kill chain |
| ⚠️ confirmed | Supported by external evidence (CVE, issue, public disclosure) | Geth CVE-2020-26242 |
| 🔶 inferred | Architectural inference, not yet empirically verified on all clients | Scaling economics |

## B.4 Limitations

1. **Not all clients source-audited for every crash path**: The cross-client analysis is based on CVEs, public incidents, and broad architecture review. Full source audit per client would require dedicated time per client team
2. **No reproduction scripts**: The analysis identifies crash paths but does not provide production-ready PoC code. The Reth ExEx kill chain is the most completely traced; other clients have documented crash paths via CVE references
3. **No economic model**: The report describes attacker motivations qualitatively. Quantifying the economic impact would require market data and financial modelling outside the scope of this analysis
4. **Survival rate estimates are inferred**: No data exists on node crash attribution rates across client operators. The 6+ month survival estimate is an architectural projection, not an empirical measurement
5. **Lurking + scaling confirmed at architecture level, not empirically**: The vulnerability is structurally scalable (cheap, permanent, cross-client triggers), but no coordinated multi-client attack has been observed in production

## B.5 References

- AID methodology: Das et al. (2021a, 2021b), Cieśliński et al. PRL 2026
- Sheaf cohomology application to program analysis: Halley Young, "Deppy" (arXiv:2603.27015)
- Dennett intuition pumps: "Intuition Pumps and Other Tools for Thinking" (Dennett 2013)
- Detailed AID theory: `docs/aid-theory-foundation.md`
- Thinking tools framework: `docs/thinking-tools.md`
