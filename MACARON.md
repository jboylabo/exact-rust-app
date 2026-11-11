# 紫のマカロンを GPU で描く: 作り方と仕組み

Exact アプリの画面に、Rust とシェーダーで描いた立体の紫のマカロンを置いた記録です。画像ファイルは使わず、形も色も光もすべて計算で出しています。

> 2026-11-11 時点の exact2(commit `c111a3d`)で確認しました。前提として、`RUST.md` の手順 1(ワークスペースをアプリ直下にコピーする)が済んでいる必要があります。

## 結果

- `bun exact.mjs verify ios`(iOS シミュレータ): テスト 3 本が通過し、指摘なし
- `bun exact.mjs verify`(Web): テスト 3 本が通過。画面の検査も指摘なし(Cargo の `patch ... was not used` という警告 3 件だけが problems に数えられます)
- 回転と上下の動きは、スクリーンショットでは確かめられていません。`verify` は時計を止めて撮るためです。動きは `bun exact.mjs ios --run` で見られます

## 全体の仕組み

```
app.contract                 gpu/src/lib.rs              gpu/src/macaron.rs           gpu/shaders/macaron.wgsl
canvas surface=macaron() ──▶ REGISTRY に "macaron" ──▶  MacaronSurface::render ──▶  画面の 1 ピクセルごとに
                             という名前で登録            毎フレーム、時刻を角度に      マカロンの色を計算
                                                         直してシェーダーに渡す
```

1. **Contract**: `canvas surface=macaron()` と書くと、その canvas を「macaron という名前の描画係(surface)」に任せることになります。
2. **GPU モジュール(`gpu/` クレート)**: アプリ専用の Rust のクレートです。`REGISTRY` に surface の名前・引数の数・作り方を登録し、`exact_gpu::module!(REGISTRY)` で公開します。最初の画面が出たあとに読み込まれます。
3. **wgpu**: Rust の描画ライブラリです。同じコードが、Web では WebGPU、iOS と macOS では Metal で動きます。そのため「WebGPU で作ったマカロン」がそのまま iPhone でも描けます。
4. **シェーダー(WGSL)**: GPU 上で動く小さなプログラムです。canvas の全ピクセルについて「このピクセルは何色か」を計算します。

## ファイル構成

```
exact-rust-app/
├── app.contract              ← canvas を 1 行置く
├── Cargo.toml                ← members に "gpu" を足す
└── gpu/
    ├── Cargo.toml
    ├── build.rs              ← シェーダーをビルド時に検査し、Rust の型を生成する
    ├── shaders/
    │   └── macaron.wgsl      ← マカロンの形・色・光(本体)
    └── src/
        ├── lib.rs            ← surface の登録
        └── macaron.rs        ← 毎フレームの描画
```

## 作り方

### 1. `gpu/Cargo.toml`

```toml
[package]
name = "exact-rust-app-gpu"
version.workspace = true
edition.workspace = true
license.workspace = true
description = "Exact Rust App's GPU module: a purple macaron"
publish = false
build = "build.rs"

[lib]
name = "exact_rust_app_gpu"
crate-type = ["cdylib", "rlib"]
test = false
bench = false

[dependencies]
exact-gpu = { path = "../../exact/gpu" }

[target.'cfg(target_arch = "wasm32")'.dependencies]
wasm-bindgen = "0.2"
wasm-bindgen-futures = "0.4"
web-sys = { version = "0.3", features = ["HtmlCanvasElement"] }

[build-dependencies]
exact-gpu-reflect = { path = "../../exact/gpu/reflect" }
```

- パッケージ名は「アプリ名 + `-gpu`」にします。
- `wasm32` 向けの 3 つの依存は、Web 版で `module!` が公開に使うので必須です。

### 2. `gpu/build.rs`

`../exact/apps/caltrain/gpu/build.rs` をそのままコピーしました。

```sh
cp ../exact/apps/caltrain/gpu/build.rs gpu/build.rs
```

この build.rs は、ビルド時に `shaders/` の WGSL を検査します。シェーダーに間違いがあると、実行時に canvas が真っ黒になるのではなく、ビルドが行番号つきのエラーで止まります。さらに、シェーダーが宣言している内容から Rust のコードを生成します(`Uniforms` 構造体、バインドの配置 `GROUP_0`、関数名 `entry::VS` / `entry::FS` など)。Rust 側で同じ内容を手で書き写す必要はありません。

### 3. `gpu/src/lib.rs`: surface を登録する

```rust
pub mod macaron;
pub use macaron::MacaronSurface;

use exact_gpu::{Registry, Surface};

pub mod shaders {
    include!(concat!(env!("OUT_DIR"), "/shaders.rs"));   // build.rs が生成したもの
}

fn macaron() -> Box<dyn Surface> {
    Box::new(MacaronSurface::new())
}

/// 名前、引数の数、作り方
pub static REGISTRY: Registry = Registry {
    surfaces: &[("macaron", 0, macaron)],
    shaders: shaders::SHADERS,
};

exact_gpu::module!(REGISTRY);
```

`("macaron", 0, macaron)` の `0` は引数の数です。Contract 側の `macaron()` と合わせます。

### 4. `gpu/src/macaron.rs`: 毎フレーム描く

`exact_gpu::Surface` トレイトを実装します。大事なのは 2 つのメソッドです。

- `bind`: Contract から渡された引数を受け取ります。マカロンは引数なしなので、何か渡されたらエラーにします。
- `render`: 1 フレームを描きます。戻り値が `true` なら「次のフレームも描いてほしい」という意味で、マカロンは回り続けるので常に `true` を返します。

`render` の流れ:

1. 初回(または描画先の形式が変わったとき)に、シェーダーと描画パイプラインを組み立てます。
2. 今の時刻から回転の角度と上下の位相を計算し、canvas の大きさと一緒に `Uniforms` としてシェーダーへ渡します。
3. 画面全体を覆う三角形を 1 枚描きます。色を決めるのはシェーダーです。

```rust
const TURN_SECONDS: f64 = 9.0;   // 1 回転にかかる秒数
const BOB_SECONDS: f64 = 3.6;    // 上下 1 往復にかかる秒数

pub fn phase(now_ms: f64, seconds: f64) -> f32 {
    let turns = (now_ms / 1000.0 / seconds).rem_euclid(1.0);
    (turns * std::f64::consts::TAU) as f32
}
```

角度への変換を Rust 側(f64)でやっているのは、精度のためです。シェーダーが使える数は f32 で、起動からの経過ミリ秒をそのまま渡すと、長時間動かしたときに細かい値が失われて動きがかくつきます。0〜2π に丸めてから渡せば、いつまでも滑らかです。

時刻には `frame.now_ms`(ホストの時計)を使っています。テストや `verify` が時計を止めたり進めたりしても、そのとおりの絵になります。

### 5. `gpu/shaders/macaron.wgsl`: マカロンの本体

下の「シェーダーの仕組み」で説明します。

### 6. ワークスペースと画面に足す

`Cargo.toml`

```toml
[workspace]
members = ["web", "apple", "data", "gpu"]
```

`app.contract`

```contract
column width="100%" border-radius=24 overflow="hidden"
  canvas surface=macaron() width="100%" height=320 aria-label="紫のマカロン" testId="macaron"
```

canvas に直接 `border-radius` を付けても角は丸くなりませんでした。`overflow="hidden"` の `column` で包むと丸くなります。

### 7. 整えて確認する

```sh
bun exact.mjs update                 # パスとパッチを整える
bun exact.mjs lock                   # Cargo.lock を合わせる
cargo check -p exact-rust-app-gpu    # シェーダーの検査を含むコンパイル確認
bun exact.mjs verify                 # Web(速いので、見た目の調整はこちらで)
bun exact.mjs verify ios             # iOS シミュレータ
bun exact.mjs ios --run              # シミュレータで実際に動かして見る
```

`lock` が「GPU モジュールが別のバージョンの wasm-bindgen をリンクしている」と言った場合は、表示された `=` 付きのバージョンを `gpu/Cargo.toml` に書いて、もう一度 `lock` します(今回は出ませんでした)。

## シェーダーの仕組み

### 考え方: ポリゴンを使わず、距離の関数で形を作る

普通の 3D は三角形の集まり(ポリゴン)で形を作りますが、ここでは使っていません。代わりに「空間のある点から、マカロンの表面までの距離」を返す関数(`map`)を 1 つ書いています。これを SDF(符号付き距離関数)と呼びます。

描くときは、ピクセルごとに次のことをします(レイマーチング)。

1. カメラからそのピクセルの方向へ、光線を 1 本飛ばします。
2. 今いる点で `map` を呼び、表面までの距離を聞きます。その距離ぶんだけ進みます(それより手前に表面はないので、安全に進めます)。
3. 距離がほぼ 0 になったら、表面に当たったとみなします。最大 72 回くり返し、当たらなければ背景を描きます。

```wgsl
for (var i = 0; i < 72; i = i + 1) {
    hit = map(eye + ray * t);
    if (hit.x < 0.0012) { found = true; break; }
    t = t + hit.x * 0.85;
    if (t > 6.0) { break; }
}
```

### マカロンの形

基本の部品は「角を丸めた円柱」(`round_cylinder`)です。これを組み合わせます。

| 部分 | 作り方 |
|---|---|
| 生地(上) | 角を丸めた円柱に、つぶした球を滑らかにつないで、真ん中をふっくら盛り上げる |
| ピエ(生地のふちのぎざぎざ) | 生地の下のふちに輪(トーラス)を置き、ノイズで表面をでこぼこにする |
| 生地(下) | 上の生地を上下反転して使う(`shell(vec3(q.x, -q.y, q.z))`) |
| クリーム | 生地より少し小さい、低い円柱。円周に沿ってゆるく波打たせる |

部品どうしをつなぐときは `smin`(滑らかな最小値)を使います。ただの `min` だとつなぎ目に角ができますが、`smin` だと粘土をなでたように滑らかにつながります。

`map` は距離と一緒に「当たったのは生地(0)かクリーム(1)か」も返します。色を塗り分けるためです。

### 動き

マカロン自体は回転対称なので、まっすぐ立てて回しても動いて見えません。そこで少し傾けて(約 24 度)から回しています。コインが揺れながら回るような動きになり、ピエのでこぼこも一緒に回るので、回転がはっきり見えます。

```wgsl
fn to_macaron(p: vec3<f32>) -> vec3<f32> {
    var q = p - vec3<f32>(0.0, 0.05 * sin(u.bob), 0.0);  // ゆっくり上下
    q = rot_y(q, -u.spin);                               // 縦軸まわりに回る
    q = rot_x(q, -0.42);                                 // 傾ける
    q = rot_y(q, -u.spin * 0.5);                         // 自分の軸でも回る
    return q;
}
```

マカロンを動かす代わりに、調べる点のほうを逆向きに動かしているので、角度にマイナスが付いています。

### 色と光

当たった点で、表面の向き(法線)を求めます。`map` を少しずつずらして 6 回呼び、距離の変化から向きを計算します。その向きを使って、次のものを足し合わせます。

- **地の色**: 生地は紫 `(0.64, 0.40, 0.88)`、クリームは白に近い薄紫 `(0.95, 0.89, 0.985)`。生地にはノイズで少し色むらを付けて、焼き菓子のつや消しの質感にしています。
- **拡散光**: 右上手前からの光。影の側も真っ暗にならないよう、光を回り込ませています。
- **環境光**: 上を向いた面は明るく、下を向いた面は暗い紫に。
- **くぼみの影(アンビエントオクルージョン)**: 生地とクリームのすき間など、奥まった所を暗くします。表面から少し離れた点で `map` を呼び、「近くに別の面があるか」を調べています。
- **つや**: 光の反射。クリームは生地より少し強めです。
- **縁の光**: 輪郭に近い所をうっすら明るくして、立体感を出します。

### 背景

薄いラベンダー色の縦のグラデーションです。マカロンの下には、楕円形にぼかした紫の影を置いて、台に乗っているように見せています。背景は不透明で、ダークモードでも同じ色のままです。

## 見た目を調整するときの場所

すべて `gpu/shaders/macaron.wgsl` と `gpu/src/macaron.rs` の中です。

| 変えたいこと | 場所 |
|---|---|
| 生地の色 | `fs` の `var albedo = vec3<f32>(0.640, 0.400, 0.880)` |
| クリームの色 | `fs` の `albedo = vec3<f32>(0.950, 0.890, 0.985)` |
| 大きさ(寄り・引き) | `fs` の `forward * 1.45`(大きくすると寄る)と `eye` の位置 |
| 傾き | `to_macaron` の `rot_x(q, -0.42)` |
| 回る速さ | `macaron.rs` の `TURN_SECONDS` |
| 上下の速さと幅 | `macaron.rs` の `BOB_SECONDS`、`to_macaron` の `0.05` |
| ピエのでこぼこ | `shell` の `ruffle` |
| クリームの厚み | `map` の `round_cylinder(q, 0.935 + bulge, 0.15, 0.10)` と、`shell` の生地の高さ `0.32` |
| 背景の色 | `background` の `top` と `low` |

## 調整の経過

| 段階 | 見た目 | 直したこと |
|---|---|---|
| 1 回目 | 寄りすぎて生地の一部しか映らない。色が明るすぎて白飛び | カメラの画角を広げ(`2.6` → `1.45`)、光を弱めた |
| 2 回目 | 形は良いが、クリームがほとんど見えず全体に暗い | 生地の間を広げてクリームを厚くし、少し明るくした |
| 3 回目 | 生地・ピエ・クリームが見える紫のマカロンに | iOS で確認 |

Web の `verify` は 10 秒ほどで終わりスクリーンショットも残るので、見た目の調整は Web でくり返し、最後に iOS で確かめるのが速いです。

## 参考

- `../exact/docs/recipes/gpu-module.md`(`bun exact.mjs contract vocab gpu-module`): GPU モジュールの公式手順
- `../exact/apps/caltrain/gpu/`: シェーダーを 3 つ持つ実例。`aurora.rs` が今回の手本
- `../exact/gpu/src/lib.rs`: `Surface` トレイトと `Frame` の定義
