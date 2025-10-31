#!/usr/bin/env bash

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SDK_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
SRC_DIR="$SDK_ROOT/resources/icons/svg"
DST_DIR="$SDK_ROOT/resources/icons/pdf"

if [[ ! -d "$SRC_DIR" ]]; then
    echo "[convert_svg_icons] Source directory not found: $SRC_DIR" >&2
    exit 1
fi

mkdir -p "$DST_DIR"

convert_with_rsvg() {
    local input="$1"
    local output="$2"
    rsvg-convert -f pdf -o "$output" "$input"
}

convert_with_inkscape() {
    local input="$1"
    local output="$2"
    inkscape --export-type=pdf --export-filename="$output" "$input" >/dev/null
}

convert_with_cairosvg() {
    local input="$1"
    local output="$2"
    python3 -c "import cairosvg; cairosvg.svg2pdf(url='$input', write_to='$output')" >/dev/null
}

if command -v rsvg-convert >/dev/null 2>&1; then
    converter="rsvg"
elif command -v inkscape >/dev/null 2>&1; then
    converter="inkscape"
elif command -v python3 >/dev/null 2>&1 && python3 -c "import cairosvg" >/dev/null 2>&1; then
    converter="cairosvg"
else
    cat >&2 <<'EOF'
[convert_svg_icons] Error: No SVG->PDF converter detected.
Install one of the following tools and re-run the build:
  * librsvg  (recommended) : brew install librsvg
  * Inkscape               : brew install --cask inkscape
  * CairoSVG (Python)      : pip3 install cairosvg
EOF
    exit 1
fi

shopt -s nullglob
converted_any=false
for svg_file in "$SRC_DIR"/*.svg; do
    base_name="$(basename "$svg_file" .svg)"
    pdf_file="$DST_DIR/$base_name.pdf"
    echo "[convert_svg_icons] Converting $base_name.svg -> $base_name.pdf using $converter"
    case "$converter" in
        rsvg) convert_with_rsvg "$svg_file" "$pdf_file" ;;
        inkscape) convert_with_inkscape "$svg_file" "$pdf_file" ;;
        cairosvg) convert_with_cairosvg "$svg_file" "$pdf_file" ;;
    esac
    converted_any=true
done
shopt -u nullglob

if [[ "$converted_any" == false ]]; then
    echo "[convert_svg_icons] No SVG files found in $SRC_DIR"
fi
