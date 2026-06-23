# which

`PATH` からコマンドの場所を探して表示します。Windows では `PATHEXT` も考慮するため、`which git` で `git.exe` などを見つけられます。

## 使い方

```powershell
cargo build --release
.\target\release\which.exe git
.\target\release\which.exe -a node
```

Cargo でインストールすると、`which` を直接呼び出せます。

```powershell
cargo install --path . --force
which git
```

## オプション

- `-a`, `--all`: 見つかった候補をすべて表示する
- `-s`, `--silent`: 結果を表示せず、終了コードだけ返す

複数のコマンドを一度に指定できます。

```powershell
which git cargo node
```

## 終了コード

- `0`: 指定したコマンドがすべて見つかった
- `1`: 1つ以上のコマンドが見つからなかった
- `2`: コマンドの使い方が誤っている
