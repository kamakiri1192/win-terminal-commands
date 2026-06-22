# win-terminal-commands

macOS、Linux でよく使うコマンドを Windows でも使いやすくするための Rust CLI 集です。  
Windows公式で配布されているCoreutilsでカバーされないコマンドを実装して、普段のmacOSでの開発に近い感覚でCLI操作ができるようになることを目指します。

`which` と `open`、`say` を実装しています。


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

## say

任意のテキストを読み上げます（macOS の `say` 相当）。Windows Runtime の `Windows.Media.SpeechSynthesis` を使うため、Windows 10/11 のモダンな（OneCore 系）音声が使われます。自然（ニューラル）音声をインストールしていれば、それも `-v` で選択できます。

```powershell
cargo build --release
.\target\release\say.exe "ビルドが終わりました"
echo "こんにちは" | say                        # 標準入力を読み上げる
.\target\release\say.exe -v Haruka "こんにちは"  # 音声を指定
.\target\release\say.exe --list-voices          # 利用可能な音声を表示
.\target\release\say.exe -o out.wav "保存します" # 再生せず WAV に書き出す
```

PATH にインストールして直接 `say` で呼ぶこともできます。

```powershell
cargo install --path . --force
say "読み上げます"
```

### オプション

- `-v voice`, `--voice voice`, `--voice=voice`: 音声を部分一致で指定（表示名・ID・言語・説明から探す）
- `--voice-id id`, `--voice-id=id`: 音声IDを完全一致で指定
- `-r n`, `--rate n`, `--rate=n`: 読み上げ速度（`-10` 〜 `10`、`0` が標準。Windows の音声エンジンの速度スケールです）
- `--volume n`, `--volume=n`: 音量（`0` 〜 `100`、既定 `100`）
- `-f file`, `--file file`, `--file=file`: ファイルの内容を読み上げる（UTF-8 / UTF-16LE / UTF-16BE の BOM に対応）
- `-o out.wav`, `--output out.wav`, `--output=out.wav`: 再生せず WAV ファイルへ書き出す（再生デバイスがない環境でも音声データを作れます）
- `-l`, `--list-voices`: 利用可能な音声を `表示名<TAB>言語<TAB>性別<TAB>ID` で表示する
- 引数も `-f` も与えず、標準入力がリダイレクトされているときは標準入力を読み上げます

音声は `設定 > 時刻と言語 > 音声認識/音声合成 > 音声` から追加インストールできます（Microsoft の自然音声を含む）。`--list-voices` で追加した音声が `MSTTS_V110_*` などの OneCore 系 ID として現れます。

終了コードは、成功が `0`、再生や書き出しに失敗した場合は `1`、使い方が誤っている場合は `2` です。

> 補足: 読み上げには従来の WAVE 出力（waveOut）デバイスを使います。リモートデスクトップや一部の仮想オーディオ構成では再生デバイスが見つからないことがあります。その場合は `-o` で WAV を書き出して確認してください。

