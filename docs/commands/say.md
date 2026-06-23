# say

任意のテキストを読み上げます（macOS の `say` 相当）。Windows Runtime の `Windows.Media.SpeechSynthesis` を使うため、Windows 10/11 のモダンな（OneCore 系・ニューラル）音声が使われます。自然（ニューラル）音声をインストールしていれば、それも `-v` で選択できます。

## 使い方

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

## オプション

- `-v voice`, `--voice voice`, `--voice=voice`: 音声を部分一致で指定（表示名・ID・言語・説明から探す）
- `--voice-id id`, `--voice-id=id`: 音声IDを完全一致で指定
- `-r n`, `--rate n`, `--rate=n`: 読み上げ速度（`-10` 〜 `10`、`0` が標準。Windows の音声エンジンの速度スケールです）
- `--volume n`, `--volume=n`: 音量（`0` 〜 `100`、既定 `100`）
- `-f file`, `--file file`, `--file=file`: ファイルの内容を読み上げる（UTF-8 / UTF-16LE / UTF-16BE の BOM に対応）
- `-o out.wav`, `--output out.wav`, `--output=out.wav`: 再生せず WAV ファイルへ書き出す（再生デバイスがない環境でも音声データを作れます）
- `-l`, `--list-voices`: 利用可能な音声を `表示名<TAB>言語<TAB>性別<TAB>ID` で表示する
- 引数も `-f` も与えず、標準入力がリダイレクトされているときは標準入力を読み上げます

終了コードは、成功が `0`、再生や書き出しに失敗した場合は `1`、使い方が誤っている場合は `2` です。

## 音声の追加

音声は `設定 > 時刻と言語 > 音声認識/音声合成 > 音声` から追加インストールできます（Microsoft の自然音声を含む）。`--list-voices` で追加した音声が `MSTTS_V110_*` などの OneCore 系 ID として現れます。

## 補足

- 読み上げには従来の WAVE 出力（waveOut）デバイスを使います。リモートデスクトップや一部の仮想オーディオ構成では再生デバイスが見つからないことがあります。その場合は `-o` で WAV を書き出して確認してください。
- WinRT（`Windows.Media.SpeechSynthesis`）を主系のエンジンとして使います。レガシーの SAPI（`SpVoice`）はフォールバック用ですが、Windows 10 1607+ では WinRT が常にあるため、現在は実装していません。
- Windows ターゲット限定で `windows` クレートに依存します。非 Windows ではバイナリはコンパイルできますが、実行すると「サポート対象外」のエラーで終了します。
