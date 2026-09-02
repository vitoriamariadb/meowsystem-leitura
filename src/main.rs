// meow-applet-leitura — os dois sliders do modo de leitura, na topbar dela.
//
// O QUE ELE FAZ, EM UMA FRASE
//   Desenha um ícone na barra; clicando, abre um popup com um interruptor, dois
//   sliders (Temperatura em Kelvin, Textura em por cento) e um agendamento com
//   hora de início e de fim. Cada arrasto GRAVA uma chave em
//   `~/.config/cosmic/com.system76.CosmicComp/v1/`. Quem PINTA é o `cosmic-comp`
//   recompilado com o patch do modo de leitura — este binário não fala protocolo
//   Wayland nenhum, não desenha um pixel na tela dela fora do próprio popup, e
//   não sabe o que é gamma.
//
// ============================================================================
// O QUE ESTÁ MEDIDO NESTA MÁQUINA — 30/08/2026, COM O ITEM 1 REMEDIDO EM 31/08
// ============================================================================
//
// 1. O COMPOSITOR JÁ LÊ ESTAS CHAVES — O QUE ESTAVA AQUI ENVELHECEU EM HORAS
//   A primeira redação deste bloco afirmava que o compositor do disco AINDA NÃO
//   sabia ler, com `strings -a /usr/bin/cosmic-comp | grep -c
//   AURORA-READING-MODE` dando 0 e o `/var/lib/aurora/cosmic-comp-patches.estado`
//   das 01:46 declarando só o patch de workspace e o de raio.
//
//   REMEDIDO EM 31/08/2026, E OS TRÊS NÚMEROS VIRARAM:
//      strings -a /usr/bin/cosmic-comp | grep -c AURORA-READING-MODE  ->  1
//      cosmic-comp-patches.estado (regravado 30/08 05:31) traz a terceira linha
//        `opt AURORA-READING-MODE-1 cosmic-comp-modo-leitura.patch presente`
//      o cosmic-comp VIVO (pid 3060, subiu 30/08 20:25) carrega o marcador:
//        strings -a /proc/3060/exe | grep -c AURORA-READING-MODE  ->  1
//   O binário é de 30/08 04:59 — o build saiu três horas DEPOIS da medição que
//   virou este comentário, e a sessão dela já subiu com ele. Ou seja: os
//   sliders pintam a tela desde a noite de 30/08, e o `aviso` do rodapé do
//   popup (mais abaixo) não aparece nesta máquina hoje.
//
//   O QUE CONTINUA VERDADE É O QUE IMPORTA: ele grava CHAVE, não pixel. No dia
//   em que um `apt upgrade` trocar o cosmic-comp por um sem o patch, o applet
//   segue gravando certo e é o aviso do rodapé que volta a explicar por que a
//   tela não obedece. Um applet que fingisse ter mexido na tela seria pior.
//
// 2. AS CINCO CHAVES, E QUEM MAIS ESCREVE NELAS
//   `leitura_temperatura` (u32, Kelvin, 0 = desligado) e `leitura_textura` (f32,
//   0.0..1.0) são os dois números que o shader lê por quadro — os tipos vêm do
//   struct `CosmicCompConfig` do patch, conferidos linha a linha em
//   `patches/cosmic-comp-modo-leitura.patch`. As outras três
//   (`leitura_agenda`, `leitura_hora_inicio`, `leitura_hora_fim`) o compositor
//   ignora: são só nossas, e quem as LÊ é o `scripts/leitura.sh`.
//
//   As duas primeiras têm DOIS escritores — este applet e o agendador — e isso
//   é o desenho, não um descuido (docs/FRONTEIRA.md). É exatamente por isso que
//   existe o `watch_config` mais abaixo: às 18:00 o agendador escreve, e o
//   slider tem de andar sozinho para o lugar novo em vez de mentir sobre o que
//   está na tela.
//
// 3. O FORMATO DO HORÁRIO NÃO É ESCOLHA NOSSA — JÁ HAVIA UM LEITOR
//   O `_leitura_minutos` do `scripts/leitura.sh` aceita `"HH:MM"` (string RON,
//   com ou sem aspas) E o inteiro de minutos desde a meia-noite. Gravamos
//   `String` "18:00": é a forma que ela lê no arquivo sem precisar dividir
//   1080 por 60 de cabeça, e o `cosmic_config` a serializa com aspas, que o
//   `_leitura_cru` já apara. Inventar um terceiro formato seria a única coisa
//   proibida aqui.
//
// ============================================================================
// AS DUAS DECISÕES DELA, DE 29/08/2026, QUE MANDAM SOBRE O PLANO
// ============================================================================
//
//   "se é um slider por que não dá pra comportar todos?"  — a faixa é INTEIRA,
//   1000K a 6500K, que é a tabela WHITEPOINTS do redshift que o
//   aurora-night-light.py já carrega. Cortar em 1700 ou 2500 seria decidir por
//   ela o que é quente demais sem ela nunca ter visto.
//
//   O HORÁRIO MORA AQUI, não no meow.conf. Uma fonte de verdade só, em vez de
//   duas que podem divergir em silêncio.
//
// ============================================================================
// O TOPO DO SLIDER GRAVA `0`, E NÃO `6500` — A ÚNICA SUTILEZA DESTE ARQUIVO
// ============================================================================
//   6500K é o neutro: a tela com 6500K é a tela sem modo de leitura. Mas o
//   `is_noop()` do patch exige `leitura_temperatura == 0` para PULAR o passe de
//   postprocess inteiro. Gravar 6500 deixaria o shader rodando uma multiplicação
//   neutra por quadro, para sempre, sem nada na tela para acusar — custo de GPU
//   invisível é o pior tipo. Então o topo do slider é DESLIGADO, e o rótulo diz
//   isso com todas as letras em vez de mostrar "6500 K" e deixá-la adivinhar.
//   O `scripts/leitura.sh` faz a mesma conta, no mesmo lugar, pelo mesmo motivo.

use cosmic::app::Core;
use cosmic::applet::padded_control;
use cosmic::cosmic_config::cosmic_config_derive::CosmicConfigEntry;
use cosmic::cosmic_config::{self, Config, ConfigGet, ConfigSet, CosmicConfigEntry};
use cosmic::iced::{Alignment, Length, Limits, Subscription, window, window::Id};
use cosmic::surface::action::{app_popup, destroy_popup};
use cosmic::theme;
use cosmic::widget::{Column, Row, Slider, container, divider, icon, spin_button, text, toggler};
use cosmic::{Action, Element, Task};

/// O `app_id` do applet. É o basename do `.desktop` que o `cosmic-panel` procura
/// e a palavra que entra no `plugins_wings` — os três TÊM de ser iguais, e é por
/// isso que a constante existe em vez de a string ser repetida.
const ID: &str = "com.meowsystem.AppletLeitura";

/// O namespace das chaves do compositor. Não é nosso: é o do `cosmic-comp`, e
/// nós só acrescentamos cinco arquivos a ele (docs/FRONTEIRA.md).
const NS_COMPOSITOR: &str = "com.system76.CosmicComp";
const NS_COMPOSITOR_V: u64 = 1;

/// O nosso próprio estado, em `~/.local/state/cosmic/`. Guarda só a última
/// temperatura e a última textura NÃO NULAS, para o interruptor "Modo de
/// leitura" saber ao que voltar. Sem isto, desligar e religar cairia no padrão
/// de fábrica e ela perderia o ponto que passou a noite achando — e perderia
/// logo no logout, que é justamente quando ela vai reiniciar este processo.
const NS_ESTADO: &str = "com.meowsystem.AppletLeitura";
const NS_ESTADO_V: u64 = 1;

// ============================================================================
// OS QUATRO ÍCONES AUTORAIS, DENTRO DO BINÁRIO — 31/08/2026
// ============================================================================
//
// O QUE ELA PEDIU
//   "faltou inserir os svgs na guia que abre e fazer o icon do applet mudar
//   conforme o horario do dia."
//
// QUAIS SÃO, E POR QUE ESTES QUATRO
//   São os do conjunto "modo de leitura" da folha de 29/08/2026
//   (docs/pesquisas/2026-08-29-modo-leitura.html, aba "Os candidatos do modo de
//   leitura"). `r2` e `s3` são o PAR que ela viu e aprovou lá — "a lua sobre o
//   texto" e "o sol no horizonte", desenhados com a mesma massa (forma grande em
//   cima, duas linhas embaixo) justamente para que a troca não pule na barra.
//   `r1` e `r6` estavam marcados na mesma folha como as outras duas opções de
//   pé; aqui eles ganham lugar próprio, ao lado dos rótulos que nomeiam.
//
// POR QUE `include_bytes!` E NÃO O TEMA DE ÍCONES
//   A folha previa instalá-los em `MeowSystem-Icons/scalable/status/`. Esse
//   diretório tem DONO — o `scripts/icones_sistema.sh`, que o varre contra o
//   `assets/icones/sistema.map` e REMOVE ÓRFÃO (o cabeçalho dele diz isso em voz alta).
//   Quatro arquivos nossos ali seriam apagados na próxima passada daquele
//   script, e o applet ficaria sem ícone sem ninguém ter mexido nele.
//
//   Embutidos no binário eles não dependem de tema, não dependem do cache de
//   ícones do lançador (que já enganou este projeto uma vez) e não podem
//   divergir do código que os desenha: o SVG e o `view()` que o usa entram no
//   mesmo build. O custo é 2 KB de binário para os quatro.
//
// `symbolic(true)` É O QUE FAZ A COR SER A DO TEMA
//   Os arquivos trazem `stroke="#cdd6f4"` (o texto do Mocha), mas nada disso
//   chega à tela: com `symbolic`, o libcosmic descarta a cor do arquivo e
//   repinta com o `on` do fundo — que é o mesmo caminho de todo ícone da barra
//   dela. É por isso que o mesmo arquivo serve tema claro e escuro.
const ARTE_LUA: &[u8] = include_bytes!("../../../../assets/icones/autorais/leitura/r2.svg");
const ARTE_SOL: &[u8] = include_bytes!("../../../../assets/icones/autorais/leitura/s3.svg");
const ARTE_LIVRO: &[u8] = include_bytes!("../../../../assets/icones/autorais/leitura/r1.svg");
const ARTE_HALO: &[u8] = include_bytes!("../../../../assets/icones/autorais/leitura/r6.svg");

/// O tamanho dos ícones que acompanham os rótulos DENTRO do popup. O da barra
/// não passa por aqui: quem o dimensiona é o `suggested_size` do libcosmic, que
/// sabe a altura da topbar dela e a escala da tela.
const ICONE_ROTULO: u16 = 16;

/// 6500K é o neutro e o topo da faixa; 1000K é o fundo da tabela WHITEPOINTS.
const NEUTRO: u32 = 6500;
const PISO: u32 = 1000;
/// O passo do slider de temperatura. Sem ele o arrasto vira uma escrita de
/// arquivo POR PIXEL percorrido — e cada escrita acorda o vigia do
/// cosmic-config do compositor.
const PASSO_K: u32 = 100;
/// O passo da textura, em pontos percentuais. Mesmo motivo.
const PASSO_TEXTURA: u32 = 5;
/// O passo do horário, em minutos. Quinze é o que faz um agendamento caber em
/// poucos cliques sem virar um relógio de precisão que ninguém pediu.
const PASSO_MINUTO: i32 = 15;

/// Por quanto tempo o dedo continua mandando depois do último `on_change` de
/// slider. Não é número de gosto: é o TETO DO ESTRAGO quando o `on_release`
/// nunca vem (ver o campo `ultimo_toque`). Dois segundos passam despercebidos
/// dentro de um arrasto — os eventos de movimento do ponteiro chegam a cada
/// poucos milissegundos — e limitam a janela em que o applet ignora o agendador
/// a algo que ninguém consegue ver.
///
/// O QUE ESTE FREIO REABRE, E POR QUE MESMO ASSIM ELE FICA EM 2 s — 31/08/2026
///   O pavio curto tem um custo, e ele não estava escrito aqui: PARAR O DEDO no
///   meio do arrasto por mais de 2 s, com o botão ainda apertado, e receber um
///   tique do agendador nesse intervalo faz o `arrastando` cair e o cursor
///   PULAR para o valor que o agendador escreveu. E olhar a tela por três
///   segundos para julgar a cor é o gesto natural deste controle — o slider
///   existe para ela ver o âmbar, não para ela mirar um número.
///
///   Dormente hoje: `leitura_agenda` vale `false` no disco (conferido em
///   31/08/2026), e com o agendamento desligado o `scripts/leitura.sh` sai sem
///   escrever chave nenhuma. Morde no dia em que ela religar o "Agendar".
///
///   FICA EM 2 s, e a razão não é o tamanho do risco, é a DIREÇÃO DELE:
///
///   a) O pulo mostra a VERDADE, e o silêncio mostraria mentira. Quando o freio
///      solta, o disco mudou de verdade — o agendador escreveu e a tela dela já
///      está na cor nova. Um popup que segurasse o número dela estaria
///      desenhando algo que não está em lugar nenhum, que é exatamente o que a
///      assinatura `watch_config` existe para impedir (ver o `subscription`).
///
///   b) O valor dela JÁ FOI PERDIDO antes de o freio opinar. Cada `on_change`
///      do arrasto grava a chave na hora; quando o agendador escreve por cima,
///      o número dela sai do disco quer o applet perceba, quer não. Nenhum
///      pavio, de nenhum tamanho, devolve esse valor — o pavio só decide se o
///      applet CONTA a ela ou não. Um pavio longo não protege o arrasto; ele
///      esconde o atropelamento.
///
///   c) O engolido não volta. O `DiscoMudou` só chega quando o arquivo muda; um
///      evento descartado deixa o popup parado até a PRÓXIMA escrita. No meio de
///      uma rampa isso se cura em um minuto (`meow-leitura.timer` é
///      `OnCalendar=*-*-* *:*:00`), mas no ÚLTIMO degrau de uma rampa a próxima
///      escrita é na outra borda da janela — horas depois. Ou seja: o erro do
///      pavio longo dura horas e é calado; o erro do pavio curto dura até o
///      próximo pixel de movimento e é visível.
///
///   d) O pulo se desfaz sozinho. MEDIDO no fonte do slider deste rev: o `draw`
///      posiciona o punho a partir de `self.value` — o valor que NÓS passamos,
///      não de estado interno (iced/widget/src/slider.rs:571-582) — e o braço
///      `CursorMoved`, enquanto `is_dragging`, recalcula pelo cursor (linhas
///      464-475). O primeiro pixel que ela mover republica `on_change` e o punho
///      volta para debaixo do dedo.
///
///   O tamanho da exposição, para o registro: o tique é de minuto em minuto e só
///   ESCREVE quando o número muda de verdade (a rampa em repouso não escreve
///   nada), então uma pausa de D segundos com o botão apertado corre risco
///   (D - 2)/60 — parar três segundos para julgar a cor dá ~1,7%.
const FREIO_ARRASTO: std::time::Duration = std::time::Duration::from_secs(2);

/// Os padrões de quando ela liga o modo pela primeira vez. São os MESMOS
/// números do `meow.conf.exemplo` (LEITURA_TEMPERATURA / LEITURA_TEXTURA), de
/// propósito: duas metades do projeto que estreiam com valores diferentes é como
/// se ganha um "por que a minha tela ficou diferente da que o arquivo diz".
const PADRAO_K: u32 = 3500;
const PADRAO_TEXTURA: f32 = 0.35;
const PADRAO_INICIO: &str = "18:00";
const PADRAO_FIM: &str = "07:00";

/// As cinco chaves, com os NOMES EXATOS dos arquivos no disco: o
/// `CosmicConfigEntry` derivado usa `stringify!(campo)` como nome de chave, então
/// renomear um campo aqui renomeia um arquivo lá — e quebraria calado tanto o
/// shader quanto o `scripts/leitura.sh`.
#[derive(Clone, Debug, PartialEq, CosmicConfigEntry)]
#[version = 1]
pub struct Chaves {
    pub leitura_temperatura: u32,
    pub leitura_textura: f32,
    pub leitura_agenda: bool,
    pub leitura_hora_inicio: String,
    pub leitura_hora_fim: String,
}

impl Default for Chaves {
    fn default() -> Self {
        Self {
            // OS DOIS NASCEM DESLIGADOS, igual ao Default do struct do
            // compositor. É o que faz uma máquina sem chave nenhuma no disco
            // acordar com a tela normal.
            leitura_temperatura: 0,
            leitura_textura: 0.0,
            // `true`, e isto NÃO é simetria com o de cima: o `leitura.sh` trata
            // a chave AUSENTE como "o relógio manda" (`LEITURA_APPLET_DIZ`
            // vazio cai no ramo do relógio). Se o padrão daqui fosse `false`, o
            // interruptor mostraria "Agendar: desligado" numa máquina onde o
            // agendamento está ligado — o applet mentiria sobre o sistema no
            // primeiro segundo em que ela o abrisse.
            leitura_agenda: true,
            leitura_hora_inicio: PADRAO_INICIO.to_string(),
            leitura_hora_fim: PADRAO_FIM.to_string(),
        }
    }
}

/// "18:00" -> 1080. Devolve `None` em qualquer outra coisa, e o chamador cai no
/// padrão em vez de mostrar um horário inventado.
///
/// O INTEIRO DE MINUTOS TAMBÉM ENTRA, E ISSO É CONTRATO, NÃO GENEROSIDADE
///   O item 3 do cabeçalho deste arquivo já declarava que o `_leitura_minutos`
///   do `scripts/leitura.sh` aceita DUAS formas — `"HH:MM"` e o inteiro de
///   minutos desde a meia-noite — e o cabeçalho do leitura.sh repete que a
///   segunda existe justamente para ela poder abrir o arquivo e escrever `1080`
///   à mão. MEDIDO em 31/08/2026: esta função implementava só a primeira. Com
///   `leitura_hora_inicio` valendo `"1200"` no disco, o leitura.sh agendava
///   20:00 (`_leitura_minutos` casa o ramo do inteiro, teto 1439) e o applet
///   mostrava 18:00 — o PADRÃO — sem uma linha em lugar nenhum; e o primeiro
///   clique no `+` gravava "18:15" por cima dos 20:00 dela. Duas metades do
///   projeto lendo a MESMA chave e discordando em silêncio é exatamente o que
///   "uma fonte de verdade só" existe para impedir.
///
/// E A GENEROSIDADE, NO MESMO DIA, ABRIU O BURACO PELO OUTRO LADO — 31/08/2026
///   O ramo do inteiro entrou frouxo, e o de `HH:MM` já era. Extraindo esta
///   função e o `_leitura_minutos` dos dois arquivos e passando a MESMA lista de
///   23 entradas pelos dois, a versão anterior daqui aceitava CINCO que o shell
///   recusa. Quatro eram defeito de verdade:
///       "+1080"    — o `i32::from_str` do Rust aceita o sinal (medido:
///                    `"+1080".parse::<i32>()` = `Ok(1080)`)
///       "18: 00"   — o `m.trim()` do meio limpava o espaço INTERNO
///       "018:00"   — três dígitos de hora, que o `case` do shell não casa
///       "18:0"     — um dígito de minuto, idem
///   A quinta era `" 18:00 "`, e essa não morreu: mudou de lugar — ver o
///   `ponta_do_horario` logo abaixo, que é onde o espaço de FORA passou a ser
///   aparado, do mesmo jeito e na mesma camada que o `_leitura_cru` do shell.
///   Com `leitura_hora_fim = "18:0"` o popup desenhava "às 18:00" e o agendador
///   caía no padrão do meow.conf, 07:00: a tela virava num horário e a interface
///   jurava outro, calada. É o defeito de cima com o sinal trocado, e é pior —
///   lá o applet desenhava o padrão e dava para desconfiar; aqui ele desenha um
///   horário plausível que nunca vai acontecer.
///
///   O CONSERTO É APERTAR ESTA FUNÇÃO, E NÃO AFROUXAR O SHELL: quem manda no
///   comportamento é o agendador; o applet só desenha. Uma tela que obedece ao
///   `leitura.sh` e um popup que aceita mais que ele são duas verdades, e a que
///   ela vê é a errada.
///
/// A GRAMÁTICA, DÍGITO A DÍGITO, COPIADA DO `case` DO `_leitura_minutos`
///   `HH:MM`   1 ou 2 dígitos de hora, EXATAMENTE 2 de minuto, hora <= 23,
///             minuto <= 59. Zero à esquerda vale — `08:00` é 480 nos dois
///             lados, que é para isso que existe o `10#` do shell.
///   inteiro   só dígitos, pelo menos um, valor <= 1439. Zero à esquerda vale
///             aqui também: `0080` é 80 nos dois lados.
///   Nenhum espaço, nem dentro nem em volta — o espaço em volta é aparado
///   ANTES, no `ponta_do_horario`, que é onde o `_leitura_cru` do shell o apara.
///   Todo o resto é `None` de um lado e `return 1` do outro: sinal, vírgula,
///   ponto, `1e3`, vazio, hora de três dígitos, minuto de um.
fn minutos_de_hhmm(v: &str) -> Option<i32> {
    if let Some((h, m)) = v.split_once(':') {
        // O `case` do shell é `[0-9][0-9]:[0-9][0-9]|[0-9]:[0-9][0-9]`, e padrão
        // de `case` casa a palavra INTEIRA — contar os dígitos aqui é a tradução
        // literal disso, e é o que recusa "018:00" e "18:0". "18:00:00" também
        // morre nesta linha: o `split_once` deixa "00:00" no minuto.
        if !(1..=2).contains(&h.len()) || m.len() != 2 {
            return None;
        }
        // O `all(is_ascii_digit)` é o `[0-9]` do padrão, e cobre o espaço
        // interno ("18: 00") e o sinal ("+1:00") de uma vez só.
        if !h.bytes().all(|b| b.is_ascii_digit()) || !m.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        let (h, m): (i32, i32) = (h.parse().ok()?, m.parse().ok()?);
        return (h <= 23 && m <= 59).then_some(h * 60 + m);
    }
    // Sem os dois pontos só resta o inteiro de minutos.
    //
    // QUEM RECUSA O SINAL É ESTA LINHA, E NÃO O `parse` — a versão anterior
    // afirmava o contrário em um comentário, e era falso nas duas metades.
    // MEDIDO em 31/08/2026: `"+1080".parse::<i32>()` devolve `Ok(1080)` e
    // `"-1".parse::<i32>()` devolve `Ok(-1)`. O `+` passava inteiro; o `-` só
    // não passava porque o `(0..=1439)` recusava o negativo DEPOIS. O shell nem
    // chega perto disso: o `*[!0-9]*` do `case` mata os dois sinais, o vazio e o
    // lixo antes de qualquer conta, e é essa ordem que se copia aqui.
    if v.is_empty() || !v.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    // `i64` e não `i32` para o veredito ser "maior que 1439" e não "não coube",
    // como no shell: `$((10#…))` do bash é intmax_t, e um `parse::<i32>()`
    // recusaria "9999999999" por transbordo (medido). Chega ao mesmo `None`,
    // pelo motivo certo. Acima de 19 dígitos os dois divergem e isso é de
    // propósito: `$((10#18446744073709551716))` dá 100 no bash — dá a volta em
    // silêncio e o shell ACEITARIA — enquanto aqui o `parse` recusa. Um bug do
    // bash não é contrato para replicar.
    let m: i64 = v.parse().ok()?;
    (m <= 1439).then_some(m as i32)
}

/// Uma ponta do horário, resolvida como o `_leitura_uma_ponta` do
/// `scripts/leitura.sh` a resolve: o valor do disco quando ele PASSA no filtro,
/// o padrão quando não.
///
/// O `trim()` MORA AQUI, E NÃO MAIS DENTRO DO `minutos_de_hhmm` — 31/08/2026
///   Ele estava lá dentro, e era a única frouxidão desta metade que NÃO era
///   defeito: o shell também apara o espaço em volta, só que uma camada antes,
///   no `_leitura_cru` — que trima, tira as aspas do RON e trima de novo, nessa
///   ordem, justamente para tolerar `" 18:00 "` escrito à mão dentro das aspas.
///   Deixá-lo dentro da função tornava as duas gramáticas comparáveis só em 22
///   das 23 entradas; tirá-lo sem mais nada faria o applet recusar um arquivo
///   que o agendador aceita, que é o defeito de novo, de costas.
///   Movido para cá, as duas metades ficam idênticas nos DOIS níveis: função
///   contra função, e caminho de leitura contra caminho de leitura. Do lado do
///   Rust quem faz o papel do `v="${v#\"}"` é o `ron` do cosmic_config, que já
///   entrega a `String` sem aspas.
fn ponta_do_horario(bruto: &str, padrao: &str) -> i32 {
    minutos_de_hhmm(bruto.trim())
        .or_else(|| minutos_de_hhmm(padrao))
        .unwrap_or(0)
}

/// Que horas são, em minutos desde a meia-noite, NO FUSO DELA.
///
/// O `jiff` já está na árvore desta compilação — ele entra pelo libcosmic, e o
/// `Cargo.lock` ao lado o registra desde antes desta função existir. Declará-lo
/// como dependência direta não baixa um byte novo; só torna explícito o que já
/// estava sendo compilado.
///
/// POR QUE NÃO `SystemTime::now()` E UMA CONTA
///   `SystemTime` é UTC. A janela do agendamento é escrita em hora LOCAL — ela
///   arrasta "das 18:00 às 07:00" olhando o relógio da barra —, e nesta máquina
///   isso são três horas de diferença. Sem fuso, o ícone viraria lua às 15:00.
///   `Zoned::now()` lê o fuso do sistema, que é o mesmo que o `date +%H:%M` do
///   `scripts/leitura.sh` usa do outro lado.
fn agora_em_minutos() -> i32 {
    let agora = jiff::Zoned::now();
    i32::from(agora.hour()) * 60 + i32::from(agora.minute())
}

fn hhmm_de_minutos(v: i32) -> String {
    let v = v.rem_euclid(1440);
    format!("{:02}:{:02}", v / 60, v % 60)
}

struct AppletLeitura {
    core: Core,
    popup: Option<Id>,
    chaves: Chaves,
    /// `None` quando não há diretório de configuração ao alcance (o `Config::new`
    /// falha). O applet continua desenhando e os sliders continuam andando; o
    /// que ele não faz é fingir que gravou.
    conf: Option<Config>,
    estado: Option<Config>,
    ultima_temp: u32,
    ultima_textura: f32,
    /// Ligado entre o primeiro `on_change` de um slider e o `on_release`.
    ///
    /// POR QUE ELE EXISTE (e não é zelo): nós gravamos a chave, o vigia do
    /// cosmic-config nos devolve o que acabamos de gravar, e esse retorno chega
    /// ATRASADO — vem de um watcher de inotify atravessando um canal. Num
    /// arrasto rápido o valor que volta é o de dois passos atrás, e aplicá-lo
    /// faria o slider PULAR PARA TRÁS debaixo do dedo dela. Enquanto ela
    /// arrasta, o dedo manda; quando solta, o disco volta a mandar.
    arrastando: bool,
    /// Quando chegou o último `on_change` de slider. É o FREIO DE MÃO do
    /// `arrastando` acima, e existe por um defeito medido — não por zelo.
    ///
    /// MEDIDO em 31/08/2026 no fonte do slider deste rev
    /// (iced/widget/src/slider.rs, o `match &event` do `update()`, linhas
    /// 433-517): são QUATRO os gestos que fogem do `on_release`, e não três como
    /// esta nota dizia. O `on_release` só sai do braço `ButtonReleased`
    /// (linha 452), e só quando `state.is_dragging`:
    ///   1. Ctrl + roda do mouse sobre a barra (linha 476)
    ///   2. seta para cima com o ponteiro em cima dela (linha 499)
    ///   3. seta para baixo, idem (linha 503)
    ///   — os três publicam `on_change` e nunca ligam o `is_dragging`.
    ///   4. Ctrl + clique (linha 441): `let _ = self.default.map(change);` e
    ///      logo abaixo `state.is_dragging = false`. Publica `on_change` E mata
    ///      a bandeira, então o `ButtonReleased` que vem em seguida acha
    ///      `is_dragging == false` e não solta `on_release` nenhum. `command()`
    ///      é `control()` fora do macOS (iced/core/src/keyboard/modifiers.rs:82),
    ///      então nesta máquina o gesto é Ctrl+clique mesmo.
    /// O quarto é o único que hoje não morde: ele só publica se o slider tiver
    /// um `.default(…)`, e os nossos dois não têm — o `self.default` é `None` e
    /// o `map` não chama nada. Ele fica escrito assim mesmo porque a armadilha é
    /// justamente essa: no dia em que alguém acrescentar um `.default(…)` para
    /// dar a ela um "volta ao 3500 K", o gesto passa a publicar `on_change` sem
    /// nunca publicar `on_release` — e o defeito abaixo nasce de novo, sem uma
    /// linha de código mudada aqui. Bastava UM deles para o
    /// `arrastando` ficar `true` até o popup fechar — e o `arrastando` é quem
    /// DESCARTA o que vem do disco: o popup pararia de seguir o agendador das
    /// 18:00 sem nada na tela para acusar. Pior, o `set_leitura_*` derivado
    /// compara com o struct em memória (cosmic-config-derive/src/lib.rs:169), e
    /// um valor que o agendador tivesse escrito viraria "não mudou": arrastar
    /// de volta ao ponto antigo não gravaria coisa nenhuma.
    ultimo_toque: std::time::Instant,
    /// O par que já está no arquivo de estado, ou `None` enquanto nada foi
    /// escrito nesta sessão. Sem ele o `lembra_ponto()` reescreve os mesmos dois
    /// números a cada soltada de slider — ver a guarda lá embaixo.
    estado_gravado: Option<(u32, f32)>,
    /// Respondido UMA VEZ, no `init`, e nunca mais.
    ///
    /// A sonda lê 33 MB de binário do disco; o `view_window` é chamado a cada
    /// quadro do arrasto. Chamá-la de dentro do desenho seria ler trinta e três
    /// megabytes por pixel percorrido — o applet ficaria mais caro que o efeito
    /// que ele controla. E a resposta não muda no meio da sessão: trocar o
    /// binário do compositor exige reiniciar o compositor, e reiniciar o
    /// compositor mata este processo junto.
    compositor_le: bool,
}

#[derive(Clone, Debug)]
enum Message {
    AbreFecha,
    PopupFechou(Id),
    Ligar(bool),
    Temperatura(u32),
    Textura(u32),
    Soltou,
    Agendar(bool),
    HoraInicio(i32),
    HoraFim(i32),
    /// Carrega o `Chaves` já extraído, e NÃO o `cosmic_config::Update<Chaves>`
    /// que a assinatura entrega. MEDIDO ao compilar em 30/08/2026: o `Update`
    /// do libcosmic 1f6dc99 não implementa `Clone`, e a `Message` de uma
    /// `cosmic::Application` tem de ser `Clone`. Os erros que vinham no
    /// `Update` são consumidos onde nascem — no `map` da assinatura.
    DiscoMudou(Chaves),
    /// O relógio bateu. Não carrega nada e não escreve nada: existe só para o
    /// `view()` rodar de novo e o ícone da barra trocar de sol para lua na hora
    /// certa. Ver o `subscription()`.
    Minuto,
}

impl AppletLeitura {
    /// `true` quando qualquer um dos dois números está acima de zero. Não é
    /// "temperatura != 0": a textura sozinha também acende o passe do shader, e
    /// um interruptor que dissesse "desligado" com a textura em 40% mentiria.
    fn ligado(&self) -> bool {
        self.chaves.leitura_temperatura != 0 || self.chaves.leitura_textura > 0.0
    }

    /// É noite AGORA, pela janela que ELA configurou?
    ///
    /// A REGRA É COPIADA, LINHA POR LINHA, DO AGENDADOR
    ///   O `BEGIN` do `_leitura_alvo_agora` (scripts/leitura.sh) e o `e_noite()`
    ///   do `wallpaper.sh` decidem assim, e a igualdade entre os três é o que
    ///   impede esta máquina de ter duas noites:
    ///
    ///       ini == fim  ->  é noite o dia inteiro
    ///       ini >  fim  ->  a janela atravessa a meia-noite
    ///       ini <  fim  ->  janela normal, dentro do mesmo dia
    ///
    /// A JANELA VALE MESMO COM O "AGENDAR" DESLIGADO, E ISSO É DE PROPÓSITO
    ///   Ela pediu "o icon do applet mudar conforme o HORÁRIO DO DIA" — não
    ///   "conforme o interruptor". Com o Agendar desligado o relógio não mexe na
    ///   tela, mas continua sendo noite lá fora, e o desenho na barra continua
    ///   dizendo a verdade sobre a hora. Quem conta o estado da TELA é o
    ///   interruptor no topo do popup, que fica a um clique de distância.
    fn e_noite(&self) -> bool {
        let ini = ponta_do_horario(&self.chaves.leitura_hora_inicio, PADRAO_INICIO);
        let fim = ponta_do_horario(&self.chaves.leitura_hora_fim, PADRAO_FIM);
        let agora = agora_em_minutos();
        if ini == fim {
            true
        } else if ini > fim {
            agora >= ini || agora < fim
        } else {
            agora >= ini && agora < fim
        }
    }

    /// O desenho que vai para a topbar: a lua de noite, o sol de dia.
    fn arte_da_barra(&self) -> &'static [u8] {
        if self.e_noite() { ARTE_LUA } else { ARTE_SOL }
    }

    /// Um dos quatro desenhos, no tamanho de rótulo, pronto para entrar numa
    /// `Row`. `symbolic(true)` é o que joga fora a cor do arquivo e repinta com
    /// a do tema — ver o bloco dos ícones lá em cima.
    fn selo(arte: &'static [u8]) -> Element<'static, Message> {
        icon::icon(icon::from_svg_bytes(arte).symbolic(true))
            .size(ICONE_ROTULO)
            .into()
    }

    fn grava_temperatura(&mut self, v: u32) {
        match self.conf.as_ref() {
            Some(c) => {
                if let Err(e) = self.chaves.set_leitura_temperatura(c, v) {
                    eprintln!("meow-applet-leitura: leitura_temperatura: {e}");
                }
            }
            None => self.chaves.leitura_temperatura = v,
        }
    }

    fn grava_textura(&mut self, v: f32) {
        match self.conf.as_ref() {
            Some(c) => {
                if let Err(e) = self.chaves.set_leitura_textura(c, v) {
                    eprintln!("meow-applet-leitura: leitura_textura: {e}");
                }
            }
            None => self.chaves.leitura_textura = v,
        }
    }

    /// Guarda o ponto ao qual o interruptor volta. Só grava valor ÚTIL: chamar
    /// isto com 0/0.0 apagaria justamente a memória que o desligar precisa.
    fn lembra_ponto(&mut self) {
        if self.chaves.leitura_temperatura != 0 {
            self.ultima_temp = self.chaves.leitura_temperatura;
        }
        if self.chaves.leitura_textura > 0.0 {
            self.ultima_textura = self.chaves.leitura_textura;
        }
        let par = (self.ultima_temp, self.ultima_textura);
        // A GUARDA É NOSSA PORQUE ESTE `set` NÃO TEM UMA.
        //   MEDIDO em 31/08/2026: o `set_<campo>` que o `CosmicConfigEntry`
        //   deriva já compara antes de escrever — `if self.campo != value`,
        //   cosmic-config-derive/src/lib.rs:169-177 — mas aqui não é o derivado,
        //   é o `ConfigSet::set` cru, que abre transação e chama `AtomicFile ->
        //   write_all` sem olhar o que havia (cosmic-config/src/lib.rs:492-497 e
        //   510-520). Sem esta linha, cada soltada de slider paga DUAS escritas
        //   atômicas (temporário + rename) para gravar o que já estava lá.
        if self.estado_gravado == Some(par) {
            return;
        }
        if let Some(e) = self.estado.as_ref() {
            let _ = e.set::<u32>("ultima_temperatura", par.0);
            let _ = e.set::<f32>("ultima_textura", par.1);
            self.estado_gravado = Some(par);
        }
    }
}

impl cosmic::Application for AppletLeitura {
    type Executor = cosmic::SingleThreadExecutor;
    type Flags = ();
    type Message = Message;
    const APP_ID: &'static str = ID;

    fn core(&self) -> &Core {
        &self.core
    }

    fn core_mut(&mut self) -> &mut Core {
        &mut self.core
    }

    fn init(core: Core, _flags: Self::Flags) -> (Self, Task<Action<Self::Message>>) {
        let conf = Config::new(NS_COMPOSITOR, NS_COMPOSITOR_V).ok();
        let estado = Config::new_state(NS_ESTADO, NS_ESTADO_V).ok();

        // `get_entry` devolve Err quando QUALQUER chave falta — o que é o caso
        // normal de uma máquina que nunca ligou o modo de leitura. O struct que
        // vem junto do erro já traz os padrões nos campos que faltaram, então é
        // ele que se usa; tratar isso como falha deixaria o applet nascer vazio
        // na primeira vez de todas.
        let chaves = conf
            .as_ref()
            .map(|c| match Chaves::get_entry(c) {
                Ok(v) => v,
                Err((_erros, v)) => v,
            })
            .unwrap_or_default();

        // AS DUAS FAIXAS SÃO ABERTAS NA PONTA QUE SIGNIFICA "DESLIGADO", E ISSO
        // ERA UM DEFEITO ATÉ 31/08/2026.
        //   O filtro do Kelvin era `PISO..=NEUTRO`, que deixa 6500 passar. Um
        //   estado com 6500 fazia o interruptor "Ligar" gravar
        //   `leitura_temperatura = 6500` — que é o NEUTRO. O `is_noop()` do
        //   patch exige `== 0` para pular o passe de postprocess, então a tela
        //   ficaria idêntica e o shader rodaria uma multiplicação neutra por
        //   quadro, para sempre, sem nada na tela para acusar: é a armadilha
        //   descrita no bloco "O TOPO DO SLIDER GRAVA 0" entrando pela porta dos
        //   fundos. O da textura deixava 0.0 passar, e aí "Ligar" religava a
        //   textura em zero — ou seja, não religava nada.
        //
        //   Nenhum dos dois é alcançável arrastando: o `Message::Temperatura`
        //   converte `>= NEUTRO` em 0 antes de lembrar, e o `Message::Textura`
        //   só lembra `> 0.0`. Chega-se lá por arquivo de estado editado à mão
        //   ou herdado de uma versão anterior — que é justamente o caso em que
        //   um filtro serve para alguma coisa.
        let ultima_temp = estado
            .as_ref()
            .and_then(|e| e.get::<u32>("ultima_temperatura").ok())
            .filter(|v| (PISO..NEUTRO).contains(v))
            .unwrap_or(PADRAO_K);
        let ultima_textura = estado
            .as_ref()
            .and_then(|e| e.get::<f32>("ultima_textura").ok())
            .filter(|v| *v > 0.0 && *v <= 1.0)
            .unwrap_or(PADRAO_TEXTURA);

        (
            Self {
                core,
                popup: None,
                chaves,
                conf,
                estado,
                ultima_temp,
                ultima_textura,
                arrastando: false,
                ultimo_toque: std::time::Instant::now(),
                // `None`, e não `Some((ultima_temp, ultima_textura))`: os dois
                // valores acima podem ter vindo do PADRÃO, não do disco (chave
                // ausente, ilegível ou fora de faixa). Declarar que já estão
                // gravados faria a primeira `lembra_ponto()` calar-se e o
                // arquivo de estado nunca nascer numa máquina nova.
                estado_gravado: None,
                compositor_le: compositor_vivo_le_as_chaves(),
            },
            Task::none(),
        )
    }

    fn on_close_requested(&self, id: window::Id) -> Option<Message> {
        Some(Message::PopupFechou(id))
    }

    fn update(&mut self, message: Message) -> Task<Action<Self::Message>> {
        match message {
            Message::AbreFecha => return self.abre_fecha(),
            Message::PopupFechou(id) => {
                if self.popup == Some(id) {
                    self.popup = None;
                    // O popup pode fechar com o dedo ainda apertado (ela clica
                    // fora). Sem esta linha o applet ficaria "arrastando" para
                    // sempre e pararia de aceitar o que o agendador escrevesse.
                    self.arrastando = false;
                }
            }
            Message::Ligar(ligar) => {
                // ESTE INTERRUPTOR É O DESFAZER, E É O ÚNICO — 31/08/2026.
                //   Havia um botão "Restaurar padrões" no pé do popup cujo braço
                //   era, linha por linha, o `else` daqui de baixo:
                //   `lembra_ponto(); grava_temperatura(0); grava_textura(0.0)`.
                //   Um segundo controle, com um nome que prometia devolver os
                //   3500 K e os 35% do `PADRAO_*`, fazendo exatamente o que o
                //   interruptor já fazia: desligar. Ela viu a medição e mandou
                //   tirar. Dois controles com o mesmo efeito e nomes diferentes
                //   não dão duas saídas — ensinam a duvidar das duas.
                if ligar {
                    let (t, x) = (self.ultima_temp, self.ultima_textura);
                    self.grava_temperatura(t);
                    self.grava_textura(x);
                } else {
                    self.lembra_ponto();
                    self.grava_temperatura(0);
                    self.grava_textura(0.0);
                }
            }
            Message::Temperatura(v) => {
                self.arrastando = true;
                self.ultimo_toque = std::time::Instant::now();
                // O TOPO É O DESLIGADO — ver o cabeçalho.
                let v = if v >= NEUTRO { 0 } else { v.max(PISO) };
                self.grava_temperatura(v);
                if v != 0 {
                    self.ultima_temp = v;
                }
            }
            Message::Textura(p) => {
                self.arrastando = true;
                self.ultimo_toque = std::time::Instant::now();
                let x = (p.min(100) as f32) / 100.0;
                self.grava_textura(x);
                if x > 0.0 {
                    self.ultima_textura = x;
                }
            }
            Message::Soltou => {
                self.arrastando = false;
                // O ponto só vai para o disco quando ela SOLTA. Gravar a cada
                // passo do arrasto seriam ~55 escritas no estado por travessia
                // do slider, para nada: o que interessa lembrar é onde o dedo
                // parou.
                self.lembra_ponto();
            }
            Message::Agendar(v) => {
                if let Some(c) = self.conf.as_ref() {
                    if let Err(e) = self.chaves.set_leitura_agenda(c, v) {
                        eprintln!("meow-applet-leitura: leitura_agenda: {e}");
                    }
                } else {
                    self.chaves.leitura_agenda = v;
                }
            }
            Message::HoraInicio(m) => {
                let hhmm = hhmm_de_minutos(m);
                if let Some(c) = self.conf.as_ref() {
                    if let Err(e) = self.chaves.set_leitura_hora_inicio(c, hhmm) {
                        eprintln!("meow-applet-leitura: leitura_hora_inicio: {e}");
                    }
                } else {
                    self.chaves.leitura_hora_inicio = hhmm;
                }
            }
            Message::HoraFim(m) => {
                let hhmm = hhmm_de_minutos(m);
                if let Some(c) = self.conf.as_ref() {
                    if let Err(e) = self.chaves.set_leitura_hora_fim(c, hhmm) {
                        eprintln!("meow-applet-leitura: leitura_hora_fim: {e}");
                    }
                } else {
                    self.chaves.leitura_hora_fim = hhmm;
                }
            }
            Message::DiscoMudou(chaves) => {
                // Ver os comentários dos campos `arrastando` e `ultimo_toque` e
                // o do `FREIO_ARRASTO`. O `elapsed` não é redundância do
                // `Soltou`: é o que garante que um `on_change` sem `on_release`
                // (os quatro gestos listados no `ultimo_toque`) não deixe o
                // applet surdo ao disco pelo resto da vida do popup. É AQUI que
                // o cursor pula quando o freio solta no meio de um arrasto
                // parado — custo assumido, e o porquê está no `FREIO_ARRASTO`.
                if self.arrastando && self.ultimo_toque.elapsed() < FREIO_ARRASTO {
                    return Task::none();
                }
                // Baixar a bandeira aqui, e não só no `Soltou`, é o que mantém o
                // campo honesto: a partir daqui o disco manda de novo, e dizer
                // "arrastando" seria mentir sobre o próprio estado.
                self.arrastando = false;
                self.chaves = chaves;
            }
            // Nada a fazer: o retorno desta função já pede um redesenho, e é o
            // redesenho que troca o sol pela lua. Ver o `subscription()`.
            Message::Minuto => {}
        }
        Task::none()
    }

    fn subscription(&self) -> Subscription<Self::Message> {
        // ESTE É O MOTIVO DE O SLIDER NÃO MENTIR ÀS 18:00
        //   O `scripts/leitura.sh`, disparado pelo meow-leitura.timer, escreve
        //   nas MESMAS duas chaves. Sem esta assinatura o popup mostraria para
        //   sempre o valor que ela arrastou de manhã, enquanto a tela já está no
        //   degrau da noite.
        //
        //   `watch_config` (e não `watch_state`): as chaves moram em ~/.config,
        //   não em ~/.local/state. Com `dbus-config` desligado no Cargo.toml,
        //   ele cai no vigia de ARQUIVO, que enxerga qualquer chave do
        //   diretório — inclusive as cinco que o cosmic-settings-daemon nunca
        //   ouviu falar.
        //
        // E O SEGUNDO FIO É O RELÓGIO, DESDE 31/08/2026
        //   O ícone da barra passou a contar a HORA (sol de dia, lua de noite),
        //   e sem um tique ele só trocaria quando alguma OUTRA coisa acordasse o
        //   applet. Na prática isso quase funcionaria — o `leitura.sh` escreve
        //   nas chaves às 18:00 e o vigia acima acorda —, mas só com o "Agendar"
        //   ligado: desligado, o desenho ficaria preso no sol até ela abrir o
        //   popup. Um ícone que mente sobre a hora é pior do que um ícone fixo.
        //
        //   60 s é o passo, e o atraso máximo é ele mesmo: a virada aparece na
        //   barra em menos de um minuto. É o mesmo período do
        //   `meow-leitura.timer`, e por um motivo parecido — não existe evento
        //   de "virou a hora" para assinar, então alguém tem de perguntar.
        //   `Message::Minuto` não lê disco, não escreve disco e não aloca: ele
        //   existe para o `view()` rodar.
        Subscription::batch([
            self.core
                .watch_config::<Chaves>(NS_COMPOSITOR)
                .map(|atualizacao| {
                    for e in &atualizacao.errors {
                        if !chave_apenas_ausente(e) {
                            eprintln!("meow-applet-leitura: lendo o disco: {e}");
                        }
                    }
                    Message::DiscoMudou(atualizacao.config)
                }),
            cosmic::iced::time::every(std::time::Duration::from_secs(60)).map(|_| Message::Minuto),
        ])
    }

    fn view(&self) -> Element<'_, Message> {
        // ERA `night-light-symbolic`, UM ÍCONE DE TEMA, ATÉ 31/08/2026
        //   O comentário que estava aqui dizia que "um ícone autoral entraria
        //   aqui, mas escolher o desenho é gosto dela, e gosto não se decide num
        //   commit de quem não é a dona". Estava certo, e o gosto foi decidido: ela viu a
        //   folha de 29/08 e aprovou o par sol/lua (`s3` e `r2`), e em 31/08
        //   pediu que o desenho seguisse a hora. É esse par que está aqui.
        //
        //   `icon_button_from_handle` e não `icon_button`: o segundo só sabe
        //   procurar por NOME no tema de ícones, e estes dois não moram em tema
        //   nenhum — moram dentro do binário (ver ARTE_LUA / ARTE_SOL). O
        //   dimensionamento é idêntico: os dois desembocam na mesma função, que
        //   pergunta ao libcosmic a altura da barra dela.
        self.core
            .applet
            .icon_button_from_handle(icon::from_svg_bytes(self.arte_da_barra()).symbolic(true))
            .on_press_down(Message::AbreFecha)
            .into()
    }

    fn view_window(&self, _id: Id) -> Element<'_, Message> {
        let ligado = self.ligado();

        // O SELO DO INTERRUPTOR É O MESMO DA BARRA, E ISSO É O DESENHO INTEIRO
        //   Quem abre o popup viu um ícone na barra um instante antes. Repetir
        //   exatamente aquele desenho na primeira linha é o que amarra as duas
        //   coisas: "este popup é daquele ícone". Se fosse outro qualquer, a
        //   primeira linha do popup seria uma segunda pergunta.
        let interruptor = padded_control(
            Row::new()
                .push(Self::selo(self.arte_da_barra()))
                .push(
                    toggler(ligado)
                        .on_toggle(Message::Ligar)
                        .label("Modo De Leitura".to_string())
                        .text_size(14)
                        .width(Length::Fill),
                )
                .spacing(8)
                .align_y(Alignment::Center),
        );

        // Com a chave em 0 o slider descansa no TOPO (6500K), que é onde
        // "desligado" e "neutro" são a mesma coisa. Pôr o cursor no fundo seria
        // desenhar 1000K — a tela mais quente possível — para dizer "apagado".
        let k = if self.chaves.leitura_temperatura == 0 {
            NEUTRO
        } else {
            self.chaves.leitura_temperatura.clamp(PISO, NEUTRO)
        };
        let rotulo_k = if self.chaves.leitura_temperatura == 0 {
            "Desligado".to_string()
        } else {
            format!("{k} K")
        };
        let temperatura = padded_control(
            Column::new()
                .push(
                    // O SOL fica na TEMPERATURA porque o slider inteiro é sobre
                    // ele: o topo da faixa (6500 K) é a luz do dia, e cada passo
                    // para baixo é sol se pondo. Ver o `rotulo_k`, que chama o
                    // topo de "Desligado" pela mesma razão.
                    Row::new()
                        .push(Self::selo(ARTE_SOL))
                        .push(text::body("Temperatura").width(Length::Fill))
                        .push(text::body(rotulo_k))
                        .spacing(8)
                        .align_y(Alignment::Center),
                )
                .push(
                    // O `.name()` NÃO É ENFEITE: o `spin_button` do horário
                    // recebe nome de a11y na própria assinatura (é o 2º
                    // argumento, e por isso os dois lá embaixo já tinham um),
                    // enquanto o `Slider` só ganha nome se alguém chamar
                    // `.name()` — MEDIDO em iced/widget/src/slider.rs:270-275,
                    // sob a feature `a11y`, que o nosso Cargo.toml liga. Sem
                    // isto o leitor de tela anunciava dois "controle
                    // deslizante" seguidos, sem dizer qual era qual, num popup
                    // cujo assunto inteiro é qual dos dois números se está
                    // mexendo.
                    Slider::new(PISO..=NEUTRO, k, Message::Temperatura)
                        .step(PASSO_K)
                        .name("Temperatura")
                        .on_release(Message::Soltou),
                )
                .spacing(4),
        );

        // Por CENTO, e não 0.0–1.0: o slider inteiro do iced trabalha em passos
        // exatos, e 5 pontos percentuais é um passo que se enxerga. Num slider
        // de f32 o `step(0.05)` acumula erro de ponto flutuante e a chave acaba
        // gravada como 0.35000002 — número que o `_leitura_mesmo_numero` do
        // leitura.sh existe para tolerar, mas que ninguém quer ver no arquivo.
        let pct = (self.chaves.leitura_textura.clamp(0.0, 1.0) * 100.0).round() as u32;
        let textura = padded_control(
            Column::new()
                .push(
                    // O LIVRO ABERTO na TEXTURA: o que este slider faz é a
                    // página — faixa dinâmica comprimida, dessaturação e grão.
                    // Era o `r1` da folha, "o mais seguro, lê em qualquer
                    // tamanho"; o preço lá era ser genérico demais para a barra,
                    // e aqui, com o rótulo ao lado, genérico é exatamente o
                    // certo.
                    Row::new()
                        .push(Self::selo(ARTE_LIVRO))
                        .push(text::body("Textura De Papel").width(Length::Fill))
                        .push(text::body(format!("{pct}%")))
                        .spacing(8)
                        .align_y(Alignment::Center),
                )
                .push(
                    Slider::new(0u32..=100u32, pct, Message::Textura)
                        .step(PASSO_TEXTURA)
                        .name("Textura De Papel")
                        .on_release(Message::Soltou),
                )
                .spacing(4),
        );

        // O LIVRO COM HALO no AGENDAR: é o desenho da luz que se acende sozinha
        // sobre a página, que é literalmente o que esta linha liga. Na folha ele
        // perdia a 16 px, onde os três raios encostam na capa e viram coroa —
        // e é por isso que ele está AQUI, no popup, e não na barra.
        let agendar = padded_control(
            Row::new()
                .push(Self::selo(ARTE_HALO))
                .push(
                    toggler(self.chaves.leitura_agenda)
                        .on_toggle(Message::Agendar)
                        .label("Agendar".to_string())
                        .text_size(14)
                        .width(Length::Fill),
                )
                .spacing(8)
                .align_y(Alignment::Center),
        );

        let ini = ponta_do_horario(&self.chaves.leitura_hora_inicio, PADRAO_INICIO);
        let fim = ponta_do_horario(&self.chaves.leitura_hora_fim, PADRAO_FIM);

        // O BLOCO DO HORÁRIO SÓ EXISTE QUANDO O "AGENDAR" ESTÁ LIGADO — 31/08/2026
        //
        //   O DEFEITO: com o Agendar desligado a linha continuava inteira, viva
        //   e clicável. Clicar no `+` gravava `leitura_hora_inicio` de verdade e
        //   nada acontecia — com `leitura_agenda = false` o `scripts/leitura.sh`
        //   devolve 4 e não escreve chave nenhuma (bloco "leitura_agenda" do
        //   cabeçalho dele). Um controle que aceita o clique e não faz nada é
        //   pior do que um controle ausente: ele ensina que o applet mente.
        //
        //   POR QUE SUMIR, E NÃO DESENHAR "DESABILITADO"
        //   Este rev do libcosmic não tem estado desabilitado para
        //   `spin_button`. MEDIDO em src/widget/spin_button.rs: não há
        //   `.disabled()`, e o `on_press` é obrigatório na assinatura; a ÚNICA
        //   forma de os botões saírem apagados é `value == min` / `value == max`
        //   (linhas 216-229), que é faixa colapsada, não intenção. Dava para
        //   passar `min = max = valor` e ganhar os dois `[−][+]` cinzas de
        //   graça — mas o rótulo do meio continuaria em contraste cheio: o
        //   `container_style` do próprio widget crava `text_color:
        //   Some(current_container.on)` (linhas 288-314) e nada de fora
        //   sobrescreve isso. Sobraria meia linha apagada com "18:00" aceso no
        //   meio dela, que é o pior dos três desenhos. E o truque deixaria uma
        //   armadilha para o dia em que o libcosmic ganhar um `.disabled()` de
        //   verdade.
        //
        //   SUMIR AINDA ENSINA DE QUEM A LINHA DEPENDE: ela aparece e some junto
        //   com o interruptor que está logo acima dela, no mesmo gesto. O custo
        //   é o popup encolher a altura de um controle ao desligar o Agendar;
        //   ele é ancorado no alto, sob a barra, então cresce e encolhe para
        //   BAIXO e nenhum controle troca de lugar debaixo do dedo dela.
        //
        // UMA LINHA SÓ: "das [18:00] às [07:00]".
        //
        // Eram duas, uma por horário, e cada uma empurrava o seu spin_button
        // para a borda direita com um `width(Length::Fill)` no rótulo. O
        // resultado era um popup alto com dois números longe das palavras que
        // os nomeiam — "das" na esquerda e "18:00" na direita, com 250px de
        // vazio no meio. Aqui a frase fica junta e se lê de uma vez.
        //
        // A CONTA DA LARGURA, PARA ISTO NÃO QUEBRAR NUMA TOPBAR ESTREITA
        //   O `spin_button` horizontal do libcosmic é [−][rótulo de 48px][+],
        //   ou seja ~120px com o container, mais 2px da borda que a `realce`
        //   desenha por fora (1px de cada lado). Dois deles (244) mais "Das"
        //   (~28), "Às" (~25), três espaços de 8 (24) e o padding do
        //   `padded_control` (32) dão ~353px — dentro do `max_width(400)` do
        //   popup, que é quem
        //   manda no tamanho final.
        //
        // O TETO DOS DOIS É `1440 - PASSO_MINUTO`, E ISSO É A GRADE, NÃO UM
        // CORTE: com passo de 15 o último ponto abaixo da meia-noite é 23:45, e
        // 23:59 nunca foi alcançável por clique. O `_leitura_minutos` do
        // leitura.sh aceita até 1439 e continua aceitando — quem escrever
        // "23:59" à mão vê o rótulo dizer 23:59 (ele vem do disco, não do valor
        // clampado), com o `+` apagado e o `−` caindo em 23:30. Mostrar 23:45
        // ali seria o applet mentir sobre o que o agendador vai usar.
        let bloco_horario: Option<Element<'_, Message>> = if self.chaves.leitura_agenda {
            // `das X às X` NÃO É "nunca", É O DIA INTEIRO — e até hoje só o
            // leitura.sh sabia. MEDIDO no `awk` que resolve a janela, o `BEGIN`
            // de dentro da `_leitura_alvo_agora` (scripts/leitura.sh): `if (ini
            // == fim) noite = 1`, palavra por palavra a mesma regra do
            // `e_noite()` do wallpaper.sh. Citado por NOME e não por número de
            // linha porque a citação anterior era um número, e ele apodreceu no
            // mesmo dia em que nasceu: o leitura.sh cresceu por cima e a linha
            // apontada virou outra coisa. Nome de função sobrevive à edição do
            // vizinho; número de linha em arquivo vivo não sobrevive a nada.
            // Com o passo de 15 minutos ela chega
            // nesse estado em dois cliques, e a tela ficaria âmbar às onze da
            // manhã sem uma linha aqui explicando por quê.
            let janela_cheia = (ini == fim).then(|| {
                padded_control(text::caption(
                    "Início e fim iguais: o modo de leitura vale o dia inteiro.",
                ))
            });

            Some(
                Column::new()
                    .push(padded_control(
                        Row::new()
                            .push(text::body("Das"))
                            .push(realce(spin_button(
                                hhmm_de_minutos(ini),
                                "hora de início",
                                ini,
                                PASSO_MINUTO,
                                0,
                                1440 - PASSO_MINUTO,
                                Message::HoraInicio,
                            )))
                            .push(text::body("Às"))
                            .push(realce(spin_button(
                                hhmm_de_minutos(fim),
                                "hora de fim",
                                fim,
                                PASSO_MINUTO,
                                0,
                                1440 - PASSO_MINUTO,
                                Message::HoraFim,
                            )))
                            .spacing(8)
                            .align_y(Alignment::Center),
                    ))
                    .push_maybe(janela_cheia)
                    .into(),
            )
        } else {
            None
        };

        // A frase que impede a pergunta "arrastei e não mudou nada". Ela só
        // aparece enquanto o compositor que está desenhando a tela AGORA não
        // souber ler as chaves — depois do logout com o binário patchado, some
        // sozinha. É `caption` e não `body` porque é uma nota de rodapé, não um
        // erro. O divisor vem COM ela e não antes dela: era o botão "Restaurar
        // padrões" que separava esta nota do resto, e com o botão fora um
        // divisor solto ficaria pendurado no pé do popup nos dias — quase todos,
        // desde 30/08 — em que o aviso não aparece.
        let aviso: Option<Element<'_, Message>> = (!self.compositor_le).then(|| {
            Column::new()
                .push(padded_control(divider::horizontal::default()))
                .push(padded_control(text::caption(
                    "O compositor desta sessão ainda não lê estas chaves: \
                     o valor fica gravado e a tela obedece no próximo login.",
                )))
                .into()
        });

        let conteudo = Column::new()
            .push(interruptor)
            .push(padded_control(divider::horizontal::default()))
            .push(temperatura)
            .push(textura)
            .push(padded_control(divider::horizontal::default()))
            .push(agendar)
            .push_maybe(bloco_horario)
            .push_maybe(aviso)
            .padding([8, 0]);

        self.core.applet.popup_container(conteudo).into()
    }

    fn style(&self) -> Option<cosmic::iced::theme::Style> {
        Some(cosmic::applet::style())
    }
}

impl AppletLeitura {
    fn abre_fecha(&mut self) -> Task<Action<Message>> {
        if let Some(popup) = self.popup.take() {
            return tarefa_de_superficie(destroy_popup(popup));
        }
        let Some(pai) = self.core.main_window_id() else {
            eprintln!("meow-applet-leitura: sem janela principal — popup não abre");
            return Task::none();
        };

        tarefa_de_superficie(app_popup::<AppletLeitura>(
            |_| Default::default(),
            move |estado: &mut AppletLeitura| {
                let popup = Id::unique();
                let mut ajustes = estado
                    .core
                    .applet
                    .get_popup_settings(pai, popup, None, None, None);
                // Largura fixa o suficiente para os dois sliders não virarem um
                // fio de 40px numa topbar estreita.
                ajustes.positioner.size_limits = Limits::NONE
                    .max_width(400.0)
                    .min_width(320.0)
                    .min_height(200.0)
                    .max_height(1080.0);
                estado.popup = Some(popup);
                ajustes
            },
            None,
        ))
    }
}

/// "Esta chave simplesmente não existe" é o caso NORMAL, não um erro digno de
/// journal: a máquina que nunca ligou o modo de leitura não tem nenhuma das
/// cinco, e o `get_entry` já devolveu o padrão para cada uma.
///
/// `Error::NotFound` NÃO BASTA, E ISSO FOI MEDIDO (30/08/2026)
///   A primeira versão filtrava só o `NotFound` e o applet escreveu três linhas
///   no log já na primeira execução:
///       meow-applet-leitura: lendo o disco: failed to get key
///       'leitura_hora_inicio': No such file or directory (os error 2)
///   O caminho real é outro: `ConfigGet::get` cai em `get_local`, que devolve
///   `NotFound`; o `get` então TENTA O PADRÃO DO SISTEMA em
///   `/usr/share/cosmic/<ns>/v1/<chave>`, que também não existe — e o erro que
///   sai é `GetKey(chave, ENOENT)`, não `NotFound`
///   (cosmic-config/src/lib.rs:449-485). Sem esta segunda perna, o applet
///   escreveria três linhas por evento de configuração, para sempre, sobre
///   arquivos que não têm por que existir.
fn chave_apenas_ausente(e: &cosmic_config::Error) -> bool {
    match e {
        cosmic_config::Error::NotFound => true,
        cosmic_config::Error::GetKey(_, io) => io.kind() == std::io::ErrorKind::NotFound,
        _ => false,
    }
}

/// O contorno que separa `[− 18:00 +]` do fundo do popup.
///
/// POR QUE ELE PRECISA EXISTIR AQUI, E NÃO SAI DE GRAÇA DO WIDGET
///   O `spin_button` do libcosmic já se embrulha num container próprio — mas o
///   `container_style` dele (`src/widget/spin_button.rs`, a `fn container_style`
///   do rev pinado no nosso Cargo.toml) só desenha borda quando o tema é de ALTO
///   CONTRASTE. No caminho normal, que é o dela, a largura é `0.0` e o fundo é
///   `None`: os dois grupos de botões flutuavam sem nada dizendo onde um começa
///   e o outro acaba. Foi o que ela apontou em 31/08/2026, olhando a barra —
///   "não conseguimos destacar os botões do agendamento? Talvez uma borda".
///
///   E não dá para reestilizar o container de DENTRO: a `class` é cravada pelo
///   widget na hora de virar `Element` e não há método para trocá-la depois.
///   Por isso o realce é um SEGUNDO container, por fora — e por isso ele é uma
///   função com nome, e não uma linha repetida duas vezes lá em cima.
///
/// `Container::Dropdown`, E NÃO UMA COR NOSSA
///   É o estilo que o próprio COSMIC usa em controle embutido em linha: fundo
///   `bg_component_color()`, borda de 1px em `bg_component_divider()` e o
///   `radius_s` do tema (`src/theme/style/iced.rs`). Escolher o estilo do tema
///   em vez de escrever um RGB nosso é o que faz o realce acompanhar o
///   Catppuccin dela quando ela trocar de sabor, e continuar legível se ela for
///   para o tema claro. Uma cor chumbada aqui seria bonita hoje e errada no dia
///   em que ela mudasse de acento — que é o dia em que ninguém lembra deste
///   arquivo.
fn realce<'a>(conteudo: impl Into<Element<'a, Message>>) -> Element<'a, Message> {
    container(conteudo).class(theme::Container::Dropdown).into()
}

fn tarefa_de_superficie(acao: cosmic::surface::Action) -> Task<Action<Message>> {
    cosmic::task::message(cosmic::Action::Cosmic(cosmic::app::Action::Surface(acao)))
}

/// O compositor QUE ESTÁ DESENHANDO A TELA AGORA sabe ler `leitura_*`?
///
/// A pergunta não é "o binário do disco tem o patch" — essa é outra, e as duas
/// divergem justamente no estado mais comum de todos: build feito, logout ainda
/// não. Por isso a sonda é em `/proc/<pid>/exe`, que é o binário do PROCESSO.
///
/// Erro de qualquer natureza responde `true`, e isso é de propósito: sem
/// certeza, o applet fica calado. Um aviso que aparece por engano ensina a
/// ignorar avisos.
fn compositor_vivo_le_as_chaves() -> bool {
    let Ok(dir) = std::fs::read_dir("/proc") else {
        return true;
    };
    for entrada in dir.flatten() {
        let nome = entrada.file_name();
        let Some(nome) = nome.to_str() else { continue };
        if !nome.bytes().all(|b| b.is_ascii_digit()) {
            continue;
        }
        let comm = std::fs::read_to_string(format!("/proc/{nome}/comm")).unwrap_or_default();
        if comm.trim() != "cosmic-comp" {
            continue;
        }
        // ABRIR `/proc/<pid>/exe`, E NÃO O CAMINHO QUE ELE APONTA — 31/08/2026
        //   A versão anterior fazia `read_link` e depois lia o caminho
        //   devolvido. São perguntas DIFERENTES: abrir a ligação mágica entrega
        //   o inode que o processo está executando, mesmo que o arquivo já
        //   tenha sido substituído ou apagado; ler o caminho entrega o que está
        //   no disco AGORA. As duas divergem exatamente no estado que esta
        //   sonda existe para pegar — build feito, logout ainda não. Depois de
        //   um `aurora-cosmic-comp-ws.sh --build`, o caminho já tem o marcador e
        //   o compositor vivo ainda não: a sonda velha responderia "ele lê" e o
        //   aviso do rodapé sumiria justo no dia em que ele é a única
        //   explicação para a tela não obedecer.
        //
        //   MEDIDO nesta máquina hoje: `wc -c < /proc/3060/exe` devolve
        //   33576448 rodando como ela mesma — o dono do processo lê o próprio
        //   `exe` sem privilégio nenhum, então a troca não custa permissão. E as
        //   duas leituras concordam agora (mesmo inode, 262538), que é como tem
        //   de ser fora da janela do build.
        let Ok(bytes) = std::fs::read(format!("/proc/{nome}/exe")) else {
            return true;
        };
        return bytes
            .windows(MARCADOR.len())
            .any(|janela| janela == MARCADOR);
    }
    // Nenhum cosmic-comp visível: não é hora de opinar.
    true
}

/// O marcador-base que o patch do compositor deixa no binário. Sem a versão no
/// fim (`-1`), porque a série da Aurora declara a base e o número muda a cada
/// edição do patch — casar com a base é o que faz esta sonda sobreviver à
/// próxima revisão sem ninguém lembrar de voltar aqui.
const MARCADOR: &[u8] = b"AURORA-READING-MODE";

fn main() -> cosmic::iced::Result {
    cosmic::applet::run::<AppletLeitura>(())
}
