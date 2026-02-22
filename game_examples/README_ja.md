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

## トラブルシューティング（Windows環境で `make` がない場合）

PowerShell やコマンドプロンプトで次のようなエラーが出る場合があります。

- `make : 用語 'make' は、コマンドレット...`
- `'make' is not recognized as an internal or external command`

このリポジトリでは実際のビルドは Docker コンテナ内で行うため、`make` の代わりに `docker compose run` で直接実行できます。

```bash
docker compose run --rm nesdev make build PROJECT=hello_nes
```

Breakout の場合:

```bash
docker compose run --rm nesdev make build PROJECT=breakout
```

ビルド後にエミュレータまで起動する場合:

```bash
docker compose run --rm nesdev make run PROJECT=hello_nes
```

`make` コマンドを使いたい場合は、Git Bash / WSL / MSYS2 など `make` が使えるシェルで `game_examples` の `Makefile` を実行してください。

もしくは、Windows向けに `make` をインストールして環境変数に追加する方法もあります。
```
winget install -e --id GnuWin32.Make
```

`C:\Program Files (x86)\GnuWin32\bin` を環境変数 PATH に追加してください。

## 追加した構成

- `docker/Dockerfile`: NES開発ツールチェーン（`cc65`, `make`）
- `docker-compose.yml`: 開発コンテナ定義
- `Makefile`: Docker経由のビルド/クリーン/シェル操作
- `projects/hello_nes`: 最小ROM生成サンプル
- `projects/breakout`: ブロック崩しサンプル

## 実行イメージ

### Hello NES

![Hello NESの実行画面](./images/hello_nes.png)

### Breakout

![Breakoutの実行画面](./images/breakout.png)
