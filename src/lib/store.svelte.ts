import { listen } from "@tauri-apps/api/event";
import { error as logError } from "@tauri-apps/plugin-log";
import { check, type Update } from "@tauri-apps/plugin-updater";
import { SvelteSet } from "svelte/reactivity";
import {
  api,
  type AppConfig,
  type BackupInfo,
  type CustomEntry,
  type Game,
  type ManifestStatus,
  type ProgressEvent,
  type ScanResult,
  type Store,
} from "./api";
import { formatSize } from "./format";
import { errorText, i18n, t } from "./i18n.svelte";

export type View = "library" | "unidentified" | "backups" | "disk" | "settings";
export type SortKey = "modified" | "name" | "size";

export interface Toast {
  id: number;
  text: string;
  kind: "info" | "error";
}

/** Dialogues de confirmation, ouverts depuis n'importe quel écran (boutons, menu, raccourcis). */
export type DialogState =
  | { kind: "delete"; gameId: string }
  | { kind: "restore"; gameId: string; backupPath?: string }
  | { kind: "batch"; gameIds: string[] }
  | { kind: "clean"; gameIds: string[] };

const DAY = 24 * 60 * 60 * 1000;

class AppStore {
  config = $state<AppConfig | null>(null);
  // Remplacés d'un bloc, jamais modifiés en place : pas besoin de réactivité profonde.
  scan = $state.raw<ScanResult | null>(null);
  backups = $state.raw<BackupInfo[]>([]);
  scanning = $state(false);
  manifest = $state<ManifestStatus | null>(null);
  view = $state<View>("library");
  selectedId = $state<string | null>(null);
  /** Sélection multiple (Ctrl/Maj + clic) dans la bibliothèque. */
  multi = new SvelteSet<string>();
  dialog = $state<DialogState | null>(null);
  progress = $state<ProgressEvent | null>(null);
  toasts = $state<Toast[]>([]);

  update = $state<Update | null>(null);
  checkingUpdate = $state(false);

  search = $state("");
  sort = $state<SortKey>("modified");
  sources = $state<Record<Store, boolean>>({
    steam: true,
    epic: true,
    ubisoft: true,
    gog: true,
    emulator: true,
    other: true,
  });
  showInstalled = $state(true);
  showNotInstalled = $state(true);
  account = $state<string>("");

  games = $derived<Game[]>(this.scan?.games ?? []);
  selected = $derived<Game | null>(this.games.find((g) => g.id === this.selectedId) ?? null);
  totalSize = $derived(this.games.reduce((sum, g) => sum + g.size, 0));
  filtered = $derived.by(() => this.#filter());

  #toastId = 0;

  #filter(): Game[] {
    const q = this.search.trim().toLowerCase();
    const list = this.games.filter(
      (g) =>
        this.sources[g.source] &&
        (g.installed ? this.showInstalled : this.showNotInstalled) &&
        (!this.account || g.accounts.includes(this.account)) &&
        (!q || g.title.toLowerCase().includes(q)),
    );
    const by: Record<SortKey, (a: Game, b: Game) => number> = {
      modified: (a, b) => (b.modified ?? 0) - (a.modified ?? 0),
      name: (a, b) => a.title.localeCompare(b.title, i18n.lang),
      size: (a, b) => b.size - a.size,
    };
    return list.sort(by[this.sort]);
  }

  game(id: string): Game | undefined {
    return this.games.find((g) => g.id === id);
  }

  backupsFor(title: string): BackupInfo[] {
    return this.backups.filter((b) => b.game === title);
  }

  toast(text: string, kind: Toast["kind"] = "info") {
    const id = ++this.#toastId;
    this.toasts.push({ id, text, kind });
    setTimeout(() => (this.toasts = this.toasts.filter((x) => x.id !== id)), kind === "error" ? 7000 : 4000);
  }

  fail(e: unknown) {
    const text = errorText(e);
    void logError(text).catch(() => {});
    this.toast(text, "error");
  }

  async init() {
    void listen<ProgressEvent>("progress", (ev) => {
      this.progress = ev.payload.finished ? null : ev.payload;
    });
    this.config = await api.getConfig();
    i18n.lang = this.config.language;
    this.scan = await api.getCachedScan();
    this.selectedId = this.scan?.games[0]?.id ?? null;
    this.manifest = await api.getManifestStatus();
    await this.refreshBackups();
    await this.rescan();
    if (this.config.checkUpdatesOnStart) void this.checkUpdate(true);
    await this.autoBackupIfDue(true);
    // L'app peut rester ouverte longtemps : on revérifie toutes les heures.
    setInterval(() => void this.autoBackupIfDue(false), 60 * 60 * 1000);
  }

  async saveConfig(patch: Partial<AppConfig>, notify = false) {
    if (!this.config) return;
    const next = { ...this.config, ...patch };
    try {
      await api.saveConfig(next);
      this.config = next;
      i18n.lang = next.language;
      if (notify) this.toast(t("settings.saved"));
    } catch (e) {
      this.fail(e);
    }
  }

  async rescan() {
    if (this.scanning) return;
    this.scanning = true;
    try {
      this.scan = await api.runScan();
      if (!this.selected) this.selectedId = this.scan.games[0]?.id ?? null;
      for (const id of [...this.multi]) if (!this.game(id)) this.multi.delete(id);
      this.manifest = await api.getManifestStatus();
    } catch (e) {
      this.fail(e);
    } finally {
      this.scanning = false;
    }
  }

  async refreshBackups() {
    try {
      this.backups = await api.listBackups();
    } catch (e) {
      this.fail(e);
    }
  }

  /** `false` (et ouverture des paramètres) si aucun dossier de backup n'est choisi. */
  requireBackupDir(): boolean {
    if (this.config?.backupDir) return true;
    this.view = "settings";
    this.toast(t("err.no_backup_dir"), "error");
    return false;
  }

  async backupGame(id: string) {
    if (!this.requireBackupDir()) return;
    try {
      const info = await api.createBackup(id);
      this.toast(t("backup.created", { name: info.fileName }));
      await this.refreshBackups();
    } catch (e) {
      this.fail(e);
    }
  }

  async backupMany(ids: string[], onlyChanged: boolean, auto = false) {
    if (!this.requireBackupDir() || ids.length === 0) return;
    try {
      const report = await api.backupGames(ids, onlyChanged, auto);
      if (!auto || report.created > 0) {
        this.toast(t(auto ? "batch.autoDone" : "batch.done", { created: report.created, skipped: report.skipped }));
      }
      if (report.failed.length) {
        this.toast(t("batch.failed", { games: report.failed.map((f) => f.game).join(", ") }), "error");
      }
      await this.refreshBackups();
    } catch (e) {
      this.fail(e);
    }
  }

  /** Backup automatique des jeux modifiés, selon la fréquence choisie dans les paramètres. */
  async autoBackupIfDue(atStartup: boolean) {
    const mode = this.config?.autoBackup ?? "off";
    if (mode === "off" || !this.config?.backupDir || !this.scan) return;
    const elapsed = Date.now() - (this.config.lastAutoBackup ?? 0);
    const due = mode === "startup" ? atStartup : elapsed >= (mode === "daily" ? DAY : 7 * DAY);
    if (!due) return;
    await this.saveConfig({ lastAutoBackup: Date.now() });
    await this.backupMany(
      this.games.map((g) => g.id),
      true,
      true,
    );
  }

  async deleteGame(id: string) {
    const game = this.game(id);
    if (!game) return;
    try {
      await api.deleteGameSaves(id);
      this.toast(t("confirm.done", { game: game.title }));
      this.multi.delete(id);
      this.scan = await api.getCachedScan();
      if (this.selectedId === id) this.selectedId = this.filtered[0]?.id ?? null;
      await this.refreshBackups();
    } catch (e) {
      this.fail(e);
    }
  }

  /** Supprime (avec backup) les saves de plusieurs jeux. */
  async deleteGames(ids: string[]) {
    let freed = 0;
    let done = 0;
    for (const id of ids) {
      const game = this.game(id);
      if (!game) continue;
      try {
        await api.deleteGameSaves(id);
        freed += game.size;
        done++;
        this.multi.delete(id);
      } catch (e) {
        this.fail(e);
      }
    }
    this.scan = await api.getCachedScan();
    if (this.selectedId && !this.game(this.selectedId)) this.selectedId = this.filtered[0]?.id ?? null;
    await this.refreshBackups();
    this.toast(t("disk.cleanDone", { n: done, size: formatSize(freed) }));
  }

  async restore(path: string) {
    try {
      await api.restoreBackup(path);
      this.toast(t("backup.restored"));
      await this.refreshBackups();
      await this.rescan();
    } catch (e) {
      this.fail(e);
    }
  }

  /** Rattache des dossiers à des jeux : une seule écriture de config et un seul scan. */
  async attach(entries: CustomEntry[]) {
    if (!this.config || entries.length === 0) return;
    const known = new Set(this.config.customEntries.map((e) => e.path));
    const fresh = entries
      .map((e) => ({ game: e.game.trim(), path: e.path }))
      .filter((e) => e.game && !known.has(e.path));
    await this.saveConfig({ customEntries: [...this.config.customEntries, ...fresh] });
    await this.rescan();
  }

  async ignore(paths: string[]) {
    if (!this.config || paths.length === 0) return;
    await this.saveConfig({ ignoredPaths: [...this.config.ignoredPaths, ...paths] });
    if (this.scan) {
      const drop = new Set(paths);
      this.scan = { ...this.scan, unidentified: this.scan.unidentified.filter((u) => !drop.has(u.path)) };
    }
  }

  async checkUpdate(silent = false) {
    this.checkingUpdate = true;
    try {
      this.update = await check();
      if (!this.update && !silent) this.toast(t("settings.upToDate"));
    } catch (e) {
      if (!silent) this.toast(t("settings.updateError", { e: errorText(e) }), "error");
    } finally {
      this.checkingUpdate = false;
    }
  }
}

export const app = new AppStore();
