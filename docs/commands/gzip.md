# gzip / gunzip

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

## オプション

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
