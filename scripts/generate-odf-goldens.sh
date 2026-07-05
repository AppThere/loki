#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 AppThere Loki contributors
#
# Generates the ODF golden set (Spec 02 §7.2 / M4, scripted LibreOffice path):
#
#   fixture .odt  --soffice --headless-->  .pdf  --pinned rasterizer-->  page PNGs
#
# The rasterization stage is appthere-conformance's PdfRasterizer (pdftoppm at
# the fixed CONFORMANCE_DPI with pinned AA) — identical to every other path,
# so golden and candidate differ only in the layout/render engine (D3).
#
# Requirements: soffice (libreoffice), poppler-utils, and the bundled
# metric-compatible fonts installed for fontconfig (e.g. copy
# loki-fonts/fonts/*.ttf to ~/.fonts && fc-cache) so LibreOffice shapes with
# the same faces Loki bundles (D4).
#
# Output: appthere-conformance/goldens/odt/<stem>/page-N.png plus a
# GENERATION.txt metadata record (operator, LO + rasterizer versions, date).

set -euo pipefail
cd "$(dirname "$0")/.."

FIXTURES=appthere-conformance/fixtures/odt
GOLDENS=appthere-conformance/goldens/odt
command -v soffice >/dev/null || { echo "ERROR: soffice not found" >&2; exit 1; }

# Regenerate fixtures from source so they always match the checked-in producer.
cargo run -q -p loki-odf --example gen_conformance_fixtures

WORK=$(mktemp -d)
trap 'rm -rf "$WORK"' EXIT

mkdir -p "$GOLDENS"
LO_VERSION=$(soffice --version 2>/dev/null | head -1)

for odt in "$FIXTURES"/*.odt; do
    stem=$(basename "$odt" .odt)
    echo "==> $stem"
    soffice --headless --convert-to pdf --outdir "$WORK" "$odt" >/dev/null
    out="$GOLDENS/$stem"
    rm -rf "$out"
    mkdir -p "$out"
    RASTER_VERSION=$(cargo run -q -p appthere-conformance --example rasterize_pdf -- \
        "$WORK/$stem.pdf" "$out" page | head -1)
    {
        echo "fixture:    $stem.odt"
        echo "reference:  LibreOffice headless ($LO_VERSION)"
        echo "rasterizer: $RASTER_VERSION @ 144 dpi (CONFORMANCE_DPI), -aa yes -aaVector yes"
        echo "generated:  $(date -u +%Y-%m-%d)"
        echo "operator:   scripts/generate-odf-goldens.sh (scripted; Spec 02 §7.2)"
    } > "$out/GENERATION.txt"
done

echo "Goldens written to $GOLDENS"
