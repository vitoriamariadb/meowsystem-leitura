# Integração com o MeowSystem

Este repositório é a casa nova do applet que hoje mora em `src/applets/leitura` do MeowSystem.
Nada do MeowSystem foi mudado: o que está abaixo é a sprint que o faz consumir este repositório.

## Onde está

| | |
|---|---|
| repositório | <https://github.com/vitoriamariadb/meowsystem-leitura> |
| clone (SSH desta máquina) | `git@github.com-personal:vitoriamariadb/meowsystem-leitura.git` |
| app id | `io.github.meowsystem.Leitura` |
| binário | `meowsystem-leitura` |
| `.desktop` | `data/io.github.meowsystem.Leitura.desktop.in` (o `@bindir@` vira o diretório do binário) |
| libcosmic | `1f6dc991eaa5115a09be2505cc8a323e5b2b0bff`, pinado no `Cargo.toml` e no `Cargo.lock` |

O histórico de `src/applets/leitura` veio junto (`git subtree split --prefix=src/applets/leitura`),
e os commits daqui em diante são deste repositório.

## Build

```bash
cargo build --release --locked      # target/release/meowsystem-leitura
cargo test --locked
```

`CARGO_TARGET_DIR` vale como em qualquer projeto cargo; o `leitura_build.sh` de hoje já o aponta
para `~/.local/state/meowsystem/leitura/target`, e pode continuar assim.

A arte entra no binário por `include_bytes!` a partir de `data/icones/`, e não mais de
`../../../../packs/meowsystem/icones/sistema/`: o repositório compila sozinho.

## As chaves (o contrato com o `scripts/leitura.sh`)

Iguais às de hoje, com o mesmo nome de arquivo e o mesmo tipo:

- `~/.config/cosmic/com.system76.CosmicComp/v1/leitura_temperatura` (`u32`, Kelvin, `0` = desligado)
- `.../leitura_textura` (`f32`, `0.0` a `1.0`)
- `.../leitura_agenda` (`bool`; ausente vale `true`)
- `.../leitura_hora_inicio` e `.../leitura_hora_fim` (`"HH:MM"` ou minutos desde a meia-noite;
  padrão `18:00` e `07:00`)

O filtro de horário tem testes (`cargo test`) com as mesmas palavras que o `case` do
`_leitura_minutos` aceita e recusa.

**Uma mudança:** o estado do interruptor (`ultima_temperatura`, `ultima_textura`) segue o app id
novo e passa a morar em `~/.local/state/cosmic/io.github.meowsystem.Leitura/v1/`. Foi o que deixou
os dois applets conviverem nesta máquina sem dividir estado.

## O que o MeowSystem muda para consumir este repositório (a sprint)

1. **A receita.** `scripts/leitura_build.sh` deixa de apontar `RECEITA` para `src/applets/leitura`
   e passa a clonar este repositório num commit pinado (como o `midia_build.sh` faz com o `PINO`),
   em `~/.local/state/meowsystem/leitura/src`. O carimbo troca o sha da fonte e da receita pelo
   commit do pino. O `applet_pino.sh` continua lendo o `rev` do libcosmic do `Cargo.lock`, agora o
   do clone.
2. **O app id e o binário.** `APP_ID` passa a `io.github.meowsystem.Leitura` e `BINARIO` a
   `~/.local/bin/meowsystem-leitura`, no `leitura_build.sh` e no `scripts/leitura.sh`
   (`LEITURA_APPLET_ID`, `LEITURA_APPLET_BIN`, `LEITURA_APPLET_SOMBRA`, `LEITURA_ESTADO_BASE`).
   A sombra pode ser gerada do `data/*.desktop.in` daqui em vez do `_texto_sombra`.
3. **A migração de quem já usa.** Na primeira passada, o build copia
   `~/.local/state/cosmic/com.meowsystem.AppletLeitura/v1/ultima_*` para o diretório novo (se o
   novo não existir), tira a sombra e o binário antigos, e avisa que a topbar cita o nome antigo.
   A troca do nome em `plugins_center`/`plugins_wings` é de quem usa (`docs/FRONTEIRA.md`): o
   MeowSystem só avisa, ou oferece o gesto no painel.
4. **A arte.** O `_sha_arte` passa a olhar `data/icones/` do clone. A fonte da arte continua sendo
   `packs/meowsystem/icones/sistema/leitura-*.svg`; um teste do MeowSystem pode conferir que as
   cópias daqui batem com as do pack.
5. **Os textos e testes que citam o caminho velho.** `tests/leitura-fonte-unica.sh`
   (`ESTADO_APPLET`), `tests/applet-pino.sh`, `lib/noite.sh` (o comentário do `e_noite`),
   `.gitignore` (`src/applets/leitura/target/`), `docs/FRONTEIRA.md` (as linhas do applet) e o
   `app/servidor.py`/`app/pagina/previas-sistema.js`, onde citarem o id antigo.
6. **Tirar `src/applets/leitura`** do MeowSystem quando os passos acima estiverem verdes.

## Tema

O applet segue o tema do COSMIC: `cosmic::applet::style()`, o `popup_container` do libcosmic e os
desenhos `symbolic`, repintados com a cor do tema. O MeowSystem não precisa escrever nada para
este applet: o tema que ele já aplica ao COSMIC basta.
