# meowsystem-leitura

O applet do **modo de leitura** na topbar do COSMIC: deixa a tela mais quente e com textura
de papel, à mão ou num horário.

Nasceu dentro do [MeowSystem](https://github.com/vitoriamariadb/MeowSystem), em
`src/applets/leitura`, e foi extraído com o histórico inteiro (`git subtree split`).

## O que ele faz

- Na barra: um sol de dia e uma lua de noite, pela janela do agendamento.
- No popup:
  - **Modo de leitura**: liga e desliga, e religa no último ponto usado.
  - **Temperatura**: de 6500 K (desligado) até 1000 K, de 100 em 100.
  - **Textura de papel**: de 0 a 100%, de 5 em 5.
  - **Agendar**: com o horário de início e de fim, de 15 em 15 minutos. Início igual ao fim
    vale o dia inteiro.
- Quando o compositor da sessão ainda não sabe ler as chaves, uma linha avisa que a tela
  obedece no próximo login.

## Do que ele depende

O applet **não pinta a tela**: ele grava números em chaves do `cosmic-config`, e quem pinta é
um `cosmic-comp` com o patch do modo de leitura do MeowSystem
(`patches/cosmic-comp-modo-leitura.patch`). Sem o patch, o applet funciona e grava, e a tela
não muda. O applet reconhece o compositor com patch pelo marcador `AURORA-READING-MODE` no
binário que está rodando.

## Compilar e instalar

Precisa de Rust 1.93 ou mais novo e das bibliotecas de desenvolvimento do libcosmic
(`libxkbcommon-dev`, `libwayland-dev`, `pkg-config`). O primeiro build compila o libcosmic e
leva vários minutos.

```bash
cargo build --release --locked
cargo test --locked
./install.sh                 # binário em ~/.local/bin, .desktop em ~/.local/share/applications
./install.sh --remover       # desfaz
```

Depois, acrescente **Modo de leitura** em Configurações > Área de trabalho > Painel > Applets.
O applet só aparece na topbar depois que o painel o carrega (novo login, ou o painel
reiniciado).

## As chaves

| onde | chave | tipo | quem escreve |
|---|---|---|---|
| `~/.config/cosmic/com.system76.CosmicComp/v1/` | `leitura_temperatura` | `u32`, Kelvin, `0` = desligado | o applet e o agendador |
| | `leitura_textura` | `f32`, `0.0` a `1.0` | o applet e o agendador |
| | `leitura_agenda` | `bool` | o applet |
| | `leitura_hora_inicio` | `"HH:MM"` ou minutos desde a meia-noite | o applet |
| | `leitura_hora_fim` | idem | o applet |
| `~/.local/state/cosmic/io.github.meowsystem.Leitura/v1/` | `ultima_temperatura`, `ultima_textura` | o ponto ao qual o interruptor volta | o applet |

O applet lê e acompanha as cinco chaves: se o agendador (o `scripts/leitura.sh` do MeowSystem)
mudar uma delas, a interface acompanha.

## Tema

O applet usa o estilo de applet do libcosmic e repinta os quatro desenhos (`data/icones/`) com
a cor do tema do COSMIC (`symbolic`). O tema que o MeowSystem aplica vale aqui sem nenhuma linha
a mais. Os desenhos são cópia dos `leitura-*.svg` de `packs/meowsystem/icones/sistema/`.

## Licença

GPL-3.0, a mesma do MeowSystem. Ver [`LICENSE`](LICENSE).
