# Gordon Surface の数学と自力実装方針

OCCT / occ_gordon / TiGL に頼らず、cadrum で Gordon surface を一から実装するための
数学的な foundation と、離散化したときのアルゴリズム手順をまとめる。

## 1. 問題設定

**与えられるもの**: 2 群の曲線

- **profile** (断面曲線): $P_1(u), P_2(u), \dots, P_M(u)$ — 全て U 方向のパラメタ $u \in [0,1]$ を持つ
- **guide**   (ガイド曲線): $G_1(v), G_2(v), \dots, G_N(v)$ — V 方向の $v \in [0,1]$ を持つ

この 2 群は「曲線ネットワーク」を形成し、各 $P_i$ と各 $G_j$ はグリッド状に**交差**する。

交差点を
$$
Q_{ij} \;=\; P_i(u_j) \;=\; G_j(v_i) \qquad (i=1,\dots,M,\ j=1,\dots,N)
$$
と定義する。ここで $v_i$ は "profile $P_i$ が V 方向でどこに位置するか" を表すパラメタで、
$u_j$ は "guide $G_j$ が U 方向でどこに位置するか"。$Q_{ij}$ は profile と guide の
幾何学的交点。

**求めるもの**: 両方向のパラメタ $(u,v) \in [0,1]^2$ を持つサーフェス $S(u,v)$ で、

$$
S(u, v_i) \;=\; P_i(u) \qquad \forall u, i
$$
$$
S(u_j, v) \;=\; G_j(v) \qquad \forall v, j
$$

つまり **profile を V 等値曲線として、guide を U 等値曲線として、どちらも厳密に**
通るサーフェス。

## 2. なぜ素朴な案は失敗するか

### 案 A: profile だけを skin する

profile 群 $\{P_i\}$ を V 方向に補間して「loft」を作る:

$$
S_P(u, v) \;=\; \sum_{i=1}^{M} \alpha_i(v)\, P_i(u)
$$

ここで $\alpha_i(v)$ は **cardinal 関数** (「$v = v_k$ で $\alpha_i(v_k) = \delta_{ik}$」を満たす
V 方向の基底関数、例えば cubic BSpline 補間基底)。

$$
S_P(u, v_i) \;=\; \sum_k \alpha_k(v_i)\, P_k(u) \;=\; P_i(u) \qquad \checkmark
$$

profile は厳密に通るが、
$$
S_P(u_j, v) \;=\; \sum_k \alpha_k(v)\, P_k(u_j) \;=\; \sum_k \alpha_k(v)\, Q_{kj} \;\neq\; G_j(v)
$$

**guide は一般に通らない**。$S_P(u_j, v)$ は guide $G_j$ の「$v_1, \dots, v_M$ 地点での値だけ
からの補間」に過ぎず、guide 全体の形状情報は反映されていない。

### 案 B: guide だけを skin する

対称に、
$$
S_G(u, v) \;=\; \sum_{j=1}^{N} \beta_j(u)\, G_j(v)
$$
で定義すると guide は厳密に通るが profile は通らない。

### 案 C: 交点格子だけの tensor 積補間

$M \times N$ 個の $Q_{ij}$ を tensor product で補間する:

$$
S_{PG}(u, v) \;=\; \sum_{i=1}^{M} \sum_{j=1}^{N} \alpha_i(v)\, \beta_j(u)\, Q_{ij}
$$

このサーフェスは $(u_j, v_i)$ の格子点 $M \cdot N$ 個を通るが、**profile も guide も一般に通らない**
(サンプル点の間は素朴な補間で、曲線の実形状は知らない)。

どの単発案も不十分。

## 3. Gordon の洞察: Boolean Sum (Transfinite 補間)

W. J. Gordon (1969, 1971) のアイデアは、上の 3 サーフェスの線形結合

$$
\boxed{\; S(u, v) \;=\; S_P(u, v) \;+\; S_G(u, v) \;-\; S_{PG}(u, v) \;}
$$

を取ると、**profile も guide も両方厳密に通過する**というもの。名前は「**Boolean sum**」
または「Gordon-Coons surface」。Coons patch (4 辺の境界曲線だけのパッチ) を $M \times N$ の
網に一般化したもの。

## 4. 厳密な導出

### 4.1 $v = v_k$ で profile が通るか

$$
S(u, v_k) \;=\; S_P(u, v_k) + S_G(u, v_k) - S_{PG}(u, v_k)
$$

各項を評価する:

- $S_P(u, v_k) = P_k(u)$ (上で確認済み)
- $S_G(u, v_k) = \sum_j \beta_j(u)\, G_j(v_k) = \sum_j \beta_j(u)\, Q_{kj}$
  ($G_j$ は $v = v_k$ で点 $Q_{kj}$ を通るため)
- $S_{PG}(u, v_k) = \sum_i \alpha_i(v_k) \sum_j \beta_j(u) Q_{ij}
                = \sum_j \beta_j(u) Q_{kj}$
  ($\alpha_i(v_k) = \delta_{ik}$ なので $i$ の和は $i=k$ だけ残る)

従って
$$
S(u, v_k) \;=\; P_k(u) + \sum_j \beta_j(u) Q_{kj} - \sum_j \beta_j(u) Q_{kj}
            \;=\; P_k(u) \qquad \checkmark
$$

**$S_G$ の第 2 項と $S_{PG}$ の tensor 項が完全にキャンセル**する。これが Boolean sum の鍵。

### 4.2 $u = u_\ell$ で guide が通るか

対称な計算で

$$
S(u_\ell, v) \;=\; G_\ell(v) \qquad \checkmark
$$

両方向の条件を同時に満たすので、$S$ は $M + N$ 本全ての曲線を「厳密な等値曲線」として
含む唯一の滑らかな補間。これが Gordon surface の定義。

### 4.3 $M = N = 2$ の場合: Coons patch

$M=N=2$ で退化させると、4 辺 $P_1, P_2, G_1, G_2$ だけの Coons patch
(双線型 Boolean sum) に一致する。Gordon は Coons の $M, N$ 方向への自然な一般化。

## 5. 離散化: BSpline 基底で計算する

連続関数 $\alpha_i(v), \beta_j(u)$ を**実装可能な BSpline 基底**に置き換える。

### 5.1 基底関数の選び方

cubic (次数 3) BSpline basis を使う。

- 入力: V 方向のノットベクトル $\{v_1, \dots, v_M\}$ (profile の位置) と U 方向の $\{u_1, \dots, u_N\}$
- "$v = v_k$ で $\alpha_i(v_k) = \delta_{ik}$" は cardinal BSpline 補間として定義:
  まず boundary 条件 (clamped / periodic) 付きの BSpline 基底 $\{N_i(v)\}$ を作り、
  $M$ 個の interpolation 方程式
  $$
  \sum_{i=1}^{M} a_{ki} N_i(v_k) = \delta_{ki} \cdot \mathbf{1} \quad (k=1,\dots,M)
  $$
  を解いて cardinal 変換行列 $A = [a_{ki}]$ を得る。$\alpha_i(v) = \sum_k a_{ki} N_k(v)$。
- ただしこの「cardinal 化」は解析的に隠せる: 実装上は

  $$
  S_P(u, v) \;=\; \sum_k N_k(v)\, \tilde P_k(u),
  \qquad
  \tilde P_k(u) = \sum_i a_{ki}\, P_i(u)
  $$

  と展開し、$\tilde P_k$ を profile の**線形結合で事前に計算**してから BSpline 基底 $N_k(v)$ で
  skin する。数値的には cardinal 行列を解くのと 1 次元補間を skinning するのは等価。

### 5.2 3 サーフェスの BSpline 表現

全ての profile $P_i$ が**同じ U 方向 knot vector** を持ち、全ての guide $G_j$ が**同じ V 方向 knot vector** を持つように
事前に再パラメタ化しておくと、3 サーフェスは以下のように書ける (すべて tensor product BSpline):

1. **profile surface** $S_P$:
   $$
   S_P(u, v) = \sum_{a=1}^{P_u} \sum_{b=1}^{M} \mathbf{C}^{P}_{a,b}\; N^u_a(u)\, M^v_b(v)
   $$
   ここで $\mathbf{C}^{P}_{a,b}$ は profile 群を V 方向に skin した時の制御点。
   $N^u_a, M^v_b$ はそれぞれ U, V 方向の BSpline basis。
   $P_u$ は profile curve の制御点数、$M$ は profile 本数。

2. **guide surface** $S_G$:
   $$
   S_G(u, v) = \sum_{a=1}^{N} \sum_{b=1}^{P_v} \mathbf{C}^{G}_{a,b}\; N^u_a(u)\, M^v_b(v)
   $$

3. **tensor surface** $S_{PG}$:
   $$
   S_{PG}(u, v) = \sum_{a=1}^{N} \sum_{b=1}^{M} \mathbf{C}^{T}_{a,b}\; N^u_a(u)\, M^v_b(v)
   $$
   ここで $\mathbf{C}^{T}$ は格子点 $\{Q_{ij}\}$ を tensor 補間して得た制御点。

### 5.3 Boolean sum の離散化

3 サーフェスの次数と knot vector を**一致させる** (knot insertion による unification) と、
制御点どうしの足し算に帰着する:

$$
\mathbf{C}^{S}_{a,b} \;=\; \mathbf{C}^{P}_{a,b} + \mathbf{C}^{G}_{a,b} - \mathbf{C}^{T}_{a,b}
$$

最終サーフェスは
$$
S(u, v) \;=\; \sum_a \sum_b \mathbf{C}^{S}_{a,b}\; N^u_a(u)\, M^v_b(v)
$$
で、OCCT で言えば `Geom_BSplineSurface(poles, uKnots, vKnots, uMults, vMults, uDeg, vDeg, uPeriodic, vPeriodic)` に流し込めば完成。

## 6. 実装アルゴリズム (step-by-step)

### Phase 1: 入力準備

1. **compatibility**: profile 群を同じ U 次数 (= 3) と同じ U knot vector に揃える。
   - 全 profile が同じ点数 $P_u$ の制御点を持つように unify
   - guide も同じく次数 3 の共通 V knot vector に揃える
   - 不一致なら `GeomBSpline` の knot insertion で揃える (1D 操作)

2. **parameter 割り当て**:
   - $v_i$ = profile $i$ が存在する V 座標。**一様**なら $v_i = (i-1)/(M-1)$ (clamped)
     または $v_i = (i-1)/M$ (periodic)
   - $u_j$ も同様に
   - 交点 $Q_{ij} = P_i(u_j)$ を評価して $M \times N$ 点配列を作る

### Phase 2: 3 サーフェスを作る

3. **profile surface $S_P$** — V 方向 1D 補間 (skinning):
   - 各 U 制御点インデックス $a$ について、V 方向の「$M$ 本の点列」(profile $i$ の $a$ 番目の pole) を
     cubic BSpline 補間ソルバーに渡し、V 方向の $M$ 個 (clamped) or $M$ 個 (periodic) の
     新制御点を得る → $\mathbf{C}^{P}_{a, 1..M_v}$
   - これは **1D BSpline interpolation を $P_u$ 回**繰り返す帯状処理。本体は $M \times M$ の
     線形系ソルバー (clamped は三重対角、periodic は循環三重対角)

4. **guide surface $S_G$** — U 方向 1D 補間:
   - 対称に、guide 群を U 方向に skin して $\mathbf{C}^{G}_{1..N_u, b}$ を得る

5. **tensor surface $S_{PG}$** — 2D 補間:
   - $M \times N$ 点 $Q_{ij}$ を BSpline tensor 積補間 (2D)
   - 手順: 1D 補間を 2 回: まず U 方向に $M$ 回 (各 $v_i$ 行について) → 中間制御点 → 次に V 方向に
     その中間結果の列ごとに $N_u$ 回補間
   - 最終的に $\mathbf{C}^{T}_{1..N_u, 1..M_v}$

### Phase 3: 統一と和

6. **knot vector の unification**: $S_P, S_G, S_{PG}$ は U 方向・V 方向の基底が**事前に
   揃っている**はずなので (Phase 1 の compatibility で保証)、追加 knot insertion は不要。
   ただし skin の内部で自動生成された knot と、tensor の内部で生成された knot が一致
   していることを確認する

7. **Boolean sum**:
   $$
   \mathbf{C}^{S}_{a,b} = \mathbf{C}^{P}_{a,b} + \mathbf{C}^{G}_{a,b} - \mathbf{C}^{T}_{a,b}
   $$
   pole 配列の pointwise 引き算

8. **`Geom_BSplineSurface` インスタンス化**: コンストラクタに poles, uKnots, vKnots,
   uMults, vMults, uDeg=3, vDeg=3, uPeriodic, vPeriodic を渡す。これが最終 Gordon surface。

### Phase 4: 検証

9. 各 profile $P_i$ の制御点と surface の $V = v_i$ iso curve の制御点が一致するか確認
10. 各 guide $G_j$ の制御点と surface の $U = u_j$ iso curve の制御点が一致するか確認
11. 交点 $Q_{ij}$ で $\|S(u_j, v_i) - Q_{ij}\| < 10^{-9}$ を確認

## 7. 周期境界 (torus) の特殊事情

核融合向けの torus topology では U も V も周期。以下の差分が必要:

### 循環三重対角ソルバー

通常の cubic BSpline 補間は三重対角の線形系 (Thomas algorithm で $O(n)$) を解くが、
周期では**循環 (cyclic) 三重対角**になる。これは

- Sherman-Morrison 公式: 循環項をランク 1 補正として扱い、三重対角ソルバーを 2 回呼ぶ
- または FFT ベース solver (Toeplitz 循環行列は FFT で対角化できる)

いずれかで $O(n)$ or $O(n \log n)$ で解ける。cadrum では `ndarray` + 手書き Thomas で十分。

### cardinal 周期 BSpline 基底

周期 BSpline の基底関数 $N_k(u)$ は $u$ に関して $2\pi$ (or period 1) 周期を持ち、制御点数
$= $ 補間点数 (non-periodic のような "degree 個多い pole" は不要)。これにより
$P_u = M$、$N_v = N$ のピッタリな tensor grid が作れる → GordonBuilder が詰まっていた
「pole count mismatch」が**そもそも発生しない** (自前実装なら BSpline 関係式を**自分で**
選ぶので 1:1 対応を保てる)。

### 周期方向の $v_i$ 配置

$v_i = (i-1)/M$ ($i=1,\dots,M$) とし、$v_{M+1} = v_1$ で循環。U 方向も同様。循環補間では
interpolation 行列が circulant 構造を持つので FFT で直接解ける。

## 8. 参考文献

- **Gordon, W.J.** (1971). "Blending-function methods of bivariate and multivariate
  interpolation and approximation." *SIAM J. Numer. Anal.* 8(1), 158–177.
- **Farin, G.** (2002). *Curves and Surfaces for CAGD*, 5th ed. Academic Press.
  章: "Coons patches", "Gordon surfaces"
- **Piegl, L., Tiller, W.** (1997). *The NURBS Book*, 2nd ed. Springer.
  §9.4 Local Interpolation Through Surface Points, §9.5 Gordon surfaces
- **Siggel, M., Kleinert, J., Stollenwerk, T., Maierl, R.** (2019). "TiGL: An
  Open Source Computational Geometry Library for Parametric Aircraft Design."
  *Math. Comput. Sci.* 13(3), 367–389. DOI:10.1007/s11786-019-00401-y
  TiGL/occ_gordon の実装論文。doubly-periodic 未対応。

## 9. cadrum での実装ステップ (現実的なロードマップ)

### Phase A: 1D 補間基盤 (Rust/C++ どちらでも)

- [ ] cubic BSpline (clamped + periodic) の 1D 補間ソルバー
  - 点列と parameter 列と periodic flag を受け取り、制御点列を返す
  - clamped: 三重対角 Thomas
  - periodic: 循環三重対角 (Sherman-Morrison)
- [ ] 単体テスト: 点列を渡して戻り curve が点列を通ることを verify

### Phase B: 2D tensor 補間

- [ ] 2D 格子点を受け取って BSpline surface の制御点を返す
- [ ] 1D 補間を U 方向 → V 方向の 2 回適用するだけ (separability)

### Phase C: Skinning (curve → surface)

- [ ] $M$ 本の compatible curves と V-parameter 配列を受け取り、V 方向に skin した
      surface を返す
- [ ] 各 U 制御点インデックスに対して 1D 補間を呼ぶ

### Phase D: Gordon Boolean sum

- [ ] 上の 3 つを組み合わせ: skin profiles (Phase C), skin guides (Phase C),
      2D tensor (Phase B), pointwise subtract → sum → Geom_BSplineSurface

### Phase E: 入力互換化

- [ ] 任意の profile/guide BSpline を共通 knot vector・次数に unify する preprocess

### Phase F: OCCT への接続

- [ ] 結果 surface を `Geom_BSplineSurface` として返し、既存 face/cap/sew/solid パスに
      繋ぐ (cap 不要なら skip、torus なら直接 shell/solid)

### Phase G: 検証

- [ ] `examples/08_gordon_surface.rs` で volume 59.22 ± 0.5 (理論値) を達成
- [ ] profile 制御点と surface iso curve 制御点の一致を assert (1e-9 精度)

---

**この note を実装時のリファレンスとする**。OCCT の GordonBuilder / occ_gordon を
迂回して、cadrum 内部に自前の `gordon::build(profiles, guides) -> Geom_BSplineSurface`
モジュールを作る方針。
