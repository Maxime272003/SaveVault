// Calcule la prochaine version à partir des Conventional Commits depuis le dernier tag `v*`.
// Écrit `release`, `version` et `notes` dans $GITHUB_OUTPUT (ou les affiche en local).
import { execSync } from "node:child_process";
import { appendFileSync, readFileSync } from "node:fs";

const sh = (cmd) => execSync(cmd, { encoding: "utf8", stdio: ["ignore", "pipe", "pipe"] }).trim();

let lastTag = "";
try {
  lastTag = sh('git describe --tags --abbrev=0 --match "v*"');
} catch {
  // Aucun tag : première release.
}

const range = lastTag ? `${lastTag}..HEAD` : "HEAD";
const raw = sh(`git log ${range} --format=%s%n%b%x1e`);
const commits = raw
  .split("\x1e")
  .map((c) => c.trim())
  .filter(Boolean)
  .map((c) => {
    const [subject, ...body] = c.split("\n");
    const m = subject.match(/^(\w+)(\([^)]*\))?(!)?:\s*(.+)$/);
    return {
      type: m?.[1]?.toLowerCase() ?? "other",
      breaking: Boolean(m?.[3]) || body.join("\n").includes("BREAKING CHANGE"),
      description: m?.[4] ?? subject,
    };
  });

let bump = null;
for (const c of commits) {
  if (c.breaking) bump = "major";
  else if (c.type === "feat" && bump !== "major") bump = "minor";
  else if ((c.type === "fix" || c.type === "perf") && !bump) bump = "patch";
}

let version;
if (!lastTag) {
  version = JSON.parse(readFileSync("package.json", "utf8")).version;
} else if (bump) {
  let [major, minor, patch] = lastTag.replace(/^v/, "").split(".").map(Number);
  // En 0.x, un changement cassant ne fait qu'incrémenter le mineur.
  if (bump === "major" && major === 0) bump = "minor";
  if (bump === "major") [major, minor, patch] = [major + 1, 0, 0];
  else if (bump === "minor") [minor, patch] = [minor + 1, 0];
  else patch += 1;
  version = `${major}.${minor}.${patch}`;
}

const release = Boolean(version);
const notes = commits
  .filter((c) => ["feat", "fix", "perf"].includes(c.type) || c.breaking)
  .map((c) => `- ${c.type}: ${c.description}`)
  .join("\n");

const out = process.env.GITHUB_OUTPUT;
if (out) {
  appendFileSync(out, `release=${release}\nversion=${version ?? ""}\nnotes<<__NOTES__\n${notes}\n__NOTES__\n`);
}
console.log(release ? `Nouvelle version : ${version}\n${notes}` : "Aucun commit feat/fix depuis le dernier tag : pas de release.");
