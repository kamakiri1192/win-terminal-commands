# open

ファイル、フォルダ、URL を既定のアプリで開きます。macOS の `open` や Windows の `start` に相当します。

## 使い方

```powershell
cargo build --release
.\target\release\open.exe README.md           # 既定のアプリで開く
.\target\release\open.exe .                   # エクスプローラーで開く
.\target\release\open.exe https://example.com # 既定のブラウザで開く
.\target\release\open.exe                     # カレントディレクトリを開く
.\target\release\open.exe -a notepad note.txt # アプリを指定して開く
```

Cargo でインストールすると、`open` を直接呼び出せます。

```powershell
cargo install --path . --force
open README.md
```

## オプション

- `-a app`, `--app app`, `--app=app`: 開くアプリを指定する。アプリ名は `PATH` と `PATHEXT` から解決される

`-a` を指定した場合、後続の対象はすべて指定したアプリへ引数として渡されます。

```powershell
open -a code README.md Cargo.toml
```

## 終了コード

- `0`: すべての対象を開けた
- `1`: 1つ以上の対象を開けなかった
- `2`: コマンドの使い方が誤っている
