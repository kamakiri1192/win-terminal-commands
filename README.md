# win-terminal-commands

[![Latest release](https://img.shields.io/github/v/release/kamakiri1192/win-terminal-commands)](https://github.com/kamakiri1192/win-terminal-commands/releases/latest)
[![Release build](https://github.com/kamakiri1192/win-terminal-commands/actions/workflows/release.yml/badge.svg)](https://github.com/kamakiri1192/win-terminal-commands/actions/workflows/release.yml)
![Platform](https://img.shields.io/badge/platform-Windows%20x64-0078D4?logo=windows)
![Language](https://img.shields.io/badge/language-Rust-000000?logo=rust)

macOS や Linux で使われているコマンドを、Windows でも利用できるようにする CLI ツール集です。
`open` や `say` など、Coreutils for Windows では提供されていないコマンドを収録しています。

## コマンド

| コマンド | 説明 | 使用例 |
| --- | --- | --- |
| [`which`](docs/commands/which.md) | `PATH` からコマンドの場所を検索 | `which git` |
| [`open`](docs/commands/open.md) | ファイル、フォルダ、URL をアプリで開く | `open .` |
| [`say`](docs/commands/say.md) | Windows の音声合成でテキストを読み上げる | `say "こんにちは"` |
| [`md5`](docs/commands/md5.md) | MD5 チェックサムを計算・検証 | `md5 file.zip` |
| [`sw_vers`](docs/commands/sw_vers.md) | Windows のバージョン情報を macOS 風に表示 | `sw_vers` |
| [`gzip` / `gunzip`](docs/commands/gzip.md) | gzip 形式の圧縮・展開 | `gzip file.txt` |

詳しい使い方とオプションは、各コマンドのドキュメントを参照してください。

## インストール

ビルド済みの実行ファイルを利用するため、Rust やビルド環境を用意する必要はありません。

1. [最新のリリース](https://github.com/kamakiri1192/win-terminal-commands/releases/latest)を開く
2. `Assets` から `win-terminal-commands-<バージョン>-x86_64-windows.zip` をダウンロードする
3. ダウンロードした ZIP ファイルを任意のフォルダに展開する（例: `%USERPROFILE%\bin\win-terminal-commands`）
4. どのフォルダからでもコマンドを実行できるように、展開先をユーザー環境変数 `Path` に追加する

### Path の設定

1. スタートメニューで「環境変数」を検索し、「システム環境変数の編集」を開く
2. 「環境変数」ボタンを選択する
3. 「ユーザー環境変数」の一覧から `Path` を選び、「編集」を選択する
4. 「新規」を選択し、ZIP ファイルの展開先を追加する
5. 設定を保存し、新しいターミナルを開く

`Path` に追加せず、展開先のフォルダから実行ファイルを直接起動することもできます。

```powershell
cd "$env:USERPROFILE\bin\win-terminal-commands"
.\which.exe git
.\open.exe .
```

`Path` の設定後は、次のように実行できます。

```powershell
which git
open .
say "ビルドが終わりました"
```

## ソースからビルドする

開発版を試す場合や開発に参加する場合は、ソースコードからビルドできます。

あらかじめ Git、[Rust](https://rustup.rs/)、[Microsoft C++ Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/) をインストールしてください。
Microsoft C++ Build Tools のインストール時には、「C++ によるデスクトップ開発」を選択します。

リポジトリを取得し、Cargo でビルドとインストールを行います。

```powershell
git clone https://github.com/kamakiri1192/win-terminal-commands.git
cd win-terminal-commands
cargo install --path . --force
```

## Coreutils for Windows

`ls` や `tail` などの基本的なコマンドには、Microsoft が提供する Coreutils for Windows を利用できます。
必要に応じて、本ツール集とあわせてインストールしてください。

```powershell
winget install Microsoft.Coreutils
```

- [Coreutils for Windows の概要](https://learn.microsoft.com/ja-jp/windows/core-utils/overview)
- [microsoft/coreutils](https://github.com/microsoft/coreutils)
