# win-terminal-commands

macOS、Linux でよく使うコマンドを Windows でも使いやすくするための Rust CLI 集です。  
Windows公式で配布されているCoreutilsでカバーされないコマンドを実装して、普段のmacOSでの開発に近い感覚でCLI操作ができるようになることを目指します。

`which`、`open`、`sw_vers` を実装しています。


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

## sw_vers

macOS の `sw_vers` に合わせて、OS のバージョン情報を表示します。Windows の情報を macOS 風のフォーマットに当てはめて出力します。

```powershell
cargo build --release
.\target\release\sw_vers.exe                  # 既定の3行を表示
.\target\release\sw_vers.exe -productVersion  # バージョン番号だけ
.\target\release\sw_vers.exe -buildVersion    # ビルド番号だけ
```

PATH にインストールして直接 `sw_vers` で呼ぶこともできます。

```powershell
cargo install --path . --force
sw_vers
```

既定の出力は macOS と同様にタブ区切りです。

```text
ProductName:    Windows
ProductVersion: 10.0.26200
BuildVersion:   26200.8655
```

### オプション

- `-productName`: プロダクト名だけを表示（`Windows`）
- `-productVersion`: バージョン番号だけを表示（`Major.Minor.Build`、例: `10.0.26200`）
- `-productVersionExtra`: 追加のバージョン修飾子だけを表示（機能更新プログラム、例: `25H2`）
- `-buildVersion`: ビルド番号だけを表示（`Build.UBR`、例: `26200.8655`）。`winver` の「OS ビルド」と一致します

これらは macOS と同じく**排他**で、複数同時には指定できません。終了コードは成功時 `0`、使い方が誤っている場合は `1` です。

### macOS との対応

macOS に直接相当する情報がないため、各項目は Windows で最も信頼できるソースに割り当てています。

| 項目 | Windows での取得元 | 備考 |
| --- | --- | --- |
| `ProductName` | `Windows`（固定） | macOS の `macOS` に相当する OS ファミリー名。レジストリの `ProductName` は Windows 11 でも `Windows 10 Pro` と記録されることがあるため使いません |
| `ProductVersion` | `RtlGetVersion`（ntdll） | マニフェストのバージョン偽装に影響されない、正確な `Major.Minor.Build` |
| `BuildVersion` | レジストリ `CurrentBuild` + `UBR` | `winver` の「OS ビルド」と同じ形式 |
| `productVersionExtra` | レジストリ `DisplayVersion`（なければ `ReleaseId`） | Windows には Rapid Security Response がないため、機能更新プログラム（`23H2` など）を代わりに出します |
