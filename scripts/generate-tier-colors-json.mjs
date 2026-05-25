#!/usr/bin/env node
/**
 * Generate public/tier-colors.json from src/tiers.ts canonical values.
 * Run via (tsx):   node --import tsx/esm scripts/generate-tier-colors-json.mjs
 * Run via (Node 22+): node --experimental-strip-types scripts/generate-tier-colors-json.mjs
 * tsx option requires: pnpm add -D tsx
 */

import { writeFileSync } from "fs";
import { fileURLToPath } from "url";
import { dirname, join } from "path";
import { TIER_COLORS } from "../src/tiers.ts";

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);

const output = {
  version: 1,
  canonical_source: "src/tiers.ts",
  note: "These HSL values MUST be mirrored in niyora-web (OrbStage.astro) and niyora-companion (Techniques.swift)",
  colors: TIER_COLORS,
};

const outPath = join(__dirname, "..", "public", "tier-colors.json");
writeFileSync(outPath, JSON.stringify(output, null, 2) + "\n");
console.log(`✓ Generated ${outPath}`);
