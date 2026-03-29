# `Generated()` が空を返す原因の仮説と調査記録 (2026-03-03)

## 前提：確認済みの事実

### 実装済みの状態

`boolean_common` / `boolean_cut` は現在、`BRepBuilderAPI_Copy` を呼ぶ**前**に
`Generated()` を呼ぶ実装になっている（Bug 1 fix との両立を解決済み）。

```cpp
BRepAlgoAPI_Common common(a, b);
common.Build();
if (!common.IsDone()) return nullptr;
// ↓ Copy 前に Generated() を呼んでいる
TopoDS_Shape new_faces = collect_generated_faces(common, b, common.Shape());
BRepBuilderAPI_Copy copier(common.Shape(), ...);
```

### 診断テストの結果（`diagnose_new_faces`）

`lambda360box` を 2 種類のツールで intersect したときの `new_faces` フェイス数：

| ツール | フェイス数（`b` が持つ面） | `Generated()` の結果 | 幾何マッチング後 |
|--------|--------------------------|----------------------|-----------------|
| `half_space`（非有界平面面） | 1 面 | **0** | 2 |
| `big_box`（有界 x∈[-1000,1]） | 6 面 | **0** | 2 |

**重要**: `Generated()` は half_space だけでなく有界ボックスに対しても空を返す。
これは「非有界フェイスだから履歴テーブルに登録されない」という仮説を否定する。

---

## 仮説 1：`Generated` ではなく `Modified` を呼ぶべき（最有力）

### 根拠

OCCT の `BRepAlgoAPI_Common(a, b)` が返す結果形状には、次の 2 種類のフェイスが含まれる：

1. **`a` のフェイスのうち `b` の内側にあるもの** → `a` のフェイスが trimmed された版
   → `Modified(face_of_a)` で取得可能

2. **`b` のフェイスのうち `a` の内側にあるもの** → `b` の境界面が trimmed された版
   → **これが断面フェイス**

断面フェイスは `b` の境界面（half_space の平面面・box の x=1.0 面）が
形状 `a` で **トリミングされた（有界化された）版**である。
これは「新規に生成された」のではなく「修正された」ものであり、
OCCT のセマンティクスでは `Modified(face_of_b)` で返るべき。

```
Generated(S): S から完全に新規のシェイプが生まれた（S と別の TShape）
Modified(S):  S が変形・トリミングされた（S と同一平面だが形状が変わった）
```

half_space の境界面は無限平面 → intersect 後は有界な断面 → これは "modification"

### 検証手順

`collect_generated_faces` の中で `op.Generated(face)` の代わりに
`op.Modified(face)` を試す：

```cpp
for (TopExp_Explorer ex(tool, TopAbs_FACE); ex.More(); ex.Next()) {
    const TopTools_ListOfShape& mod = op.Modified(ex.Current());  // Generated → Modified
    for (const TopoDS_Shape& s : mod) {
        builder.Add(raw, s);
    }
}
```

---

## 仮説 2：`myFillHistory` が False になっている

### 根拠

OCCT の `BRepAlgoAPI_BuilderShape::Generated(S)` の実装：

```cpp
const TopTools_ListOfShape& BRepAlgoAPI_BuilderShape::Generated(const TopoDS_Shape& S)
{
    myGenerated.Clear();
    if (myFillHistory) {                      // ← ここが False なら常に空
        myHistory->Generated(S, myGenerated);
    }
    return myGenerated;
}
```

`myFillHistory` のデフォルト値は OCCT ソース上は `Standard_True` だが、
バージョンや Build フラグによっては `Standard_False` になりうる。

### 検証手順

```cpp
BRepAlgoAPI_Common common(a, b);
common.SetToFillHistory(Standard_True);  // 明示的に有効化
common.Build();
```

`BRepAlgoAPI_BuilderShape` が `SetToFillHistory` を持つかどうかを確認する。

---

## 仮説 3：`BRepAlgoAPI_Common` のツール境界フェイスは履歴テーブルに登録されない

### 根拠

`BRepAlgoAPI_Common` は内部で `BOPAlgo_BOP` を使用する。
`BOPAlgo_BOP` の履歴テーブルへの登録処理は「対象シェイプから生成されたエッジ・フェイス」を
記録するが、**ツール (`b`) 側のフェイスが結果に残るケース**（intersection では
ツールの断面がそのまま結果に入る）については、`Modified` として登録するか
`Generated` として登録するか、実装依存の可能性がある。

OCCT ソースの `BOPAlgo_BuilderFace.cxx` や `BOPDS_DS.cxx` の詳細を確認する必要がある。

### 補足：`SectionEdges()` は別経路

`SectionEdges()` は `myHistory` を使わず `myDS->SectionEdges()` から取得するため、
仮説 2・3 の影響を受けない。

```cpp
// BRepAlgoAPI_BuilderShape の実装（概略）
const TopTools_ListOfShape& SectionEdges() const {
    return myBuilder->SectionEdges();  // BOPAlgo_Builder::mySectionEdges
}
```

`BOPAlgo_Builder::mySectionEdges` は Boolean 演算時に常に更新される。
これが `SectionEdges()` がより信頼できる理由。

---

## 仮説の優先度

| # | 仮説 | 可能性 | 検証コスト |
|---|------|--------|-----------|
| 1 | `Modified()` を呼ぶべき | **高** | 1 行変更 |
| 2 | `myFillHistory` が False | 中 | `SetToFillHistory(true)` 追加 |
| 3 | ツール境界フェイスが履歴未登録 | 中 | OCCT ソース精査が必要 |

---

## 解決策の候補

### 解決策 A：`Modified()` を使う（仮説 1 の検証を兼ねる）

```cpp
// Generated の代わりに Modified を試す
for (TopExp_Explorer ex(tool, TopAbs_FACE); ex.More(); ex.Next()) {
    for (const TopoDS_Shape& s : op.Modified(ex.Current())) {
        builder.Add(raw, s);
    }
}
```

### 解決策 B：`SectionEdges()` → ワイヤー → フェイス再構築

`SectionEdges()` は断面エッジを確実に返す（別経路）。
エッジ → 閉じたワイヤー → プレーナーフェイス の手順で断面フェイスを再構築する。

```cpp
const TopTools_ListOfShape& edges = op.SectionEdges();
// ShapeAnalysis_FreeBounds::ConnectEdgesToWires で閉ワイヤーに整理
// BRepBuilderAPI_MakeFace(wire, Standard_True) でプレーナーフェイスを生成
```

opencascade-rs (bschwind) も `SectionEdges()` を採用している（`Generated()` は未使用）。
ただし同プロジェクトではエッジのみ返しており、フェイスへの変換は行っていない。

### 解決策 C：現行の幾何マッチング（廃棄候補）

ツールの各平面と結果フェイスの法線・重心を比較して断面を特定する。
動作するが OCCT の内部構造を使わない heuristic であり、厳密でない。
ユーザーの要求により廃棄対象。

---

## 検証結果（2026-03-03 実施）

### 仮説 1 — 確定 ✓

`op.Generated(face)` → `op.Modified(face)` に 1 行変更した結果：

| ツール | `Modified()` の結果 |
|--------|---------------------|
| `half_space`（非有界） | `new_faces face_count=2` ✓ |
| `big_box`（有界） | `new_faces face_count=2` ✓ (center.x=1.0, normal=(1,0,0)) |

全 24 テスト通過。`stretch_box_known_error_case_1_0_1` も `shell_count=1` で通過。

### 根本原因の確定

断面フェイスは `b`（ツール）の境界面が `a`（形状）によってトリミングされた版であり、
OCCT はこれを **「修正されたフェイス」= `Modified()`** として記録する。
`Generated()` は呼ぶ相手が間違いだった。

```
Generated(S) = S から全く新規のシェイプが生まれた  → 断面には該当しない
Modified(S)  = S が形状変化（トリミング）された    → 断面はこれ ✓
```

仮説 2・3 の検証は不要。
