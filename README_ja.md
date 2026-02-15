# Rust クロスプラットフォーム NES エミュレータ

Rustで書かれた、デスクトップ（Windows/Mac/Linux）およびWebAssembly（WASM）対応のファミリーコンピュータ（NES）エミュレータです。

[**ブラウザでプレイ**](https://yoshy3.github.io/rust_emu/)

[English README](./README.md)

> [!NOTE]
> **Vibe Coding**: このプロジェクトは、AIネイティブな開発プロセスである「Vibe Coding」を通じて開発されました。
> **謝辞**: このエミュレータのコアロジック、アーキテクチャ設計、および実装を強力にサポートしてくれた **Gemini code assist** に深く感謝します。

## 特徴
- **クロスプラットフォーム**: デスクトップおよびモダンなウェブブラウザで動作します。
- **レンダリング**: `pixels` ライブラリを使用したハードウェアアクセラレーションによる2D描画。
- **オーディオ**: OversamplingとDCブロッカーを搭載したAPU実装（デスクトップ・Web両対応）。
- **Web対応**: `wasm-bindgen` を使用したビルドと、720p相当へのスケーリング対応。

## 必要条件
- [Rust](https://www.rust-lang.org/tools/install)
- [wasm-pack](https://rustwasm.github.io/wasm-pack/installer.html) (Webビルド用)
- Python 3 (オプション、ローカルWebサーバー用)

## ローカルでの実行方法

### デスクトップ
ネイティブ環境での実行：
```bash
cargo run -- /path/to/game.nes
```
`Esc` キーで終了します。

### Web (WASM)
1. Web向けにビルド：
   ```bash
   wasm-pack build --target web --no-typescript
   ```
2. ローカルサーバーの起動：
   ```bash
   python3 -m http.server 8000
   ```
3. ブラウザで `http://localhost:8000` を開きます。
4. ROMファイルをブラウザ上の画面にドラッグ＆ドロップしてください。

## 操作方法 (Controls)

| NESボタン | デスクトップ (Cargo) | Web (WASM) |
|-----------|--------------------|------------|
| **A**     | `Z`                | `Z`        |
| **B**     | `X`                | `X`        |
| **Select**| `Right Shift`      | `Shift`    |
| **Start** | `Enter`            | `Enter`    |
| **Up**    | `Up Arrow`         | `Up Arrow` |
| **Down**  | `Down Arrow`       | `Down Arrow` |
| **Left**  | `Left Arrow`       | `Left Arrow` |
| **Right** | `Right Arrow`      | `Right Arrow` |
| **Exit**  | `Esc`              | -          |

## プロジェクト構造
- `src/main.rs`: デスクトップ向けハードウェアインターフェース（pixels + cpal）。
- `src/lib.rs`: WebAssemblyブリッジおよび共有エミュレータ。
- `src/cpu.rs`: サイクル精度の6502 CPUコア。
- `src/ppu.rs`: 背景およびスプライト描画をサポートするPPU。
- `src/apu.rs`: 矩形波、三角波、ノイズをサポートするAPU。
- `src/bus.rs`: メモリマップとI/Oを制御するシステムバス。
- `src/cartridge.rs`: iNESフォーマットのローダーとマッパー。
- `src/joypad.rs`: コントローラーの入力状態管理。
- `src/opcodes.rs`: 命令セットとアドレッシングモードの定義。

## トラブルシューティング

### macOS で「開発元を検証できないため開けません」と表示される場合

Release からダウンロードしたバイナリを実行しようとした際に、「"rust_emu"は開発元を検証できないため開けません」といった警告が表示される場合は、以下のコマンドで検疫属性（quarantine attribute）を削除してください：

```bash
xattr -d com.apple.quarantine rust_emu
```

### Linux Wayland 環境での起動について

Linux の Wayland 環境において、バッファサイズやサーフェイスエラー（例: `Buffer size must be an integer multiple of the buffer_scale`）が発生する場合は、以下のように `WAYLAND_DISPLAY` 環境変数を空にして XWayland で実行することで解決する場合があります：

```bash
WAYLAND_DISPLAY= cargo run -- path/to/game.nes
```

## ライセンス
MIT

## GitHub Pages への公開（WASM）

このリポジトリには、WASM ビルド成果物を GitHub Pages に公開する GitHub Actions ワークフローが含まれています。

- 実行トリガー: Git Tag を push したとき（例: `v0.2.1`）
- ワークフロー: `.github/workflows/deploy-pages.yml`
- 公開用ディレクトリ（artifact ルート）: `docs/`
- `docs/` に配置される内容: `index.html` と `pkg/`（`wasm-pack` の生成物）

### 初回のみ必要な設定

GitHub リポジトリの Settings で、**Pages** の公開元を **GitHub Actions** に設定してください。

### リリース手順

```bash
git tag v0.2.1
git push origin v0.2.1
```
