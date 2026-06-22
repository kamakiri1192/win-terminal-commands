# win-terminal-commands

macOS、Linux でよく使うコマンドを Windows でも使いやすくするための Rust CLI 集です。  
Windows公式で配布されているCoreutilsでカバーされないコマンドを実装して、普段のmacOSでの開発に近い感覚でCLI操作ができるようになることを目指します。

`which`、`open`、`md5` を実装しています。


### Tips: Windows公式のUNIX スタイルのコマンドライン ユーティリティのセット
Coreutilsはlsやtailなどを使用できるようになるWindows公式ユーリティです。  
まずは、こちらもインストールすることを推奨します。

**Coreutils for Windows**
- https://learn.microsoft.com/ja-jp/windows/core-utils/overview
- https://github.com/microsoft/coreutils

```
winget install Microsoft.Coreutils
```

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

## md5

ファイルや文字列・標準入力の MD5 チェックサムを計算・検証します（macOS / BSD の `md5` 互換）。MD5 の実装は純粋な Rust で書いており、外部 crate に依存しません。

```powershell
cargo build --release
.\target\release\md5.exe README.md            # MD5 (README.md) = <hash>
.\target\release\md5.exe -q README.md         # ハッシュだけ出力
.\target\release\md5.exe -s "abc"             # MD5 ("abc") = 900150983cd24fb0d6963f7d28e17f72
Write-Output "abc" | .\target\release\md5.exe # 標準入力をハッシュ（テキスト。末尾の改行も含まれます）
.\target\release\md5.exe file.zip             # バイナリはファイル指定が安全
.\target\release\md5.exe -c checksums.md5     # チェックサムを検証
```

> [!NOTE]
> バイナリファイルのチェックサムは、ファイルを直接指定してください。PowerShell の `Get-Content` は既定でテキストとしてデコードするため、`Get-Content file.zip | md5.exe` のようにパイプで渡すとバイト列が変化してしまいます。標準入力でどうしても渡したい場合は `Get-Content -AsByteStream file.zip | md5.exe`（Windows PowerShell 5.1 では `-Encoding Byte`）を使います。

PATH にインストールして直接 `md5` で呼ぶこともできます。

```powershell
cargo install --path . --force
md5 README.md
```

### オプション

- `-s string`, `--string string`: 指定した文字列のチェックサムを計算する
- `-c file`, `--check file`: チェックサムファイルを読み込んで検証する
- `-q`, `--quiet`: ハッシュだけを出力する（検証時は `OK` 行を抑制）
- `-r`, `--reverse`: ハッシュを先に出力する（`<hash>  <name>` 形式）
- `-p`, `--print`: 標準入力をそのまま標準出力へ書き出し、末尾にハッシュを追加する

検証（`-c`）は次の両方の形式に対応します。

- GNU / coreutils 形式: `<hash>  <name>`（`*` 始まりのバイナリ表記も可）
- BSD 形式: `MD5 (<name>) = <hash>`

終了コードは、成功時は `0`、読み込めないファイルや不一致があった場合は `1`、使い方が誤っている場合は `2` です。
