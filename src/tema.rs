use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use cosmic::cosmic_theme::palette::{Srgb, Srgba};
use cosmic::cosmic_theme::{Component, Container, Theme as CosmicTheme, ThemeBuilder};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cor(pub [u8; 4]);

impl Cor {
    pub fn parse(texto: &str) -> Option<Cor> {
        let hex = texto.strip_prefix('#')?;
        if !(hex.len() == 6 || hex.len() == 8) || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
            return None;
        }
        let byte = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).ok();
        let alfa = if hex.len() == 8 { byte(6)? } else { 255 };
        Some(Cor([byte(0)?, byte(2)?, byte(4)?, alfa]))
    }

    pub fn srgba(self) -> Srgba {
        let [r, g, b, a] = self.0.map(|c| f32::from(c) / 255.0);
        Srgba::new(r, g, b, a)
    }

    pub fn srgb(self) -> Srgb {
        let [r, g, b, _] = self.0.map(|c| f32::from(c) / 255.0);
        Srgb::new(r, g, b)
    }
}

pub const CORES: [&str; 6] = ["acento", "destaque", "fundo", "borda", "texto", "bloco"];

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Tema {
    pub cores: BTreeMap<String, Cor>,
    pub origem: Option<PathBuf>,
    pub ignoradas: usize,
}

impl Tema {
    pub fn cor(&self, chave: &str) -> Option<Cor> {
        self.cores.get(chave).copied()
    }

    pub fn parse(texto: &str) -> Tema {
        let mut tema = Tema::default();
        for linha in texto.lines().map(str::trim) {
            if linha.is_empty() || linha.starts_with('#') {
                continue;
            }
            let par = linha.split_once('=').map(|(c, v)| (c.trim(), v.trim()));
            match par {
                Some((chave, valor)) if CORES.contains(&chave) => match Cor::parse(valor) {
                    Some(cor) => {
                        tema.cores.insert(chave.to_owned(), cor);
                    }
                    None => tema.ignoradas += 1,
                },
                _ => tema.ignoradas += 1,
            }
        }
        tema
    }

    pub fn load(app_id: &str) -> Tema {
        let Some(caminho) = caminho(app_id) else {
            return Tema::default();
        };
        match std::fs::read_to_string(&caminho) {
            Ok(texto) => Tema {
                origem: Some(caminho),
                ..Tema::parse(&texto)
            },
            Err(e) => {
                if e.kind() != std::io::ErrorKind::NotFound {
                    eprintln!("tema: {}: {e}", caminho.display());
                }
                Tema::default()
            }
        }
    }

    pub fn cosmic(&self, escuro: bool) -> Option<cosmic::Theme> {
        if self.cores.is_empty() {
            return None;
        }
        let mut builder = if escuro {
            ThemeBuilder::dark_config()
                .ok()
                .and_then(|h| cosmic::cosmic_config::CosmicConfigEntry::get_entry(&h).ok())
                .unwrap_or_else(ThemeBuilder::dark)
        } else {
            ThemeBuilder::light_config()
                .ok()
                .and_then(|h| cosmic::cosmic_config::CosmicConfigEntry::get_entry(&h).ok())
                .unwrap_or_else(ThemeBuilder::light)
        };
        if let Some(cor) = self.cor("acento") {
            builder = builder.accent(cor.srgb());
        }
        if let Some(cor) = self.cor("fundo") {
            builder = builder.bg_color(cor.srgba());
        }
        let mut theme = builder.build();
        self.cravar(&mut theme);
        Some(cosmic::Theme::custom(Arc::new(theme)))
    }

    pub fn cravar(&self, theme: &mut CosmicTheme) {
        if let Some(cor) = self.cor("acento") {
            theme.accent.base = cor.srgba();
            theme.accent_button.base = cor.srgba();
        }
        if let Some(cor) = self.cor("destaque") {
            theme.accent.on = cor.srgba();
            theme.accent_button.on = cor.srgba();
        }
        let mut fundos = [theme.background(false).clone(), theme.background(true).clone()];
        for fundo in &mut fundos {
            self.pintar_fundo(fundo);
        }
        if (fundos[0] != *theme.background(false) || fundos[1] != *theme.background(true))
            && let Some(novo) = trocar_fundos(theme, &fundos)
        {
            *theme = novo;
        }
    }

    fn pintar_fundo(&self, fundo: &mut Container) {
        if let Some(cor) = self.cor("fundo") {
            fundo.base = cor.srgba();
        }
        if let Some(cor) = self.cor("borda") {
            fundo.divider = cor.srgba();
            fundo.component.divider = cor.srgba();
        }
        if let Some(cor) = self.cor("texto") {
            fundo.on = cor.srgba();
            fundo.component.on = cor.srgba();
        }
        if let Some(cor) = self.cor("bloco") {
            pintar(&mut fundo.component, cor);
        }
    }
}

fn trocar_fundos(theme: &CosmicTheme, fundos: &[Container; 2]) -> Option<CosmicTheme> {
    let troca = || -> serde_json::Result<CosmicTheme> {
        let mut valor = serde_json::to_value(theme)?;
        valor["background"] = serde_json::to_value(&fundos[0])?;
        valor["transparent_background"] = serde_json::to_value(&fundos[1])?;
        serde_json::from_value(valor)
    };
    troca().inspect_err(|e| eprintln!("tema: o fundo não entrou: {e}")).ok()
}

fn pintar(componente: &mut Component, cor: Cor) {
    let on = componente.on;
    let mistura = |t: f32| {
        let c = cor.srgba();
        Srgba::new(
            c.red + (on.red - c.red) * t,
            c.green + (on.green - c.green) * t,
            c.blue + (on.blue - c.blue) * t,
            c.alpha,
        )
    };
    componente.base = cor.srgba();
    componente.hover = mistura(0.1);
    componente.pressed = mistura(0.2);
}

pub fn caminho(app_id: &str) -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("MEOWSYSTEM_TEMA").filter(|p| !p.is_empty()) {
        return Some(PathBuf::from(p));
    }
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .filter(|d| !d.is_empty())
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))?;
    Some(sob(&base, app_id))
}

fn sob(base: &Path, app_id: &str) -> PathBuf {
    base.join("meowsystem").join("extensoes").join(app_id).join("tema.conf")
}

#[cfg(test)]
mod testes {
    use super::*;
    use cosmic::cosmic_theme::palette::{encoding::Srgb as Codificacao, rgb::Rgba};

    static AMBIENTE: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn hex(c: Srgba) -> [u8; 3] {
        let c: Rgba<Codificacao, u8> = c.into_format();
        [c.red, c.green, c.blue]
    }

    #[test]
    fn cor_aceita_so_hex() {
        assert_eq!(Cor::parse("#cba6f7"), Some(Cor([0xcb, 0xa6, 0xf7, 255])));
        assert_eq!(Cor::parse("#1e1e2ecc"), Some(Cor([0x1e, 0x1e, 0x2e, 0xcc])));
        for ruim in ["cba6f7", "#cba6f", "#gggggg", "mauve", "#cba6f7a", "", "#ção12"] {
            assert_eq!(Cor::parse(ruim), None, "{ruim}");
        }
    }

    #[test]
    fn linha_ruim_e_pulada_sem_derrubar() {
        let tema = Tema::parse("acento = mauve\nnada = #ffffff\nsem igual\n# nota\n\ntexto=#ffffff\n");
        assert_eq!(tema.ignoradas, 3);
        assert_eq!(tema.cores.len(), 1);
        assert!(Tema::parse("").cosmic(true).is_none());
        assert!(Tema::parse("\u{0}\u{ff}###\n=\n==\n").cosmic(false).is_none());
    }

    #[test]
    fn cores_cravadas_sao_exatas() {
        let mut theme = ThemeBuilder::dark().build();
        Tema::parse("acento = #ff79c6\ndestaque = #282a36\nfundo = #282a36\ntexto = #f8f8f2\nbloco = #44475a\n")
            .cravar(&mut theme);
        assert_eq!(hex(theme.accent_color()), [0xff, 0x79, 0xc6]);
        assert_eq!(hex(theme.accent_button.base), [0xff, 0x79, 0xc6]);
        assert_eq!(hex(theme.accent_button.on), [0x28, 0x2a, 0x36]);
        for transparente in [false, true] {
            let fundo = theme.background(transparente);
            assert_eq!(hex(fundo.base), [0x28, 0x2a, 0x36]);
            assert_eq!(hex(fundo.on), [0xf8, 0xf8, 0xf2]);
            assert_eq!(hex(fundo.component.base), [0x44, 0x47, 0x5a]);
            assert_ne!(fundo.component.hover, fundo.component.base);
        }
    }

    #[test]
    fn caminho_usa_o_app_id() {
        assert_eq!(
            sob(Path::new("/tmp/cfg"), "io.github.meowsystem.Exemplo"),
            PathBuf::from("/tmp/cfg/meowsystem/extensoes/io.github.meowsystem.Exemplo/tema.conf")
        );
    }

    #[test]
    fn le_o_tema_conf_que_o_meowsystem_escreve_no_home() {
        let _trava = AMBIENTE.lock().unwrap_or_else(|e| e.into_inner());
        let app_id = crate::ID;
        let casa = std::env::temp_dir().join(format!("{app_id}-casa-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&casa);
        let arquivo = sob(&casa.join(".config"), app_id);
        std::fs::create_dir_all(arquivo.parent().unwrap()).unwrap();
        std::fs::write(
            &arquivo,
            "acento = #cba6f7\ndestaque = #1e1e2e\nfundo = #1e1e2e\nborda = #45475a\ntexto = #cdd6f4\nbloco = #313244\n",
        )
        .unwrap();
        let antes: Vec<_> = ["HOME", "XDG_CONFIG_HOME", "MEOWSYSTEM_TEMA"]
            .iter()
            .map(|n| (*n, std::env::var_os(n)))
            .collect();
        unsafe {
            std::env::set_var("HOME", &casa);
            std::env::remove_var("XDG_CONFIG_HOME");
            std::env::remove_var("MEOWSYSTEM_TEMA");
        }
        let tema = Tema::load(app_id);
        let sem = {
            unsafe { std::env::set_var("MEOWSYSTEM_TEMA", casa.join("nao-existe.conf")) };
            Tema::load(app_id)
        };
        for (nome, valor) in antes {
            unsafe {
                match valor {
                    Some(v) => std::env::set_var(nome, v),
                    None => std::env::remove_var(nome),
                }
            }
        }
        let _ = std::fs::remove_dir_all(&casa);
        assert_eq!(tema.origem, Some(arquivo));
        assert_eq!(tema.ignoradas, 0);
        assert_eq!(tema.cores.len(), CORES.len());
        assert!(tema.cosmic(true).is_some());
        assert_eq!(sem, Tema::default());
        assert!(sem.cosmic(true).is_none());
    }
}
