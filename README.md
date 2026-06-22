# win-terminal-commands

Linux でよく使うコマンドを Windows でも使いやすくするための Rust CLI 集です。

`which` と `open` を実装しています。

## Rust 環境

このリポジトリは Rust でビルドします。Windows で MSVC Build Tools が未導入の場合は、GNU toolchain を使うと軽く始められます。

```powershell
# rustup を入れた後、現在のシェルだけ PATH を反映する場合
$env:Path = "$env:USERPROFILE\.cargo\bin;$env:Path"

rustup toolchain install stable-x86_64-pc-windows-gnu --profile minimal
rustup default stable-x86_64-pc-windows-gnu
```

新しいターミナルを開くと、通常は `cargo` がそのまま使えます。

## which

`PATH` からコマンドの場所を探して表示します。Windows では `PATHEXT` も考慮するため、`which git` で `git.exe` などを見つけられます。

```powershell
cargo build --release
.\target\release\which.exe git
.\target\release\which.exe -a node
```

PATH から `which` として直接呼びたい場合は、Cargo の bin ディレクトリへインストールします。

```powershell
cargo install --path . --force
which git
```

### オプション

- `-a`, `--all`: 見つかった候補をすべて表示する
- `-s`, `--silent`: 結果を表示せず、終了コードだけ返す

終了コードは、すべて見つかった場合は `0`、1つでも見つからない場合は `1`、使い方が誤っている場合は `2` です。

## open

ファイル・フォルダ・URL を既定のアプリで開きます（macOS の `open` や Windows の `start` 相当）。

```powershell
cargo build --release
.\target\release\open.exe README.md          # 既定のアプリで開く
.\target\release\open.exe .                   # エクスプローラーで開く
.\target\release\open.exe https://example.com # 既定のブラウザで開く
.\target\release\open.exe                     # 引数なし = カレントディレクトリを開く
.\target\release\open.exe -a notepad note.txt # アプリを指定して開く
```

PATH にインストールして直接 `open` で呼ぶこともできます。

```powershell
cargo install --path . --force
open README.md
```

### オプション

- `-a app`, `--app app`, `--app=app`: 開くアプリを指定する（`PATH` と `PATHEXT` から解決）

終了コードは、すべて開けた場合は `0`、1つでも開けなかった場合は `1`、使い方が誤っている場合は `2` です。
