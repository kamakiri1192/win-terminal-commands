# win-terminal-commands

macOS、Linux でよく使うコマンドを Windows でも使いやすくするための Rust CLI 集です。  
Windows公式で配布されているCoreutilsでカバーされないコマンドを実装して、普段のmacOSでの開発に近い感覚でCLI操作ができるようになることを目指します。

`which`, `open`, `gzip` / `gunzip` を実装しています。


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

## gzip / gunzip

ファイルや標準入力を gzip 形式（RFC 1952）で圧縮・展開します。Windows に標準で gzip が無いため用意しました。標準の `gunzip`、`.NET` の `GZipStream`、`tar -z` などと互換性のあるアーカイブを出力します（相互に読み書きできることを確認済みです）。

```powershell
cargo build --release
.\target\release\gzip.exe file.txt        # file.txt.gz を作り、file.txt を削除
.\target\release\gzip.exe -k file.txt     # 元のファイルを残す
.\target\release\gzip.exe -c file.txt > out.gz   # 標準出力へ（元は残す）
Get-Content file.txt | .\target\release\gzip.exe > file.txt.gz   # 標準入力 → 標準出力

.\target\release\gunzip.exe file.txt.gz   # file.txt に展開
.\target\release\gzip.exe -d file.txt.gz  # gzip -d でも展開
.\target\release\gzip.exe -t file.txt.gz  # 整合性テスト
.\target\release\gzip.exe -9 file.txt     # 圧縮レベル 1〜9（既定は 6）
```

PATH にインストールして直接 `gzip` / `gunzip` で呼ぶこともできます。

```powershell
cargo install --path . --force
gzip file.txt
gunzip file.txt.gz
```

### オプション

- `-c`, `--stdout`: 標準出力へ書き出し、元のファイルを残す
- `-d`, `--decompress`: 展開する（`gunzip` は既定で展開）
- `-f`, `--force`: 出力ファイルがあっても上書きする（未知の拡張子も展開）
- `-k`, `--keep`: 元のファイルを残す
- `-t`, `--test`: アーカイブの整合性をテストする
- `-1`〜`-9`: 圧縮レベル（`--fast`=`-1`、`--best`=`-9`、既定は `-6`）
- `-h`, `--help`: ヘルプを表示
- `-V`, `--version`: バージョンを表示

引数なし（または `-`）のときは標準入力を圧縮して標準出力へ出します。展開時は `.gz` / `.z` を取り除き、`.tgz` / `.taz` は `.tar` に展開します。未知の拡張子は `-f` を付けない限りスキップします。終了コードは、成功時 `0`、一部でも失敗があれば `1`、使い方が誤っている場合は `2` です。

> 注: GNU gzip の `-l`（内容一覧）、`-r`（再帰）、`-S`（サフィックス指定）、`-n`/`-N`（名前の扱い）など、日常的に使わないオプションは実装していません。
