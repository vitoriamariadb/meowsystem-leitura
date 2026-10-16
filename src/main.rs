// meow-applet-leitura — os dois sliders do modo de leitura, na topbar dela.
//
// Desenha um ícone na barra; clicando, abre um popup com um interruptor, dois
// sliders (Temperatura em Kelvin, Textura em por cento) e um agendamento com
// hora de início e de fim.
//
// `leitura_temperatura` (u32, Kelvin, 0 = desligado) e `leitura_textura` (f32,
// 0.0..1.0) são os dois números que o shader lê por quadro — os tipos vêm do
// struct `CosmicCompConfig` do patch, conferidos linha a linha em
// `patches/cosmic-comp-modo-leitura.patch`.

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
/// leitura" saber ao que voltar.
const NS_ESTADO: &str = "com.meowsystem.AppletLeitura";
const NS_ESTADO_V: u64 = 1;

//   "faltou inserir os svgs na guia que abre e fazer o icon do applet mudar
//   conforme o horario do dia."
//
// São os do conjunto "modo de leitura" da folha
// (docs/pesquisas/2026-08-29-modo-leitura.html, aba "Os candidatos do modo de
// leitura").
//
// A folha previa instalá-los em `MeowSystem-Icons/scalable/status/`.
const ARTE_LUA: &[u8] = include_bytes!("../../../../packs/meowsystem/icones/sistema/leitura/r2.svg");
const ARTE_SOL: &[u8] = include_bytes!("../../../../packs/meowsystem/icones/sistema/leitura/s3.svg");
const ARTE_LIVRO: &[u8] = include_bytes!("../../../../packs/meowsystem/icones/sistema/leitura/r1.svg");
const ARTE_HALO: &[u8] = include_bytes!("../../../../packs/meowsystem/icones/sistema/leitura/r6.svg");

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
/// slider.
const FREIO_ARRASTO: std::time::Duration = std::time::Duration::from_secs(2);

/// Os padrões de quando ela liga o modo pela primeira vez.
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

/// O `ultima_*` era lido só ao nascer. A assinatura `watch_state` (ver o
/// `subscription()`) relê.
#[derive(Clone, Debug, Default, PartialEq, CosmicConfigEntry)]
#[version = 1]
pub struct Ponto {
    pub ultima_temperatura: u32,
    pub ultima_textura: f32,
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
            // vazio cai no ramo do relógio).
            leitura_agenda: true,
            leitura_hora_inicio: PADRAO_INICIO.to_string(),
            leitura_hora_fim: PADRAO_FIM.to_string(),
        }
    }
}

/// Devolve `None` em qualquer outra coisa, e o chamador cai no padrão em vez de
/// mostrar um horário inventado.
///
/// O item 3 do cabeçalho deste arquivo já declarava que o `_leitura_minutos` do
/// `scripts/leitura.sh` aceita DUAS formas — `"HH:MM"` e o inteiro de minutos
/// desde a meia-noite — e o cabeçalho do leitura.sh repete que a segunda existe
/// justamente para ela poder abrir o arquivo e escrever `1080` à mão.
fn minutos_de_hhmm(v: &str) -> Option<i32> {
    if let Some((h, m)) = v.split_once(':') {
        // O `case` do shell é `[0-9][0-9]:[0-9][0-9]|[0-9]:[0-9][0-9]`, e
        // padrão de `case` casa a palavra INTEIRA — contar os dígitos aqui é a
        // tradução literal disso, e é o que recusa "018:00" e "18:0".
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
    // O `+` passava inteiro; o `-` só não passava porque o `(0..=1439)`
    // recusava o negativo DEPOIS.
    if v.is_empty() || !v.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    // `i64` e não `i32` para o veredito ser "maior que 1439" e não "não coube",
    // como no shell: `$((10#…))` do bash é intmax_t, e um `parse::<i32>()`
    // recusaria "9999999999" por transbordo (medido).
    let m: i64 = v.parse().ok()?;
    (m <= 1439).then_some(m as i32)
}

/// Uma ponta do horário, resolvida como o `_leitura_uma_ponta` do
/// `scripts/leitura.sh` a resolve: o valor do disco quando ele PASSA no filtro,
/// o padrão quando não.
fn ponta_do_horario(bruto: &str, padrao: &str) -> i32 {
    minutos_de_hhmm(bruto.trim())
        .or_else(|| minutos_de_hhmm(padrao))
        .unwrap_or(0)
}

/// Que horas são, em minutos desde a meia-noite, NO FUSO DELA.
///
/// O `jiff` já está na árvore desta compilação — ele entra pelo libcosmic, e o
/// `Cargo.lock` ao lado o registra desde antes desta função existir.
///
/// A janela do agendamento é escrita em hora LOCAL — ela arrasta "das 18:00 às
/// 07:00" olhando o relógio da barra —, e nesta máquina isso são três horas de
/// diferença.
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
    /// ATRASADO — vem de um watcher de inotify atravessando um canal.
    arrastando: bool,
    /// Quando chegou o último `on_change` de slider. É o FREIO DE MÃO do
    /// `arrastando` acima, e existe por um defeito medido — não por zelo.
    ultimo_toque: std::time::Instant,
    /// Sem ele o `lembra_ponto()` reescreve os mesmos dois números a cada
    /// soltada de slider — ver a guarda lá embaixo.
    estado_gravado: Option<(u32, f32)>,
    /// Respondido UMA VEZ, no `init`, e nunca mais.
    ///
    /// A sonda lê 33 MB de binário do disco; o `view_window` é chamado a cada
    /// quadro do arrasto.
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
    /// que a assinatura entrega.
    DiscoMudou(Chaves),
    /// O estado mudou por fora (o `meow leitura adotar`): o ponto ao qual o
    /// interruptor volta passa a ser o de lá.
    EstadoMudou(Ponto),
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
    /// O `BEGIN` do `_leitura_alvo_agora` (scripts/leitura.sh) e o `e_noite()`
    /// do `wallpaper.sh` decidem assim, e a igualdade entre os três é o que
    /// impede esta máquina de ter duas noites:
    ///
    ///       ini == fim  ->  é noite o dia inteiro
    ///       ini >  fim  ->  a janela atravessa a meia-noite
    ///       ini <  fim  ->  janela normal, dentro do mesmo dia
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
        // Sem esta linha, cada soltada de slider paga DUAS escritas atômicas
        // (temporário + rename) para gravar o que já estava lá.
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
        // normal de uma máquina que nunca ligou o modo de leitura.
        let chaves = conf
            .as_ref()
            .map(|c| match Chaves::get_entry(c) {
                Ok(v) => v,
                Err((_erros, v)) => v,
            })
            .unwrap_or_default();

        // O filtro do Kelvin era `PISO..=NEUTRO`, que deixa 6500 passar. Um
        // estado com 6500 fazia o interruptor "Ligar" gravar
        // `leitura_temperatura = 6500` — que é o NEUTRO.
        //
        // Nenhum dos dois é alcançável arrastando: o `Message::Temperatura`
        // converte `>= NEUTRO` em 0 antes de lembrar, e o `Message::Textura` só
        // lembra `> 0.0`.
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
                // ausente, ilegível ou fora de faixa).
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
                // Havia um botão "Restaurar padrões" no pé do popup cujo braço
                // era, linha por linha, o `else` daqui de baixo:
                // `lembra_ponto(); grava_temperatura(0); grava_textura(0.0)`.
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
                // O ponto só vai para o disco quando ela SOLTA.
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
                // o do `FREIO_ARRASTO`.
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
            // O MESMO FILTRO DO NASCIMENTO (ver o `init`): fora de faixa, ou a
            // chave ausente (o `Default` dá 0), não troca o ponto que já há.
            Message::EstadoMudou(p) => {
                let temp_ok = (PISO..NEUTRO).contains(&p.ultima_temperatura);
                let textura_ok = p.ultima_textura > 0.0 && p.ultima_textura <= 1.0;
                if temp_ok {
                    self.ultima_temp = p.ultima_temperatura;
                }
                if textura_ok {
                    self.ultima_textura = p.ultima_textura;
                }
                // Só o par que VEIO DO DISCO conta como gravado.
                if temp_ok && textura_ok {
                    self.estado_gravado = Some((self.ultima_temp, self.ultima_textura));
                }
            }
        }
        Task::none()
    }

    fn subscription(&self) -> Subscription<Self::Message> {
        // O `scripts/leitura.sh`, disparado pelo meow-leitura.timer, escreve
        // nas MESMAS duas chaves.
        //
        // `watch_config` (e não `watch_state`): as chaves moram em ~/.config,
        // não em ~/.local/state.
        //
        // 60 s é o passo, e o atraso máximo é ele mesmo: a virada aparece na
        // barra em menos de um minuto.
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
            // O TERCEIRO FIO É O ESTADO. `watch_state`, porque o `ultima_*`
            // mora em ~/.local/state; o `meow leitura adotar` o grava, e o
            // interruptor tem de religar no ponto de lá.
            self.core
                .watch_state::<Ponto>(NS_ESTADO)
                .map(|atualizacao| Message::EstadoMudou(atualizacao.config)),
        ])
    }

    fn view(&self) -> Element<'_, Message> {
        // O comentário que estava aqui dizia que "um ícone autoral entraria
        // aqui, mas escolher o desenho é gosto dela, e gosto não se decide num
        // commit de quem não é a dona".
        //
        // `icon_button_from_handle` e não `icon_button`: o segundo só sabe
        // procurar por NOME no tema de ícones, e estes dois não moram em tema
        // nenhum — moram dentro do binário (ver ARTE_LUA / ARTE_SOL).
        self.core
            .applet
            .icon_button_from_handle(icon::from_svg_bytes(self.arte_da_barra()).symbolic(true))
            .on_press_down(Message::AbreFecha)
            .into()
    }

    fn view_window(&self, _id: Id) -> Element<'_, Message> {
        let ligado = self.ligado();

        // Quem abre o popup viu um ícone na barra um instante antes. Repetir
        // exatamente aquele desenho na primeira linha é o que amarra as duas
        // coisas: "este popup é daquele ícone".
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
                    // ele: o topo da faixa (6500 K) é a luz do dia, e cada
                    // passo para baixo é sol se pondo.
                    Row::new()
                        .push(Self::selo(ARTE_SOL))
                        .push(text::body("Temperatura").width(Length::Fill))
                        .push(text::body(rotulo_k))
                        .spacing(8)
                        .align_y(Alignment::Center),
                )
                .push(
                    // Sem isto o leitor de tela anunciava dois "controle
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
        // exatos, e 5 pontos percentuais é um passo que se enxerga.
        let pct = (self.chaves.leitura_textura.clamp(0.0, 1.0) * 100.0).round() as u32;
        let textura = padded_control(
            Column::new()
                .push(
                    // O LIVRO ABERTO na TEXTURA: o que este slider faz é a
                    // página — faixa dinâmica comprimida, dessaturação e grão.
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
        // sobre a página, que é literalmente o que esta linha liga.
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

        // O DEFEITO: com o Agendar desligado a linha continuava inteira, viva e
        // clicável.
        //
        // POR QUE SUMIR, E NÃO DESENHAR "DESABILITADO" Este rev do libcosmic
        // não tem estado desabilitado para `spin_button`.
        //
        // SUMIR AINDA ENSINA DE QUEM A LINHA DEPENDE: ela aparece e some junto
        // com o interruptor que está logo acima dela, no mesmo gesto.
        let bloco_horario: Option<Element<'_, Message>> = if self.chaves.leitura_agenda {
            // == fim) noite = 1`, palavra por palavra a mesma regra do
            // Nome de função sobrevive à edição do vizinho; número de linha em
            // arquivo vivo não sobrevive a nada.
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

        // A frase que impede a pergunta "arrastei e não mudou nada".
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
fn chave_apenas_ausente(e: &cosmic_config::Error) -> bool {
    match e {
        cosmic_config::Error::NotFound => true,
        cosmic_config::Error::GetKey(_, io) => io.kind() == std::io::ErrorKind::NotFound,
        _ => false,
    }
}

/// O contorno que separa `[− 18:00 +]` do fundo do popup.
///
/// O `spin_button` do libcosmic já se embrulha num container próprio — mas o
/// `container_style` dele (`src/widget/spin_button.rs`, a `fn container_style`
/// do rev pinado no nosso Cargo.toml) só desenha borda quando o tema é de ALTO
/// CONTRASTE.
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
/// não.
///
/// Erro de qualquer natureza responde `true`, e isso é de propósito: sem
/// certeza, o applet fica calado.
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
        // São perguntas DIFERENTES: abrir a ligação mágica entrega o inode que
        // o processo está executando, mesmo que o arquivo já tenha sido
        // substituído ou apagado; ler o caminho entrega o que está no disco
        // AGORA.
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

/// O marcador-base que o patch do compositor deixa no binário.
const MARCADOR: &[u8] = b"AURORA-READING-MODE";

fn main() -> cosmic::iced::Result {
    cosmic::applet::run::<AppletLeitura>(())
}
