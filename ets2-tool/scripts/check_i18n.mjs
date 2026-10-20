import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("../", import.meta.url));
const flatten = (value, prefix = "", result = {}) => {
  for (const [name, entry] of Object.entries(value)) {
    const key = prefix ? `${prefix}.${name}` : name;
    if (entry && typeof entry === "object") flatten(entry, key, result);
    else result[key] = entry;
  }
  return result;
};
const files = (directory) => fs.readdirSync(directory, { withFileTypes: true })
  .flatMap((entry) => entry.isDirectory() ? files(path.join(directory, entry.name)) : [path.join(directory, entry.name)]);
const localeDir = path.join(root, "src-tauri/locales");
const locales = Object.fromEntries(fs.readdirSync(localeDir).filter((name) => name.endsWith(".json"))
  .map((name) => [name.slice(0, -5), flatten(JSON.parse(fs.readFileSync(path.join(localeDir, name), "utf8")))]));
const master = locales.en;
const referenced = new Set();
const errors = [];
const warnings = [];
const placeholders = (text) => [...String(text).matchAll(/\{([A-Za-z_][\w]*)\}/g)].map((m) => m[1]).sort().join(",");

for (const [language, entries] of Object.entries(locales)) {
  for (const [key, english] of Object.entries(master)) {
    if (!(key in entries)) {
      (language === "de" ? errors : warnings).push(`${language}: missing ${key} (English fallback)`);
    } else if (typeof entries[key] !== "string" || !entries[key].trim()) {
      errors.push(`${language}: empty or non-string value for ${key}`);
    } else if (placeholders(english) !== placeholders(entries[key])) {
      errors.push(`${language}: placeholder mismatch for ${key}`);
    }
    if (language === "de" && /\[NEEDS TRANSLATION\]/i.test(entries[key] || "")) {
      errors.push(`de: untranslated marker for ${key}`);
    }
  }
  for (const key of Object.keys(entries)) {
    if (!(key in master)) warnings.push(`${language}: key without English entry: ${key}`);
  }
}

const sourceFiles = [...files(path.join(root, "src")), ...files(path.join(root, "src-tauri/src"))]
  .filter((file) => /\.(js|html|rs)$/.test(file));
for (const file of sourceFiles) {
  const source = fs.readFileSync(file, "utf8");
  const relative = path.relative(root, file);
  // Literal references only. Dynamic key families must be reviewed separately.
  const patterns = [
    /\b(?:t|translate|showToast|openModalMulti|openModalText|openModalNumber|openModalSlider|showTrailerActionError)\(\s*["']([\w]+(?:\.[\w-]+)+)["']/g,
    /data-translate(?:-placeholder|-aria-label|-title)?\s*=\s*["']([\w.-]+)["']/g,
  ];
  for (const pattern of patterns) {
    for (const match of source.matchAll(pattern)) {
      const key = match[1];
      referenced.add(key);
      if (!(key in master)) errors.push(`${relative}: undefined key ${key}`);
    }
  }
  // Include keys in copy maps without mistaking command names, paths or data for UI text.
  for (const match of source.matchAll(/["']([a-z_]+(?:\.[A-Za-z_0-9-]+)+)["']/g)) {
    if (match[1] in master) referenced.add(match[1]);
  }
}

const unused = Object.keys(master).filter((key) => !referenced.has(key));
const report = {
  locales: Object.keys(locales), englishKeys: Object.keys(master).length,
  germanKeys: Object.keys(locales.de).length, literalReferences: referenced.size,
  errors, warnings, unassignedCandidates: unused,
  note: "Unassigned candidates may be used by dynamic keys. This check does not prove that all UI text is localized.",
};
if (process.argv.includes("--json")) console.log(JSON.stringify(report, null, 2));
else {
  console.log(`${report.englishKeys} English / ${report.germanKeys} German keys; ${referenced.size} referenced keys.`);
  for (const error of errors) console.error(`ERROR ${error}`);
  console.log(`${errors.length} errors; ${warnings.length} fallback/extra-key warnings; ${unused.length} unassigned candidates.`);
  console.log("Use --json for the full report, including unassigned candidates and other-language fallbacks.");
}
process.exitCode = errors.length ? 1 : 0;
