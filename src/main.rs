// meow-applet-leitura — os dois sliders do modo de leitura, na topbar dela.
//
// O QUE ELE FAZ, EM UMA FRASE
//   Desenha um ícone na barra; clicando, abre um popup com um interruptor, dois
//   sliders (Temperatura em Kelvin, Textura em por cento), um agendamento com
//   hora de início e de fim, e um botão "Restaurar padrões". Cada arrasto GRAVA
//   uma chave em `~/.config/cosmic/com.system76.CosmicComp/v1/`. Quem PINTA é o
//   `cosmic-comp` recompilado com o patch do modo de leitura — este binário não
//   fala protocolo Wayland nenhum, não desenha um pixel na tela dela fora do
//   próprio popup, e não sabe o que é gamma.
//
// ============================================================================
// O QUE ESTAVA MEDIDO NESTA MÁQUINA EM 30/08/2026, ANTES DA PRIMEIRA LINHA
// ============================================================================
//
// 1. O COMPOSITOR DO DISCO AINDA NÃO SABE LER ESTAS CHAVES
//      strings -a /usr/bin/cosmic-comp | grep -c AURORA-READING-MODE  ->  0
//      /var/lib/aurora/cosmic-comp-patches.estado (01:46 de hoje) declara só
//      `req AURORA-COSMIC-WS-PATCH-3.67` e `opt AURORA-COSMIC-RADIUS-PATCH-1`.
//   O `patches/cosmic-comp-modo-leitura.patch` existe e está na `series`, mas
//   os dois são de DEPOIS do build (02:13 e 02:29 contra 01:46). Ou seja: a
//   etapa 1 escreveu o patch e ainda não recompilou.
//
//   ISSO NÃO IMPEDE ESTE APPLET DE FUNCIONAR, E É IMPORTANTE ENTENDER POR QUÊ.
//   Ele grava CHAVE, não pixel. As chaves ficam gravadas, conferíveis e certas
//   desde hoje; a tela obedece quando o compositor patchado subir. Enquanto
//   isso, arrastar o slider é uma operação silenciosa — e é o `meow leitura
//   estado` que diz, em uma linha, se o binário do disco e a sessão viva já
//   sabem ler. Um applet que fingisse ter mexido na tela seria pior.
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
use cosmic::applet::{menu_button, padded_control};
use cosmic::cosmic_config::cosmic_config_derive::CosmicConfigEntry;
use cosmic::cosmic_config::{self, Config, ConfigGet, ConfigSet, CosmicConfigEntry};
use cosmic::iced::{Alignment, Length, Limits, Subscription, window, window::Id};
use cosmic::surface::action::{app_popup, destroy_popup};
use cosmic::widget::{Column, Row, Slider, divider, spin_button, text, toggler};
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
fn minutos_de_hhmm(v: &str) -> Option<i32> {
    let v = v.trim();
    let (h, m) = v.split_once(':')?;
    let h: i32 = h.trim().parse().ok()?;
    let m: i32 = m.trim().parse().ok()?;
    if !(0..=23).contains(&h) || !(0..=59).contains(&m) {
        return None;
    }
    Some(h * 60 + m)
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
    Restaurar,
    /// Carrega o `Chaves` já extraído, e NÃO o `cosmic_config::Update<Chaves>`
    /// que a assinatura entrega. MEDIDO ao compilar em 30/08/2026: o `Update`
    /// do libcosmic 1f6dc99 não implementa `Clone`, e a `Message` de uma
    /// `cosmic::Application` tem de ser `Clone`. Os erros que vinham no
    /// `Update` são consumidos onde nascem — no `map` da assinatura.
    DiscoMudou(Chaves),
}

impl AppletLeitura {
    /// `true` quando qualquer um dos dois números está acima de zero. Não é
    /// "temperatura != 0": a textura sozinha também acende o passe do shader, e
    /// um interruptor que dissesse "desligado" com a textura em 40% mentiria.
    fn ligado(&self) -> bool {
        self.chaves.leitura_temperatura != 0 || self.chaves.leitura_textura > 0.0
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
        if let Some(e) = self.estado.as_ref() {
            let _ = e.set::<u32>("ultima_temperatura", self.ultima_temp);
            let _ = e.set::<f32>("ultima_textura", self.ultima_textura);
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

        let ultima_temp = estado
            .as_ref()
            .and_then(|e| e.get::<u32>("ultima_temperatura").ok())
            .filter(|v| (PISO..=NEUTRO).contains(v))
            .unwrap_or(PADRAO_K);
        let ultima_textura = estado
            .as_ref()
            .and_then(|e| e.get::<f32>("ultima_textura").ok())
            .filter(|v| (0.0..=1.0).contains(v))
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
                // O TOPO É O DESLIGADO — ver o cabeçalho.
                let v = if v >= NEUTRO { 0 } else { v.max(PISO) };
                self.grava_temperatura(v);
                if v != 0 {
                    self.ultima_temp = v;
                }
            }
            Message::Textura(p) => {
                self.arrastando = true;
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
            Message::Restaurar => {
                // ZERA OS DOIS, e mais nada. O horário e o "Agendar" ficam de
                // pé de propósito: "restaurar padrões" aqui quer dizer "apaga o
                // efeito agora", não "esquece a rotina que eu montei". Quem
                // desmonta a rotina é o interruptor Agendar, que está logo ali.
                self.lembra_ponto();
                self.grava_temperatura(0);
                self.grava_textura(0.0);
            }
            Message::DiscoMudou(chaves) => {
                if self.arrastando {
                    // Ver o comentário do campo `arrastando`.
                    return Task::none();
                }
                self.chaves = chaves;
            }
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
        self.core
            .watch_config::<Chaves>(NS_COMPOSITOR)
            .map(|atualizacao| {
                for e in &atualizacao.errors {
                    if !chave_apenas_ausente(e) {
                        eprintln!("meow-applet-leitura: lendo o disco: {e}");
                    }
                }
                Message::DiscoMudou(atualizacao.config)
            })
    }

    fn view(&self) -> Element<'_, Message> {
        // `night-light-symbolic` existe no Papirus E no Adwaita desta máquina
        // (conferido em 30/08/2026), então o ícone não some se ela trocar de
        // tema de ícones. Um ícone autoral entraria aqui, mas escolher o
        // desenho é gosto dela, e gosto não se decide num commit de frente.
        self.core
            .applet
            .icon_button("night-light-symbolic")
            .on_press_down(Message::AbreFecha)
            .into()
    }

    fn view_window(&self, _id: Id) -> Element<'_, Message> {
        let ligado = self.ligado();

        let interruptor = padded_control(
            toggler(ligado)
                .on_toggle(Message::Ligar)
                .label("Modo de leitura".to_string())
                .text_size(14)
                .width(Length::Fill),
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
            "desligado".to_string()
        } else {
            format!("{k} K")
        };
        let temperatura = padded_control(
            Column::new()
                .push(
                    Row::new()
                        .push(text::body("Temperatura").width(Length::Fill))
                        .push(text::body(rotulo_k))
                        .align_y(Alignment::Center),
                )
                .push(
                    Slider::new(PISO..=NEUTRO, k, Message::Temperatura)
                        .step(PASSO_K)
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
                    Row::new()
                        .push(text::body("Textura de papel").width(Length::Fill))
                        .push(text::body(format!("{pct}%")))
                        .align_y(Alignment::Center),
                )
                .push(
                    Slider::new(0u32..=100u32, pct, Message::Textura)
                        .step(PASSO_TEXTURA)
                        .on_release(Message::Soltou),
                )
                .spacing(4),
        );

        let agendar = padded_control(
            toggler(self.chaves.leitura_agenda)
                .on_toggle(Message::Agendar)
                .label("Agendar".to_string())
                .text_size(14)
                .width(Length::Fill),
        );

        let ini = minutos_de_hhmm(&self.chaves.leitura_hora_inicio)
            .unwrap_or_else(|| minutos_de_hhmm(PADRAO_INICIO).unwrap_or(0));
        let fim = minutos_de_hhmm(&self.chaves.leitura_hora_fim)
            .unwrap_or_else(|| minutos_de_hhmm(PADRAO_FIM).unwrap_or(0));

        let linha_ini = padded_control(
            Row::new()
                .push(text::body("das").width(Length::Fill))
                .push(spin_button(
                    hhmm_de_minutos(ini),
                    "hora de início",
                    ini,
                    PASSO_MINUTO,
                    0,
                    1440 - PASSO_MINUTO,
                    Message::HoraInicio,
                ))
                .align_y(Alignment::Center),
        );
        let linha_fim = padded_control(
            Row::new()
                .push(text::body("às").width(Length::Fill))
                .push(spin_button(
                    hhmm_de_minutos(fim),
                    "hora de fim",
                    fim,
                    PASSO_MINUTO,
                    0,
                    1440 - PASSO_MINUTO,
                    Message::HoraFim,
                ))
                .align_y(Alignment::Center),
        );

        // A frase que impede a pergunta "arrastei e não mudou nada". Ela só
        // aparece enquanto o compositor que está desenhando a tela AGORA não
        // souber ler as chaves — depois do logout com o binário patchado, some
        // sozinha. É `caption` e não `body` porque é uma nota de rodapé, não um
        // erro.
        let aviso = (!self.compositor_le).then(|| {
            padded_control(text::caption(
                "O compositor desta sessão ainda não lê estas chaves: \
                 o valor fica gravado e a tela obedece no próximo login.",
            ))
        });

        let conteudo = Column::new()
            .push(interruptor)
            .push(padded_control(divider::horizontal::default()))
            .push(temperatura)
            .push(textura)
            .push(padded_control(divider::horizontal::default()))
            .push(agendar)
            .push(linha_ini)
            .push(linha_fim)
            .push(padded_control(divider::horizontal::default()))
            .push(menu_button(text::body("Restaurar padrões")).on_press(Message::Restaurar))
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
        // `/proc/<pid>/exe` é um link para o binário; ler o link é barato e não
        // exige permissão de leitura do processo alheio, mas ler o ARQUIVO
        // apontado exige só a permissão do arquivo — que é 0755.
        let Ok(caminho) = std::fs::read_link(format!("/proc/{nome}/exe")) else {
            return true;
        };
        let Ok(bytes) = std::fs::read(&caminho) else {
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
