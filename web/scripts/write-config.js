const fs = require("fs");
const path = require("path");

const raw = (process.env.CHESS_API_URL || "http://localhost:8080").trim();
let apiUrl = "";
try {
  const parsed = new URL(raw);
  if (parsed.protocol === "http:" || parsed.protocol === "https:") {
    apiUrl = `${parsed.origin}${parsed.pathname}`.replace(/\/$/, "");
  }
} catch {
  apiUrl = "";
}

const file = path.join(__dirname, "..", "config.js");
fs.writeFileSync(file, `window.CHESS_API_URL = ${JSON.stringify(apiUrl)};\n`);
console.log(`Wrote ${file} with CHESS_API_URL=${apiUrl || "(empty)"}`);
