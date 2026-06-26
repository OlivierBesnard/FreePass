// Rasterize assets/app-icon.svg -> assets/app-icon.png (1024x1024), the master
// for `pnpm tauri icon`. Headless (resvg), no browser needed.
import { readFileSync, writeFileSync } from "node:fs";
import { Resvg } from "@resvg/resvg-js";

const svg = readFileSync("assets/app-icon.svg", "utf8");
const resvg = new Resvg(svg, { fitTo: { mode: "width", value: 1024 } });
const png = resvg.render().asPng();
writeFileSync("assets/app-icon.png", png);
console.log("wrote assets/app-icon.png (1024x1024)");
