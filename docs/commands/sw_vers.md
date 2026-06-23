# sw_vers

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

## オプション

- `-productName`: プロダクト名だけを表示（`Windows`）
- `-productVersion`: バージョン番号だけを表示（`Major.Minor.Build`、例: `10.0.26200`）
- `-productVersionExtra`: 追加のバージョン修飾子だけを表示（機能更新プログラム、例: `25H2`）
- `-buildVersion`: ビルド番号だけを表示（`Build.UBR`、例: `26200.8655`）。`winver` の「OS ビルド」と一致します

これらは macOS と同じく**排他**で、複数同時には指定できません。終了コードは成功時 `0`、使い方が誤っている場合は `1` です。

## macOS との対応

macOS に直接相当する情報がないため、各項目は Windows で最も信頼できるソースに割り当てています。

| 項目 | Windows での取得元 | 備考 |
| --- | --- | --- |
| `ProductName` | `Windows`（固定） | macOS の `macOS` に相当する OS ファミリー名。レジストリの `ProductName` は Windows 11 でも `Windows 10 Pro` と記録されることがあるため使いません |
| `ProductVersion` | `RtlGetVersion`（ntdll） | マニフェストのバージョン偽装に影響されない、正確な `Major.Minor.Build` |
| `BuildVersion` | レジストリ `CurrentBuild` + `UBR` | `winver` の「OS ビルド」と同じ形式 |
| `productVersionExtra` | レジストリ `DisplayVersion`（なければ `ReleaseId`） | Windows には Rapid Security Response がないため、機能更新プログラム（`23H2` など）を代わりに出します |
