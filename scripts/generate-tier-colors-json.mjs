#!/usr/bin/env node
/**
 * Generate public/tier-colors.json from src/tiers.ts canonical values.
 * Run via: node scripts/generate-tier-colors-json.mjs
 */

import { writeFileSync } from "fs";
import { fileURLToPath } from "url";
import { dirname, join } from "path";

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);

const TIER_COLORS = {
  spark:      { hue: 30,  saturation: 70, lightness: 60 },
  glow:       { hue: 335, saturation: 70, lightness: 60 },
  shine:      { hue: 280, saturation: 65, lightness: 60 },
  radiance:   { hue: 230, saturation: 65, lightness: 60 },
  brilliance: { hue: 210, saturation: 60, lightness: 60 },
};

const output = {
  version: 1,
  canonical_source: "src/tiers.ts",
  note: "These HSL values MUST be mirrored in niyora-web (OrbStage.astro) and niyora-companion (Techniques.swift)",
  colors: TIER_COLORS,
};

const outPath = join(__dirname, "..", "public", "tier-colors.json");
writeFileSync(outPath, JSON.stringify(output, null, 2) + "\n");
console.log(`✓ Generated ${outPath}`);
