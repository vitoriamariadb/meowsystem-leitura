#!/bin/sh
# Compila e instala o applet para quem usa (padrão) ou sob outro prefixo:
#   ./install.sh                 -> ~/.local
#   ./install.sh /usr/local      -> o sistema inteiro (rode como root)
#   ./install.sh --remover [pfx] -> tira o binário e o .desktop
# O binário entra antes do .desktop e sai depois dele: um .desktop sem binário
# deixa um buraco na topbar.
set -e
id=io.github.meowsystem.Leitura
bin=meowsystem-leitura
cd "$(dirname "$0")"

if [ "${1:-}" = "--remover" ]; then
  prefix="${2:-$HOME/.local}"
  rm -f "$prefix/share/applications/$id.desktop"
  rm -f "$prefix/bin/$bin"
  echo "Removido de $prefix."
  exit 0
fi

prefix="${1:-$HOME/.local}"
cargo build --release --locked
alvo="${CARGO_TARGET_DIR:-target}"
install -Dm0755 "$alvo/release/$bin" "$prefix/bin/$bin"
mkdir -p "$prefix/share/applications"
sed "s|@bindir@|$prefix/bin|" "data/$id.desktop.in" > "$prefix/share/applications/$id.desktop"
echo "Instalado. Acrescente \"Modo de leitura\" em Configurações > Área de trabalho > Painel > Applets."
