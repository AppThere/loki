#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 AppThere Loki contributors
#
# Generate a Golden PNG render from a PDF *printed from an office suite* — the
# manual-print half of the visual conformance axis (Spec 02 §7.2 / M4):
#
#   fixture .docx  --Word: Export to PDF-->  .pdf  --pinned rasterizer-->  page PNGs
#   fixture .odt   --LO:   Export to PDF-->  .pdf  --pinned rasterizer-->  page PNGs
#
# WHY THIS EXISTS ALONGSIDE generate-odf-goldens.sh
# -------------------------------------------------
# `generate-odf-goldens.sh` drives `soffice --headless --convert-to pdf`
# itself, so the LibreOffice/ODF golden is fully scripted. Microsoft Office,
# however, cannot be automated headlessly on Linux/CI, so its OOXML goldens
# must be produced on a machine that has Word/Excel: open the committed
# fixture, Export/Print to PDF at default settings, and feed THAT PDF here.
# This script therefore INGESTS a pre-printed PDF rather than producing one.
#
# It reuses the one pinned rasterizer for every golden — appthere-conformance's
# `PdfRasterizer` (`pdftoppm` at CONFORMANCE_DPI = 144 dpi with pinned AA), the
# exact stage generate-odf-goldens.sh and the candidate side both use — so the
# golden and candidate PNGs differ only in the layout/render engine (Spec 02
# D3), never in DPI, AA, or PNG encoder. That makes the resulting golden
# directly comparable to Loki's render via the same SSIM/ΔE differ the ODF
# goldens use.
#
# HOW TO PRINT THE REFERENCE PDF (do this once per fixture, on the ref app)
#   Microsoft Word / Excel (OOXML):
#     File → Export → Create PDF/XPS  (or  File → Save As → PDF), "Standard"
#     quality, default page setup. Do NOT use "Minimum size". One PDF per
#     fixture, page geometry as authored.
#   LibreOffice (ODF, when capturing a manual print instead of the scripted
#   path): File → Export As → Export as PDF, default settings.
#   Record the exact application + version in --reference so the golden's
#   provenance is data, not folklore (Spec 02 §7.4).
#
# Requirements: poppler-utils (`pdftoppm`) on PATH; the bundled
# metric-compatible fonts installed for the *reference* app so it shapes with
# the same faces Loki bundles (D4) — e.g. copy loki-fonts/fonts/*.ttf to the
# machine that runs Word/LibreOffice.
#
# Output: appthere-conformance/goldens/<format>/<stem>/page-N.png plus a
# GENERATION.txt provenance record (reference app+version, rasterizer version,
# source-PDF sha256, optional fixture sha256, date, operator).
#
# Usage:
#   scripts/generate-office-goldens.sh \
#       --format  docx \
#       --stem    tc-docx-001-line-spacing \
#       --pdf     ~/prints/tc-docx-001.pdf \
#       --reference "Microsoft 365 Word (2508, Build 16.0.18827.20164)" \
#       [--fixture appthere-conformance/fixtures/docx/tc-docx-001-line-spacing.docx] \
#       [--goldens-root appthere-conformance/goldens]
#
# --fixture is optional but recommended: it copies the exact source document
# into the fixtures tree next to the golden and records its checksum, so the
# candidate (which imports the fixture) and the golden (Office's render of that
# same fixture) can never silently drift apart.

set -euo pipefail
cd "$(dirname "$0")/.."

FORMAT=""
STEM=""
PDF=""
REFERENCE=""
FIXTURE=""
GOLDENS_ROOT="appthere-conformance/goldens"

usage() {
    sed -n '2,59p' "$0" | sed 's/^# \{0,1\}//'
    exit "${1:-0}"
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --format)        FORMAT="${2:-}"; shift 2 ;;
        --stem)          STEM="${2:-}"; shift 2 ;;
        --pdf)           PDF="${2:-}"; shift 2 ;;
        --reference)     REFERENCE="${2:-}"; shift 2 ;;
        --fixture)       FIXTURE="${2:-}"; shift 2 ;;
        --goldens-root)  GOLDENS_ROOT="${2:-}"; shift 2 ;;
        -h|--help)       usage 0 ;;
        *) echo "ERROR: unknown argument: $1" >&2; usage 2 ;;
    esac
done

die() { echo "ERROR: $*" >&2; exit 1; }

# --- Validate inputs (fail loudly, never silently skip) ----------------------
[[ -n "$FORMAT"    ]] || die "--format is required (docx|xlsx|odt|ods|pptx)"
[[ -n "$STEM"      ]] || die "--stem is required (the fixture identifier)"
[[ -n "$PDF"       ]] || die "--pdf is required (a PDF printed from the reference app)"
[[ -n "$REFERENCE" ]] || die "--reference is required — record the exact app+version so provenance is data, not folklore"

case "$FORMAT" in
    docx|xlsx|odt|ods|pptx) ;;
    *) die "unsupported --format '$FORMAT' (expected docx|xlsx|odt|ods|pptx)" ;;
esac

[[ -f "$PDF" ]] || die "PDF not found: $PDF"
# Cheap sanity check that the input really is a PDF, so a mis-pass surfaces here
# rather than as an opaque pdftoppm failure.
head -c 5 "$PDF" | grep -q '^%PDF-' || die "not a PDF (no %PDF- header): $PDF"

command -v pdftoppm >/dev/null || die "pdftoppm (poppler-utils) not found on PATH — install poppler-utils"

if [[ -n "$FIXTURE" ]]; then
    [[ -f "$FIXTURE" ]] || die "fixture not found: $FIXTURE"
    fixture_ext="${FIXTURE##*.}"
    [[ "$fixture_ext" == "$FORMAT" ]] \
        || die "fixture extension .$fixture_ext does not match --format $FORMAT"
fi

# --- Rasterize through the ONE pinned stage ---------------------------------
OUT="$GOLDENS_ROOT/$FORMAT/$STEM"
rm -rf "$OUT"
mkdir -p "$OUT"

echo "==> $FORMAT/$STEM  (reference: $REFERENCE)"
# rasterize_pdf prints the rasterizer version on line 1, then the page PNGs.
RASTER_VERSION=$(cargo run -q -p appthere-conformance --example rasterize_pdf -- \
    "$PDF" "$OUT" page | head -1)

PAGE_COUNT=$(find "$OUT" -maxdepth 1 -name 'page-*.png' | wc -l | tr -d ' ')
[[ "$PAGE_COUNT" -gt 0 ]] || die "rasterizer produced no page PNGs for $PDF"

PDF_SHA=$(sha256sum "$PDF" | cut -d' ' -f1)

# --- Record the source fixture alongside the golden (recommended) -----------
FIXTURE_LINE="fixture-source: (not supplied — pass --fixture to lock candidate↔golden)"
if [[ -n "$FIXTURE" ]]; then
    FIXTURES_DIR="appthere-conformance/fixtures/$FORMAT"
    mkdir -p "$FIXTURES_DIR"
    dest="$FIXTURES_DIR/$STEM.$FORMAT"
    cp "$FIXTURE" "$dest"
    FIXTURE_SHA=$(sha256sum "$dest" | cut -d' ' -f1)
    FIXTURE_LINE="fixture-source: $dest (sha256 $FIXTURE_SHA)"
    echo "    copied fixture -> $dest"
fi

# --- Provenance record (Spec 02 §7.2 / §7.4) --------------------------------
{
    echo "fixture:    $STEM.$FORMAT"
    echo "reference:  $REFERENCE (printed to PDF)"
    echo "rasterizer: $RASTER_VERSION @ 144 dpi (CONFORMANCE_DPI), -aa yes -aaVector yes"
    echo "pages:      $PAGE_COUNT"
    echo "source-pdf: sha256 $PDF_SHA"
    echo "$FIXTURE_LINE"
    echo "generated:  $(date -u +%Y-%m-%d)"
    echo "operator:   scripts/generate-office-goldens.sh (ingest printed PDF; Spec 02 §7.2)"
} > "$OUT/GENERATION.txt"

echo "Golden written to $OUT ($PAGE_COUNT page(s))"
