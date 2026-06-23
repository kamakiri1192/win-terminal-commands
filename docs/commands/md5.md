# md5

ファイルや文字列・標準入力の MD5 チェックサムを計算・検証します（macOS / BSD の `md5` 互換）。MD5 の実装は純粋な Rust で書いており、外部 crate に依存しません。

## 使い方

```powershell
cargo build --release
.\target\release\md5.exe README.md            # MD5 (README.md) = <hash>
.\target\release\md5.exe -q README.md         # ハッシュだけ出力
.\target\release\md5.exe -s "abc"             # MD5 ("abc") = 900150983cd24fb0d6963f7d28e17f72
Write-Output "abc" | .\target\release\md5.exe # 標準入力をハッシュ（テキスト。末尾の改行も含まれます）
.\target\release\md5.exe file.zip             # バイナリはファイル指定が安全
.\target\release\md5.exe -c checksums.md5     # チェックサムを検証
```

PATH にインストールして直接 `md5` で呼ぶこともできます。

```powershell
cargo install --path . --force
md5 README.md
```

> [!NOTE]
> バイナリファイルのチェックサムは、ファイルを直接指定してください。PowerShell の `Get-Content` は既定でテキストとしてデコードするため、`Get-Content file.zip | md5.exe` のようにパイプで渡すとバイト列が変化してしまいます。標準入力でどうしても渡したい場合は `Get-Content -AsByteStream file.zip | md5.exe`（Windows PowerShell 5.1 では `-Encoding Byte`）を使います。

## オプション

- `-s string`, `--string string`: 指定した文字列のチェックサムを計算する
- `-c file`, `--check file`: チェックサムファイルを読み込んで検証する
- `-q`, `--quiet`: ハッシュだけを出力する（検証時は `OK` 行を抑制）
- `-r`, `--reverse`: ハッシュを先に出力する（`<hash>  <name>` 形式）
- `-p`, `--print`: 標準入力をそのまま標準出力へ書き出し、末尾にハッシュを追加する

短いオプションは結合できます（例: `md5 -qr file`）。

## 検証（`-c`）が対応する形式

- GNU / coreutils 形式: `<hash>  <name>`（`*` 始まりのバイナリ表記も可）
- BSD 形式: `MD5 (<name>) = <hash>`

不正な行が混ざっているチェックサムファイルは検証失敗（終了コード `1`）になります。

## 終了コード

- `0`: 成功（全ファイルをハッシュ、または検証がすべて一致）
- `1`: 読み込めないファイルがあった、不一致があった、チェックサムファイルに不正行があった
- `2`: 使い方エラー
