# EF Cross-Client Plugin Crash Isolation Report

面向 Ethereum Foundation Protocol Security Team 的完整報告。

## 文件結構

### 主要報告（建議閱讀順序）

| # | 文件 | 內容 |
|---|------|------|
| 1 | `01-executive-summary.md` | 執行摘要：問題 + 發現 + 建議 |
| 2 | `02-technical-analysis.md` | 技術分析：逐 client crash path + 跨 client 數據 |
| 3 | `03-threat-model.md` | 威脅模型：攻擊向量 + 經濟學 + 跨 client 場景 |
| 4 | `04-attack-surface.md` | 攻擊面分類：已修復/未修復/推測 |
| 5 | `05-recommendations.md` | 建議：code fix + EF policy + disclosure 策略 |

### 附錄

| 文件 | 內容 | 狀態 |
|------|------|------|
| `appendix/a-evidence-chain.md` | 證據鏈：每個 client 的 code trace | ✅ 有內容 |
| `appendix/b-methodology.md` | AID 分析方法論 | ✅ 有內容 |

## 證據層級

- ✅ verified: source code line panel-by-panel 確認
- ⚠️ confirmed: 外部證據（CVE, public incident, official issue）
- 🔶 inferred: 架構推理

## 關鍵數字

- **30+** confirmed crash paths across 5 EL clients
- **18+** production incidents（含 Ethereum + cross-chain）
- **0** 個 client 有 plugin/extension runtime containment
- **$2-30** 攻擊成本（一次 gas）
- **10 new** unpatched Geth crash paths discovered (G-1..G-10)

## Code Fix 狀態

Reth ExEx fix: verified local, 69 lines/2 files, `reth-exex` compiles clean. Upstream `paradigmxyz/reth` HEAD still vulnerable.

---

*報告版本: 2026-06-16*
