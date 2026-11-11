# Exact を Rust で動かす

Expo の新しいクロスプラットフォーム・ランタイム [Exact](https://github.com/expo/exact) を、環境構築から Rust のデータソースまで試した記録です。

> Exact は 1.0 前で、毎日変わります。この内容は 2026-11 時点のものです。

## Exact とは

- UI は **Contract** という小さな宣言的言語で書く(`app.contract`)
- データとロジックは **TypeScript**(`app.ts`)か **Rust**(データクレート)で書く
- 同じアプリが Web / macOS / iOS / Linux で動く
- Web は本物の DOM に描画し、ネイティブは Rust のランナーとレイアウトカーネルが同じ plan を実行する

## 環境構築で詰まったところ

| 症状 | 原因と対処 |
|---|---|
| `bun install` だけでは何も作れない | 配布用の CLI はない。リポジトリを `git clone` して、その中のスクリプトを使う |
| `error: Module not found "scripts/exact.mjs"` | clone した `exact/` の外で実行していた。`exact/` の中に `cd` してから実行する |
| セットアップが遅い | 遅いのは `bun install` ではなく Rust のビルド。`setup` と初回ビルドで数分〜十数分かかる |
| `iOS SDK: 27.0 (SDK 27.1 or later)` | Xcode が古い。Xcode 27.1 以降に更新して `sudo xcode-select -s /Applications/Xcode.app` |
| `ios --run` が止まったように見える | アプリを起動したあともログを待ち受けているだけ。シミュレータでアプリが動いていれば正常 |

## 1. 必要なツール

- **Rust**: [rustup](https://rustup.rs) で入れる。固定バージョンは初回の Cargo 実行時に自動で入る
- **Bun 1.4.2**: `package.json` が固定しているバージョン。Node と npm は不要
- **Google Chrome**: エージェントが Web を操作するのに使う
- **Xcode 27.1 以降**: macOS / iOS 向けにビルドする場合

```sh
# 既存の Bun と並べて 1.4.2 を入れる場合
curl -fsSL https://bun.sh/install | BUN_INSTALL=~/.bun-1.4.2 bash -s bun-v1.4.2
```

## 2. Exact を clone してセットアップ

```sh
mkdir exact-pj && cd exact-pj
git clone https://github.com/expo/exact.git
cd exact
bun scripts/exact.mjs setup --check   # 足りないものを表示するだけ(何も変更しない)
bun scripts/exact.mjs setup           # 重い。時間のあるときに
```

`setup` は Rust のツールチェーン、wasm-bindgen、Binaryen、Bun の依存、Hermes と iOS のバンドル、クレート一式を入れます。

## 3. 新規プロジェクトを作る

**`exact/` の中で**実行します。`../` を付けると、`exact/` と同じ階層に作られます。

```sh
# exact-pj/exact にいる状態で
bun scripts/exact.mjs new ../my-app
cd ../my-app
```

```
exact-pj/
├── exact/    ← clone したもの(消したり移動したりしない)
└── my-app/   ← 新しいアプリ。exact/ をパスで参照する
```

表示名とバンドル ID も指定できます。

```sh
bun scripts/exact.mjs new ../my-app --name "My App" --bundle-id com.example.myapp
```

生成されるファイル:

| ファイル | 役割 |
|---|---|
| `app.contract` | 画面(ビュー) |
| `app.ts` | データ(TypeScript) |
| `app.json` | マニフェスト(名前、バンドル ID、対象プラットフォーム) |

## 4. ビルドと起動

`my-app/` の中で実行します。

```sh
bun exact.mjs web                 # http://127.0.0.1:8765/ 保存すると即反映
bun exact.mjs ios --run           # iOS シミュレータ
bun exact.mjs mac --run           # この Mac
```

- 初回のネイティブビルドは 10〜15 分、2 回目以降は 1〜2 分
- シミュレータの機種を指定するには `EXACT_SIM="iPhone 17 Pro" bun exact.mjs ios --run`

## 5. UI と機能を追加する

画面は `app.contract` に書きます。`state` が状態、`action` が操作、`view` が見た目です。

```contract
component App
  state count = 0
  action increment
    count = count + 1
  view
    column padding=24 gap=16
      text `${count}` font-size=48 testId="count"
      button press=increment testId="plus"
        text "+1"
```

`bun exact.mjs web` を起動したまま保存すると、ブラウザに約 0.1 秒で反映されます。

書き方を調べるには次のコマンドを使います。

```sh
bun exact.mjs pattern                          # 雛形の一覧(one-screen, list-detail, tabs など)
bun exact.mjs contract vocab button            # タグや機能の説明
bun exact.mjs contract build app.contract      # コンパイルしてエラーを確認
```

## 6. Rust でボタンを作る: 😇 ⇄ ☀️

ボタンを押すたびに 😇 と ☀️ が切り替わるアプリを、データ部分を Rust で書いて作ります。

### 仕組み

- Contract の `resource` が Rust の関数を名前で呼ぶ
- Rust 側は `exact_runner::DataSource` トレイトを実装し、`match source { "名前" => ... }` で応答する
- 返す値はレコードで、**shape のフィールドの宣言順**に値を並べる

### Contract(`app.contract`)

状態は Contract が持ち、表示する絵文字とラベルは Rust が決めます。

```contract
shape Face
  emoji: string
  label: string

component App
  state angel = true
  resource face = faceFor(angel) as shape Face
  action toggle
    angel = not angel
  view
    column padding=24 gap=16
      text face.emoji font-size=96 testId="face"
      button press=toggle testId="toggle"
        text face.label
```

`angel` が変わると `faceFor(angel)` がもう一度呼ばれ、画面が更新されます。

### Rust(`data/src/lib.rs`)

```rust
use exact_plan::Value;
use exact_runner::{DataError, DataSource};

#[derive(Default, Clone, Copy)]
pub struct Toggle;

fn face(angel: bool) -> Value {
    // shape Face の宣言順: emoji, label
    if angel {
        Value::record(vec![Value::str("😇"), Value::str("太陽にする")])
    } else {
        Value::record(vec![Value::str("☀️"), Value::str("天使にする")])
    }
}

impl DataSource for Toggle {
    fn query(&mut self, source: &str, args: &[Value]) -> Result<Value, DataError> {
        match source {
            "faceFor" => match args {
                [Value::Bool(angel)] => Ok(face(*angel)),
                _ => Err(DataError::Unavailable("faceFor は bool を 1 つ取る".into())),
            },
            _ => Err(DataError::UnknownSource(source.into())),
        }
    }
}
```

shape から Rust の struct を生成して、フィールドの順番を確認できます。

```sh
bun exact.mjs contract rust app.contract -o /tmp/shapes.rs
```

### `data/Cargo.toml`

```toml
[package]
name = "my-app-data"
version.workspace = true
edition.workspace = true
license.workspace = true
publish = false

[lib]
name = "my_app_data"
crate-type = ["rlib"]

[dependencies]
exact-plan = { path = "<exact へのパス>/plan" }
exact-runner = { path = "<exact へのパス>/runner" }
```

### アプリに Rust クレートをつなぐ

> このリポジトリで実装済みです(`data/`、`web/`、`apple/`)。`bun exact.mjs verify` は clean、`cargo test -p exact-rust-app-data` も通ります。`web/build.rs` は `exact/apps/geofence-fixture/web/build.rs` を、`apple/build.rs` は `exact/apps/hello/apple/build.rs` を手本にしました。`app.ts` は不要になったので削除しています。

`exact new` で作ったアプリには Rust のクレートがありません(ビルド用のクレートは `.exact/shells/` に自動生成されます)。自分の Rust を足すには、そのワークスペースをアプリ直下にコピーして自分のものにします。この手順はリポジトリの `docs/recipes/gpu-module.md` が GPU モジュール向けに書いているもので、データクレートでも同じ方式です。

```sh
bun exact.mjs ios                    # 一度実行すると .exact/shells/ が生成される
cp -R .exact/shells/. . && rm -rf .exact/shells .stamp.json
# data/ を作り、Cargo.toml の members に "data" を追加
bun exact.mjs update
bun exact.mjs lock
```

そのあと、ホスト側のクレート(`apple/` と `web/`)の `build.rs` を、TypeScript ではなく自分のデータクレートを使うように書き換えます。Rust だけで書かれた最小アプリ `exact/apps/hello` が手本です。

- `apps/hello/apple/build.rs`: `contract::bake(plan, hello_data::Hello)` と `contract::rust_entry("hello_data::Hello", ...)` で、データクレートを指定している
- `apps/hello/data/src/lib.rs`: 何も答えない最小の `DataSource`

### 発展: 状態も Rust に持たせる(要確認)

`mutation` を使うと、Rust 側の構造体に状態を持たせて `answer` で切り替えられます。

```contract
component App
  resource first = currentFace() as shape Face
  mutation flipped as shape Face
  derive face = match flipped { case some(f) => f, case none => first }
  action toggle
    send flipped = flip()
  view
    column padding=24 gap=16
      text face.emoji font-size=96 testId="face"
      button press=toggle testId="toggle"
        text face.label
```

```rust
use exact_runner::{Answer, Store};

pub struct Toggle { angel: bool }
impl Default for Toggle { fn default() -> Self { Self { angel: true } } }

impl DataSource for Toggle {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        match source {
            "currentFace" => Ok(face(self.angel)),
            _ => Err(DataError::UnknownSource(source.into())),
        }
    }
    fn answer(&mut self, _: &mut Store, source: &str, args: &[Value]) -> Result<Answer, DataError> {
        match source {
            "flip" => {
                self.angel = !self.angel;
                Ok(Answer::Now(face(self.angel)))
            }
            _ => self.query(source, args).map(Answer::Now),
        }
    }
}
```

`query` はビルド時にも呼ばれ、最初の画面に焼き込まれます。`answer` は実行時に呼ばれます。

## 参考

- [expo/exact](https://github.com/expo/exact)
- `docs/start-here.md`: アプリ作成の入門(必読とされているガイド)
- `docs/reference.md` の「Rust data sources」節: `query` / `answer` / `parse` / `app_id` / `grants`
- `apps/hello`(Rust だけの最小アプリ)、`apps/caltrain/data`(Rust データの実例)

Rust の章について補足です。

- **パターンの選び方**: Contract に状態を持たせる前半の方式の方が素直で、まず試すならこちらです。後半の mutation 版は、Rust 側の状態がビルド時と実行時でどう扱われるかを確認できていません。
- **絵文字の警告**: Exact のガイドは、アイコンに絵文字ではなくシンボルを使うよう勧めています。ビルドで警告が出るかもしれませんが、デモとしては問題ないはずです。
- **確認が必要な箇所**: 今回 `apps/toggle-check` という検証用アプリで上の 2 パターンを試していましたが、ビルド途中で止めたので結果は出ていません。手元の `exact/apps/hello` で `data/src/lib.rs` と `app.contract` を差し替えて `bun scripts/exact.mjs` から動かすのが、一番早く確かめられる方法だと思います。
