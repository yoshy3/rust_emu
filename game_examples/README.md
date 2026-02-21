# game_examples

このディレクトリは、ファミコン（NES）向けゲーム開発環境のルートです。

- 各ゲームプロジェクトは `projects/` 配下に作成します
- 生成したROM（`.nes`）や関連アセットもこの配下で管理します
- このリポジトリのエミュレータ（`rust_emu`）で動作確認できます

## 前提

- Docker Desktop がインストール済み
- `docker compose` コマンドが利用可能

## 使い方

`game_examples` で実行:

```bash
make build PROJECT=hello_nes
```

Breakoutサンプルをビルドする場合:

```bash
make build PROJECT=breakout
```

ビルドしてそのままエミュレータを起動する場合:

```bash
make run PROJECT=hello_nes
```

```bash
make run PROJECT=breakout
```

初回はDockerイメージのビルドが走ります。成功すると以下が生成されます。

- `projects/hello_nes/build/hello_nes.nes`

## 追加した構成

- `docker/Dockerfile`: NES開発ツールチェーン（`cc65`, `make`）
- `docker-compose.yml`: 開発コンテナ定義
- `Makefile`: Docker経由のビルド/クリーン/シェル操作
- `projects/hello_nes`: 最小ROM生成サンプル
- `projects/breakout`: ブロック崩しサンプル
