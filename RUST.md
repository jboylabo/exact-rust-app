# Exact アプリのデータを Rust で書く: 構築手順

`exact new` で作ったアプリ(データは `app.ts`)を、データを Rust のクレートで答える構成に切り替えた手順の記録です。このリポジトリは実際にこの手順で作りました。

> 2026-11-11 時点の exact2(commit `c111a3d`)で確認しました。Exact は 1.0 前なので、手順が変わることがあります。

## 結果

- `bun exact.mjs verify`(Web): テスト 3 本が通過し、指摘なし(clean)
- `cargo test -p exact-rust-app-data`: 2 本が通過
- `cargo check -p exact-rust-app-apple`: 通過(iOS シミュレータでの起動はまだ試していません)

## 構成

```
exact-pj/
├── exact/                  ← exact2 の checkout(パスで参照)
└── exact-rust-app/
    ├── app.contract        ← 画面。状態は Contract が持つ
    ├── app.test.contract   ← 画面のテスト
    ├── app.json            ← マニフェスト
    ├── Cargo.toml          ← ワークスペース(members = web, apple, data)
    ├── Cargo.lock
    ├── rust-toolchain.toml ← 生成物。編集しない
    ├── .cargo/config.toml  ← 生成物。編集しない
    ├── data/               ← ★ Rust のデータクレート(自分で書く)
    │   ├── Cargo.toml
    │   └── src/lib.rs
    ├── web/                ← Web 用のホストクレート(wasm)
    │   ├── Cargo.toml
    │   ├── build.rs
    │   └── src/lib.rs
    └── apple/              ← macOS / iOS 用のホストクレート
        ├── Cargo.toml
        ├── build.rs
        └── src/lib.rs
```

`app.ts` は使わなくなったので削除しました。

### 仕組み

1. `app.contract` の `resource face = faceFor(angel) as shape Face` が、Rust の `faceFor` を名前で呼びます。
2. `data/` の `Toggle` が `exact_runner::DataSource` を実装しており、`query` の中で `source` の名前ごとに答えます。
3. 返す値はレコードで、**shape で宣言した順に**フィールドを並べます(`Face` なら emoji, label の順)。
4. `web/build.rs` と `apple/build.rs` は、ビルド時に `app.contract` をコンパイルし、`contract::bake(plan, Toggle)` で最初の画面を焼き込みます。さらに `contract::rust_entry(...)` で、実行時のデータソースとして `Toggle` をつなぎます。

## 手順

`exact-rust-app/` の中で実行します。

### 1. 生成されたワークスペースを自分のものにする

ネイティブ用のコマンド(`bun exact.mjs ios` など)を一度実行すると、`.exact/shells/` が生成されます。それをアプリの直下にコピーします。

```sh
cp -R .exact/shells/. . && rm -rf .exact/shells .stamp.json
```

これでアプリ直下に `Cargo.toml`、`Cargo.lock`、`rust-toolchain.toml`、`web/`、`apple/` ができます。この時点ではまだ `app.ts` を使う構成です。

公式の手順は `../exact/docs/recipes/gpu-module.md`(`bun exact.mjs contract vocab gpu-module`)にあります。ただし GPU モジュール向けの説明で、データクレートを足す手順はドキュメントにありません。下の 2〜5 は、リポジトリ内のアプリを手本にしました。

### 2. データクレート `data/` を作る

`data/Cargo.toml`

```toml
[package]
name = "exact-rust-app-data"
version.workspace = true
edition.workspace = true
license.workspace = true
description = "Exact Rust App's data source, in Rust"
publish = false

[lib]
name = "exact_rust_app_data"
crate-type = ["rlib"]

[dependencies]
exact-plan = { path = "../../exact/plan" }
exact-runner = { path = "../../exact/runner" }
```

`data/src/lib.rs`

```rust
//! Exact Rust App's data source: the view asks `faceFor(angel)` and Rust
//! answers which face to show and what the button says.

#![deny(missing_docs)]

use exact_plan::Value;
use exact_runner::{DataError, DataSource};

/// The app's data source.
#[derive(Default, Clone, Copy)]
pub struct Toggle;

/// A `Face` record, its fields in the order `shape Face` declares them: emoji, label.
fn face(angel: bool) -> Value {
    if angel {
        Value::record(vec![Value::str("😇"), Value::str("太陽にする")])
    } else {
        Value::record(vec![Value::str("☀️"), Value::str("天使にする")])
    }
}

impl DataSource for Toggle {
    fn app_id(&self) -> &str {
        "com.example.exact-rust-app-2d7a17"
    }

    fn query(&mut self, source: &str, args: &[Value]) -> Result<Value, DataError> {
        match (source, args) {
            ("faceFor", [Value::Bool(angel)]) => Ok(face(*angel)),
            ("faceFor", _) => Err(DataError::Unavailable("faceFor takes one bool".into())),
            _ => Err(DataError::UnknownSource(source.into())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn angel_and_sun_alternate() {
        let mut data = Toggle;
        let angel = data.query("faceFor", &[Value::Bool(true)]).unwrap();
        let sun = data.query("faceFor", &[Value::Bool(false)]).unwrap();
        assert_eq!(angel, face(true));
        assert_eq!(sun, face(false));
        assert_ne!(angel, sun);
    }

    #[test]
    fn unknown_source_is_an_error() {
        assert!(Toggle.query("nope", &[]).is_err());
    }
}
```

- `app_id()` には `app.json` の `app.id` と同じ値を返します。アプリのストレージの名前になります。
- `query` はビルド時にも呼ばれ、最初の画面に焼き込まれます。
- shape のフィールドの順番は、`bun exact.mjs contract rust app.contract -o /tmp/shapes.rs` で生成される struct を見れば確認できます。

### 3. ワークスペースに `data` を足す

`Cargo.toml`

```toml
[workspace]
members = ["web", "apple", "data"]
resolver = "2"

[workspace.package]
edition = "2021"
version = "0.1.0"
license = "MIT"

# [patch.crates-io] は生成物。`bun exact.mjs update` が書くので手で直さない
```

### 4. Web のホストクレートを Rust データ用に書き換える

手本は `../exact/apps/geofence-fixture/web/` です。

`web/Cargo.toml`: `exact-js-web` と `exact-js-bake`(TypeScript 用)を外し、データクレートと `contract`、`exact-bake` を足します。

```toml
[package]
name = "exact-rust-app-web"
version.workspace = true
edition.workspace = true
license.workspace = true
publish = false
build = "build.rs"

[lib]
crate-type = ["cdylib", "rlib"]
test = false
bench = false

[dependencies]
exact-logic = { path = "../../exact/logic" }
exact-web = { path = "../../exact/host/web" }
exact-web-capabilities = { path = "../../exact/host/web-capabilities" }
exact-rust-app-data = { path = "../data" }

[build-dependencies]
contract = { path = "../../exact/contract/cli" }
exact-bake = { path = "../../exact/bake" }
exact-rust-app-data = { path = "../data" }
```

`web/build.rs`

```rust
//! Compile and bake `app.contract` with the Rust data source into `OUT_DIR/app.plan`.

use contract::DataSource;

fn main() {
    contract::rerun_if_changed(std::path::Path::new("../app.contract"));
    println!("cargo:rerun-if-changed=../app.json");
    println!("cargo:rerun-if-changed=../data");
    println!("cargo:rerun-if-changed=build.rs");
    let plan = match contract::compile_path(std::path::Path::new("../app.contract")) {
        Ok(p) => p,
        Err(e) => panic!("app.contract:{e}"),
    };
    let baked = contract::bake(plan, exact_rust_app_data::Toggle)
        .unwrap_or_else(|e| panic!("bake: {e:?}"));
    let out_dir = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap());
    std::fs::write(out_dir.join("app.plan"), baked.encode()).unwrap();
    let app_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let target = std::env::var("TARGET").unwrap_or_default();
    let manifest = contract::Manifest::read(&app_dir).unwrap_or_else(|e| panic!("app.json: {e}"));
    let grants = exact_rust_app_data::Toggle.grants();
    let compat = exact_bake::compatibility_id(&app_dir, "web", &target, &manifest, Some(grants))
        .unwrap_or_else(|e| panic!("compatibility id: {e}"));
    std::fs::write(out_dir.join("compat.json"), compat.to_json()).unwrap();
    let entry = contract::rust_entry(
        "exact_rust_app_data::Toggle",
        "exact_rust_app_data::Toggle",
        contract::web_rust_mode(&compat.inputs),
    )
    .unwrap();
    std::fs::write(
        out_dir.join("entry.rs"),
        format!(
            "{entry}\n{}exact_web::host!(AppData, PLAN, COMPAT, app_data);\n",
            contract::web_linked(&baked, &compat.inputs)
        ),
    )
    .unwrap();
}
```

`web/src/lib.rs`

```rust
//! Exact Rust App on the web: the Contract UI over the Rust data crate.

/// The baked plan, written by `build.rs`.
pub const PLAN: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/app.plan"));

/// The compatibility id and its inputs, written beside it.
pub const COMPAT: &str = include_str!(concat!(env!("OUT_DIR"), "/compat.json"));

include!(concat!(env!("OUT_DIR"), "/entry.rs"));
```

### 5. Apple のホストクレートを Rust データ用に書き換える

手本は `../exact/apps/hello/apple/` です。

`apple/Cargo.toml`: `exact-js` と `exact-js-bake` を外し、データクレートと `contract`、`exact-bake` を足します。

```toml
[package]
name = "exact-rust-app-apple"
version.workspace = true
edition.workspace = true
license.workspace = true
publish = false
build = "build.rs"

[lib]
crate-type = ["rlib"]
test = false
bench = false

[dependencies]
exact-logic = { path = "../../exact/logic" }
exact-apple = { path = "../../exact/host/apple" }
exact-apple-update = { path = "../../exact/host/apple-update" }
exact-rust-app-data = { path = "../data" }

[build-dependencies]
contract = { path = "../../exact/contract/cli" }
exact-bake = { path = "../../exact/bake" }
exact-rust-app-data = { path = "../data" }
```

`apple/build.rs`: `apps/hello/apple/build.rs` の `hello_data::Hello` を `exact_rust_app_data::Toggle` に置き換え、`../../../Cargo.lock` を参照している行を消しました。

```sh
sed -e 's/hello_data::Hello/exact_rust_app_data::Toggle/g' -e '/Cargo.lock/d' \
  ../exact/apps/hello/apple/build.rs > apple/build.rs
```

この build.rs は、`app.json` の `deploy.store.ios` が `"0"`(アップデート用のストアなし)なら `exact_apple` を、それ以外なら `exact_apple_update` をリンクします。

`apple/src/lib.rs`

```rust
//! Exact Rust App on Apple platforms: the Contract UI over the Rust data crate.

/// The baked plan, written by `build.rs`.
pub const PLAN: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/app.plan"));

/// The compatibility id and its inputs, written beside it.
pub const COMPAT: &str = include_str!(concat!(env!("OUT_DIR"), "/compat.json"));

include!(concat!(env!("OUT_DIR"), "/entry.rs"));
```

### 6. 画面とテスト

`app.contract`

```contract
shape Face
  emoji: string
  label: string

routes nav
  home "/"

component ExactRustApp
  state angel = true
  resource face = faceFor(angel) as shape Face
  state liked = false
  action toggle
    angel = not angel
  action like
    liked = not liked
  view
    main testId="root" navigationKey=`${top(nav).id}` navigationBack="back" width="100%" height="100%"
      for e in stack(nav) key=e.id
        column navigationKey=`${e.id}` navigationScroll="content" position="absolute" inset=0 display="flex" flex-direction="column"
          header display="flex" align-items="center"
            text "Exact Rust App" role="heading" aria-level=1 flex=1
            button press=like aria-label=(liked ? "Unlike" : "Like") testId="like"
              image (liked ? "symbol:heart-fill" : "symbol:heart")
          scroll id="content" flex=1 min-height=0 padding=24
            column gap=16 align-items="center"
              text face.emoji font-size="6rem" testId="face"
              button press=toggle testId="toggle"
                text face.label testId="toggle-label"
```

`font-size=96` のように px で固定すると、`verify` に「文字サイズの設定(Dynamic Type)を大きくしても変わらない」と指摘されます。`"6rem"` にすると設定に合わせて大きくなります。

`app.test.contract`

```contract
test "Rust answers the angel first"
  expect tree has "root"
  expect text "face" == "😇"
  expect text "toggle-label" == "太陽にする"

test "the button flips angel and sun"
  tap "toggle"
  expect state angel == false
  expect text "face" == "☀️"
  expect text "toggle-label" == "天使にする"
  tap "toggle"
  expect text "face" == "😇"

test "the header's button likes it"
  tap "like"
  expect state liked == true
```

### 7. 不要になった `app.ts` を消し、パスと lock を整える

```sh
git rm app.ts
bun exact.mjs update   # パッチ・ツールチェーン・パスを整える
bun exact.mjs lock     # Cargo.lock をマニフェストに合わせる
```

### 8. 確認する

```sh
cargo test -p exact-rust-app-data    # Rust 側のテスト
cargo check -p exact-rust-app-apple  # Apple ホストのコンパイル確認
bun exact.mjs verify                 # Web: ビルド、テスト、スクリーンショット、検査
bun exact.mjs web                    # ブラウザで開く
bun exact.mjs ios --run              # iOS シミュレータ(初回 10〜15 分。まだ試していない)
```

## つまずいたところ

| 症状 | 対処 |
|---|---|
| `exact new` で作ったアプリにデータクレートを足す手順がドキュメントにない | `apps/geofence-fixture/web` と `apps/hello/apple` を手本にして、`build.rs` を `contract::bake` と `rust_entry` を使う形に書き換えた |
| コピー直後の `Cargo.toml` のパスが `.exact/shells/` からの相対パスのまま | `bun exact.mjs update` で直る(`../exact/...` に揃う) |
| `update` のあとに `Cargo.lock does not match` と出る | `bun exact.mjs lock` |
| 初回の `verify` で `patch ... was not used in the crate graph` という警告が 4 件出る | Web ビルドで使わないパッチがあるだけ。`[patch.crates-io]` は生成物なので手で直さない。2 回目以降は出なかった |
| `font-size=96` が `verify` で指摘される | `font-size="6rem"` にする |

## データの書き方をもっと知るには

- `../exact/docs/reference.md` の「Rust data sources」節: `query` / `answer` / `parse` / `app_id` / `grants`、ストレージ(SQLite・ファイル)の使い方
- `../exact/apps/caltrain/data/src/lib.rs`、`../exact/apps/fieldnotes/data/src/lib.rs`: 実際のアプリのデータクレート
